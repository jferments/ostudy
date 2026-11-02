//! One-click OpenRouter sign-in: OAuth with PKCE and a loopback callback.
//!
//! The learner approves oStudy in their browser; OpenRouter redirects to a
//! listener on this device with a one-time code, and the code (with the
//! verifier only this process knows) is exchanged for an API key. On Android
//! the browser is a separate app, so the page it lands on offers a link that
//! brings oStudy back to the front; the code has already arrived by then, and
//! an exchange the system blocked while oStudy was in the background is tried
//! again until it gets through. The key
//! stays native: it is held here until it is saved as an OpenRouter
//! service's credential, and React only ever sees an attempt identifier.
//!
//! The flow is split so no library lease is held while the learner is in the
//! browser: `start` binds the listener, `wait` takes the callback and
//! exchanges the code without touching the library, and `save` stores the key
//! through the ordinary credential path.

use std::{
    io::{ErrorKind, Read, Write},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    database::Database, error::CommandError, platform::BROWSER_RETURN_URL, util::validate_entity_id,
};

use super::{
    super::{catalog::ProviderKind, AiRuntimeState},
    contracts::{AiProviderProfileDetailDto, SetAiProviderCredentialInput},
    repository, service,
};
use crate::ai::secrets::CredentialPersistence;

const AUTH_URL: &str = "https://openrouter.ai/auth";
const EXCHANGE_URL: &str = "https://openrouter.ai/api/v1/auth/keys";
const CALLBACK_PATH: &str = "/openrouter-callback";
/// OpenRouter's code is single-use and lasts ten minutes.
const WAIT_LIMIT: Duration = Duration::from_secs(10 * 60);
const POLL_INTERVAL: Duration = Duration::from_millis(150);
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REQUEST_BYTES: usize = 8 * 1024;
const EXCHANGE_TIMEOUT_MS: i64 = 30_000;
const MAX_EXCHANGE_BYTES: usize = 64 * 1024;
/// How long a code exchange that could not reach OpenRouter keeps trying. On
/// Android the code arrives while the browser is in front, and the system can
/// refuse a background app's connections until oStudy is open again; a failed
/// connection never reached OpenRouter, so the single-use code is still good.
const EXCHANGE_RETRY_LIMIT: Duration = Duration::from_secs(3 * 60);
const EXCHANGE_RETRY_DELAYS: [Duration; 4] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(3),
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRouterSignInStartDto {
    pub attempt_id: String,
    /// The page to open in the learner's browser. It carries only the PKCE
    /// challenge and the loopback address, never a secret.
    pub authorization_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenRouterSignInAttemptInput {
    pub attempt_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveOpenRouterSignInInput {
    pub attempt_id: String,
    pub operation_id: String,
    pub profile_id: String,
    pub revision: i64,
    pub persistence: CredentialPersistence,
}

struct Attempt {
    id: String,
    verifier: String,
    listeners: Vec<TcpListener>,
    canceled: Arc<AtomicBool>,
    key: Option<String>,
}

/// The one sign-in in progress. Starting another cancels the previous one.
#[derive(Default)]
pub(crate) struct SignInSlot {
    current: Mutex<Option<Attempt>>,
}

impl SignInSlot {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Attempt>> {
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub(super) fn start(runtime: &AiRuntimeState) -> Result<OpenRouterSignInStartDto, CommandError> {
    if cfg!(mobile) && BROWSER_RETURN_URL.is_none() {
        return Err(CommandError::new(
            "openrouter_sign_in_unavailable",
            "One-click sign-in is not available on this device. Paste an OpenRouter key instead.",
        ));
    }
    let listeners = bind_loopback()?;
    let port = listeners
        .first()
        .and_then(|listener| listener.local_addr().ok())
        .map(|address| address.port())
        .ok_or_else(sign_in_failed)?;
    let verifier = URL_SAFE_NO_PAD.encode(random_bytes::<32>()?);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let attempt_id = hex(&random_bytes::<16>()?);
    let url = authorization_url(port, &challenge)?;

    let mut slot = runtime.sign_in.lock();
    if let Some(previous) = slot.take() {
        previous.canceled.store(true, Ordering::SeqCst);
    }
    *slot = Some(Attempt {
        id: attempt_id.clone(),
        verifier,
        listeners,
        canceled: Arc::new(AtomicBool::new(false)),
        key: None,
    });
    Ok(OpenRouterSignInStartDto {
        attempt_id,
        authorization_url: url.to_string(),
    })
}

/// Waits for the browser to come back, then exchanges the code for a key and
/// keeps it for `save`. Resolves once the key is held.
pub(super) async fn wait(
    runtime: &AiRuntimeState,
    input: OpenRouterSignInAttemptInput,
) -> Result<(), CommandError> {
    let (listeners, canceled, verifier) = {
        let mut slot = runtime.sign_in.lock();
        let attempt = current(&mut slot, &input.attempt_id)?;
        if attempt.listeners.is_empty() {
            return Err(CommandError::new(
                "openrouter_sign_in_invalid",
                "This sign-in is already waiting or finished.",
            ));
        }
        (
            std::mem::take(&mut attempt.listeners),
            attempt.canceled.clone(),
            attempt.verifier.clone(),
        )
    };

    let receiving = canceled.clone();
    let code = tauri::async_runtime::spawn_blocking(move || {
        receive_code(&listeners, &receiving, Instant::now() + WAIT_LIMIT)
    })
    .await
    .map_err(|_| sign_in_failed())??;

    let url = Url::parse(EXCHANGE_URL).map_err(|_| sign_in_failed())?;
    let body = json!({
        "code": code,
        "code_verifier": verifier,
        "code_challenge_method": "S256",
    });
    let started = Instant::now();
    let mut attempt_number = 0_usize;
    let response = loop {
        match runtime
            .gateway
            .post_json(
                url.clone(),
                None,
                &body,
                EXCHANGE_TIMEOUT_MS,
                MAX_EXCHANGE_BYTES,
            )
            .await
        {
            Ok(response) => break response,
            Err(failure) => {
                let Some(delay) =
                    exchange_retry_delay(&failure.safe.code, attempt_number, started.elapsed())
                else {
                    return Err(CommandError::new(failure.safe.code, failure.safe.message));
                };
                tokio::time::sleep(delay).await;
                if canceled.load(Ordering::SeqCst) {
                    return Err(canceled_error());
                }
                attempt_number += 1;
            }
        }
    };
    let key = exchanged_key(&response)?;

    let mut slot = runtime.sign_in.lock();
    let attempt = current(&mut slot, &input.attempt_id)?;
    if attempt.canceled.load(Ordering::SeqCst) {
        return Err(canceled_error());
    }
    attempt.key = Some(key);
    Ok(())
}

/// Saves the exchanged key as the given OpenRouter service's credential, the
/// same way a pasted key is saved (including first-key task defaults).
pub(super) fn save(
    database: &Database,
    runtime: &AiRuntimeState,
    input: SaveOpenRouterSignInInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_entity_id(&input.profile_id, "AI provider profile")?;
    let profile = database
        .with_connection(|connection| repository::load_profile(connection, &input.profile_id))?;
    if profile.provider_kind != ProviderKind::Openrouter {
        return Err(CommandError::new(
            "openrouter_sign_in_invalid",
            "An OpenRouter sign-in can only connect an OpenRouter service.",
        ));
    }
    let key = {
        let mut slot = runtime.sign_in.lock();
        let attempt = current(&mut slot, &input.attempt_id)?;
        attempt.key.clone().ok_or_else(|| {
            CommandError::new(
                "openrouter_sign_in_invalid",
                "This sign-in has not finished in the browser yet.",
            )
        })?
    };
    let saved = service::set_credential(
        database,
        runtime,
        SetAiProviderCredentialInput {
            operation_id: input.operation_id,
            profile_id: input.profile_id,
            revision: input.revision,
            credential: key,
            persistence: input.persistence,
        },
    )?;
    let mut slot = runtime.sign_in.lock();
    if slot
        .as_ref()
        .is_some_and(|attempt| attempt.id == input.attempt_id)
    {
        *slot = None;
    }
    Ok(saved)
}

/// Stops a sign-in; its wait resolves as canceled and any key it held is dropped.
pub(super) fn cancel(runtime: &AiRuntimeState, input: OpenRouterSignInAttemptInput) {
    let mut slot = runtime.sign_in.lock();
    if let Some(attempt) = slot.as_ref() {
        if attempt.id == input.attempt_id {
            attempt.canceled.store(true, Ordering::SeqCst);
            *slot = None;
        }
    }
}

fn current<'a>(
    slot: &'a mut Option<Attempt>,
    attempt_id: &str,
) -> Result<&'a mut Attempt, CommandError> {
    match slot.as_mut() {
        Some(attempt) if attempt.id == attempt_id => Ok(attempt),
        _ => Err(canceled_error()),
    }
}

/// Listens on both loopback addresses: browsers may resolve `localhost` to
/// either. The IPv6 listener is best effort on the IPv4 listener's port.
fn bind_loopback() -> Result<Vec<TcpListener>, CommandError> {
    let v4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .map_err(|_| sign_in_failed())?;
    let port = v4.local_addr().map_err(|_| sign_in_failed())?.port();
    let mut listeners = vec![v4];
    if let Ok(v6) = TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port))) {
        listeners.push(v6);
    }
    for listener in &listeners {
        listener
            .set_nonblocking(true)
            .map_err(|_| sign_in_failed())?;
    }
    Ok(listeners)
}

fn authorization_url(port: u16, challenge: &str) -> Result<Url, CommandError> {
    let mut url = Url::parse(AUTH_URL).map_err(|_| sign_in_failed())?;
    url.query_pairs_mut()
        .append_pair(
            "callback_url",
            &format!("http://localhost:{port}{CALLBACK_PATH}"),
        )
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url)
}

fn receive_code(
    listeners: &[TcpListener],
    canceled: &AtomicBool,
    deadline: Instant,
) -> Result<String, CommandError> {
    loop {
        if canceled.load(Ordering::SeqCst) {
            return Err(canceled_error());
        }
        if Instant::now() >= deadline {
            return Err(CommandError::new(
                "openrouter_sign_in_timed_out",
                "The OpenRouter sign-in took too long. Try again.",
            ));
        }
        for listener in listeners {
            match listener.accept() {
                Ok((stream, _)) => {
                    if let Some(outcome) = answer(stream) {
                        return outcome;
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(_) => return Err(sign_in_failed()),
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Answers one browser request. Returns `None` for anything that is not the
/// callback (a favicon request, say), so waiting continues.
fn answer(mut stream: TcpStream) -> Option<Result<String, CommandError>> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(REQUEST_READ_TIMEOUT));
    let request = read_request_head(&mut stream)?;
    let outcome = callback_outcome(&request);
    let (status, body) = callback_page(outcome.as_ref(), BROWSER_RETURN_URL);
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
    outcome
}

/// The page the browser shows after OpenRouter sends it back. With a return
/// link (Android), the learner taps it to get back to oStudy.
fn callback_page(
    outcome: Option<&Result<String, CommandError>>,
    return_url: Option<&str>,
) -> (&'static str, String) {
    let (status, message) = match (outcome, return_url) {
        (None, _) => ("404 Not Found", "Not found."),
        (Some(Ok(_)), None) => (
            "200 OK",
            "oStudy is connecting to OpenRouter. You can close this tab and go back to oStudy.",
        ),
        (Some(Ok(_)), Some(_)) => ("200 OK", "oStudy is connecting to OpenRouter."),
        (Some(Err(_)), _) => (
            "200 OK",
            "OpenRouter did not connect oStudy. Go back to oStudy to try again or paste a key.",
        ),
    };
    let link = match (outcome, return_url) {
        (Some(_), Some(url)) => format!(
            "<p><a href=\"{url}\" style=\"display:inline-block;padding:.75rem 1.25rem;\
             border-radius:.5rem;background:#1f6feb;color:#fff;text-decoration:none\">\
             Back to oStudy</a></p>"
        ),
        _ => String::new(),
    };
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>oStudy</title>\
         <body style=\"font:16px system-ui,sans-serif;margin:3rem 1.5rem\"><p>{message}</p>{link}</body>"
    );
    (status, body)
}

fn read_request_head(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    while buffer.len() < MAX_REQUEST_BYTES {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(buffer).ok()
}

/// Reads the code, or OpenRouter's refusal, from the callback's request line.
fn callback_outcome(request: &str) -> Option<Result<String, CommandError>> {
    let target = request
        .lines()
        .next()?
        .strip_prefix("GET ")?
        .split(' ')
        .next()?;
    let url = Url::parse(&format!("http://localhost{target}")).ok()?;
    if url.path() != CALLBACK_PATH {
        return None;
    }
    let mut code = None;
    let mut refused = false;
    for (name, value) in url.query_pairs() {
        match name.as_ref() {
            "code" if !value.trim().is_empty() => code = Some(value.into_owned()),
            "error" => refused = true,
            _ => {}
        }
    }
    Some(match code {
        Some(code) if !refused => Ok(code),
        _ => Err(CommandError::new(
            "openrouter_sign_in_denied",
            "OpenRouter did not approve oStudy. Try again, or paste a key instead.",
        )),
    })
}

/// The pause before the code exchange is tried again, or `None` to give up.
/// Only a connection that never reached OpenRouter is retried: any answer,
/// or a timeout after the request may have arrived, could mean the code was
/// already spent.
fn exchange_retry_delay(code: &str, attempt_number: usize, elapsed: Duration) -> Option<Duration> {
    if code != "ai_connection_failed" || elapsed >= EXCHANGE_RETRY_LIMIT {
        return None;
    }
    Some(EXCHANGE_RETRY_DELAYS[attempt_number.min(EXCHANGE_RETRY_DELAYS.len() - 1)])
}

fn exchanged_key(response: &Value) -> Result<String, CommandError> {
    response
        .get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            CommandError::new(
                "openrouter_sign_in_failed",
                "OpenRouter did not return a key. Try again, or paste a key instead.",
            )
        })
}

fn random_bytes<const N: usize>() -> Result<[u8; N], CommandError> {
    let mut value = [0_u8; N];
    SystemRandom::new()
        .fill(&mut value)
        .map_err(|_| sign_in_failed())?;
    Ok(value)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn canceled_error() -> CommandError {
    CommandError::new(
        "openrouter_sign_in_canceled",
        "The OpenRouter sign-in was stopped.",
    )
}

fn sign_in_failed() -> CommandError {
    CommandError::new(
        "openrouter_sign_in_failed",
        "One-click sign-in could not start. Paste an OpenRouter key instead.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_authorization_url_carries_the_loopback_callback_and_s256_challenge() {
        let url = authorization_url(43_210, "challenge-value").unwrap();
        assert_eq!(url.host_str(), Some("openrouter.ai"));
        assert_eq!(url.path(), "/auth");
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        assert!(pairs.contains(&(
            "callback_url".into(),
            "http://localhost:43210/openrouter-callback".into()
        )));
        assert!(pairs.contains(&("code_challenge".into(), "challenge-value".into())));
        assert!(pairs.contains(&("code_challenge_method".into(), "S256".into())));
    }

    #[test]
    fn the_callback_yields_its_code_and_ignores_other_requests() {
        let request = "GET /openrouter-callback?code=abc-123 HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(callback_outcome(request).unwrap().unwrap(), "abc-123");
        assert!(callback_outcome("GET /favicon.ico HTTP/1.1\r\n\r\n").is_none());
        assert!(callback_outcome("POST /openrouter-callback HTTP/1.1\r\n\r\n").is_none());
        let refused =
            callback_outcome("GET /openrouter-callback?error=access_denied HTTP/1.1\r\n\r\n")
                .unwrap()
                .unwrap_err();
        assert_eq!(refused.code, "openrouter_sign_in_denied");
    }

    #[test]
    fn the_callback_page_offers_a_way_back_only_where_one_exists() {
        let received: Result<String, CommandError> = Ok("code".into());
        let (status, desktop) = callback_page(Some(&received), None);
        assert_eq!(status, "200 OK");
        assert!(desktop.contains("close this tab"));
        assert!(!desktop.contains("<a "));

        let back = "io.github.jferments.ostudy.dev://openrouter-signed-in";
        let (_, android) = callback_page(Some(&received), Some(back));
        assert!(android.contains(&format!("href=\"{back}\"")));
        assert!(android.contains("Back to oStudy"));
        let refused = Err(canceled_error());
        assert!(callback_page(Some(&refused), Some(back)).1.contains(back));

        let (missing, page) = callback_page(None, Some(back));
        assert_eq!(missing, "404 Not Found");
        assert!(!page.contains(back));
    }

    #[test]
    fn only_a_refused_connection_retries_the_exchange_and_only_for_a_while() {
        let soon = Duration::from_secs(1);
        assert_eq!(
            exchange_retry_delay("ai_connection_failed", 0, soon),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            exchange_retry_delay("ai_connection_failed", 40, soon),
            Some(Duration::from_secs(3))
        );
        assert_eq!(
            exchange_retry_delay("ai_connection_failed", 3, EXCHANGE_RETRY_LIMIT),
            None
        );
        for spent in ["ai_request_timeout", "ai_network_error", "ai_bad_request"] {
            assert_eq!(exchange_retry_delay(spent, 0, soon), None, "{spent}");
        }
    }

    #[test]
    fn only_a_nonempty_key_is_accepted_from_the_exchange() {
        assert_eq!(
            exchanged_key(&json!({ "key": "sk-or-v1-test" })).unwrap(),
            "sk-or-v1-test"
        );
        assert!(exchanged_key(&json!({ "key": " " })).is_err());
        assert!(exchanged_key(&json!({})).is_err());
    }

    #[test]
    fn a_browser_return_to_the_listener_delivers_the_code_and_a_page() {
        let listeners = bind_loopback().unwrap();
        let port = listeners[0].local_addr().unwrap().port();
        let canceled = AtomicBool::new(false);
        let browser = std::thread::spawn(move || {
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            stream
                .write_all(b"GET /openrouter-callback?code=from-browser HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut page = String::new();
            stream.read_to_string(&mut page).unwrap();
            page
        });
        let code = receive_code(
            &listeners,
            &canceled,
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(code, "from-browser");
        let page = browser.join().unwrap();
        assert!(page.starts_with("HTTP/1.1 200 OK"));
        assert!(page.contains("You can close this tab"));
    }

    #[test]
    fn a_canceled_wait_stops_without_a_code() {
        let listeners = bind_loopback().unwrap();
        let canceled = AtomicBool::new(true);
        let error = receive_code(
            &listeners,
            &canceled,
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap_err();
        assert_eq!(error.code, "openrouter_sign_in_canceled");
    }

    #[test]
    fn save_refuses_an_attempt_that_never_finished() {
        let database = Database::open_in_memory().unwrap();
        let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
        let started = start(&runtime).unwrap();
        let mut input = super::super::tests::default_input(ProviderKind::Openrouter);
        input.operation_id = "31313131313131313131313131313131".to_string();
        let profile = service::create_profile(&database, &runtime, input).unwrap();
        let error = save(
            &database,
            &runtime,
            SaveOpenRouterSignInInput {
                attempt_id: started.attempt_id.clone(),
                operation_id: "32323232323232323232323232323232".to_string(),
                profile_id: profile.id,
                revision: profile.revision,
                persistence: CredentialPersistence::Session,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "openrouter_sign_in_invalid");
        cancel(
            &runtime,
            OpenRouterSignInAttemptInput {
                attempt_id: started.attempt_id,
            },
        );
        assert!(runtime.sign_in.lock().is_none());
    }
}
