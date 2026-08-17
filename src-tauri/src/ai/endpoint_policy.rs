use std::net::{IpAddr, Ipv6Addr};

use serde::Serialize;
use url::{Host, Url};

use crate::error::CommandError;

use super::catalog::ProviderKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkScope {
    Local,
    Lan,
    Cloud,
}

#[derive(Clone, Debug)]
pub struct ValidatedEndpoint {
    pub normalized_url: String,
    pub scope: NetworkScope,
    pub origin: String,
}

pub fn validate_endpoint(
    provider: ProviderKind,
    raw_url: &str,
    allow_insecure_http: bool,
) -> Result<ValidatedEndpoint, CommandError> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 {
        return Err(CommandError::new(
            "invalid_ai_base_url",
            "Enter an AI server URL no longer than 2,048 characters.",
        ));
    }

    let mut url = Url::parse(trimmed).map_err(|_| {
        CommandError::new(
            "invalid_ai_base_url",
            "Enter a complete http or https AI server URL.",
        )
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(CommandError::new(
            "invalid_ai_base_url",
            "AI server URLs must use http or https.",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(CommandError::new(
            "invalid_ai_base_url",
            "Do not put usernames or credentials in an AI server URL.",
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(CommandError::new(
            "invalid_ai_base_url",
            "AI server URLs cannot include a query string or fragment.",
        ));
    }
    let host = url.host().ok_or_else(|| {
        CommandError::new(
            "invalid_ai_base_url",
            "The AI server URL must include a host.",
        )
    })?;
    let scope = classify_host(host);
    if url.scheme() == "http" {
        match scope {
            NetworkScope::Local => {}
            NetworkScope::Lan if allow_insecure_http => {}
            NetworkScope::Lan => {
                return Err(CommandError::new(
                    "ai_insecure_http_confirmation_required",
                    "Confirm that you accept unencrypted HTTP traffic to this local-network server.",
                ));
            }
            NetworkScope::Cloud => {
                return Err(CommandError::new(
                    "invalid_ai_base_url",
                    "Remote AI providers must use HTTPS.",
                ));
            }
        }
    }

    normalize_path(&mut url);
    let normalized_url = url.to_string();

    if let Some(fixed) = provider.fixed_base_url() {
        let fixed_url = Url::parse(fixed).expect("built-in provider URL should be valid");
        if !same_origin_and_path(&url, &fixed_url) {
            return Err(CommandError::new(
                "invalid_ai_base_url",
                "This built-in cloud provider uses a fixed official API endpoint.",
            ));
        }
    }

    let origin = endpoint_origin(&url)?;
    Ok(ValidatedEndpoint {
        normalized_url,
        scope,
        origin,
    })
}

pub fn endpoint_origin_from_str(value: &str) -> Result<String, CommandError> {
    let url = Url::parse(value).map_err(|_| {
        CommandError::new("invalid_ai_base_url", "The saved AI server URL is invalid.")
    })?;
    endpoint_origin(&url)
}

pub fn join_endpoint(base_url: &str, relative: &str) -> Result<Url, CommandError> {
    let base = Url::parse(base_url).map_err(|_| {
        CommandError::new("invalid_ai_base_url", "The saved AI server URL is invalid.")
    })?;
    base.join(relative).map_err(|_| {
        CommandError::new(
            "invalid_ai_base_url",
            "The AI provider endpoint could not be constructed.",
        )
    })
}

fn normalize_path(url: &mut Url) {
    let path = url.path();
    if path.is_empty() {
        url.set_path("/");
    } else if !path.ends_with('/') {
        let mut normalized = path.to_string();
        normalized.push('/');
        url.set_path(&normalized);
    }
}

fn endpoint_origin(url: &Url) -> Result<String, CommandError> {
    let host = url.host_str().ok_or_else(|| {
        CommandError::new(
            "invalid_ai_base_url",
            "The AI server URL must include a host.",
        )
    })?;
    let mut origin = format!("{}://{}", url.scheme(), format_host(host));
    if let Some(port) = url.port() {
        origin.push(':');
        origin.push_str(&port.to_string());
    }
    Ok(origin)
}

fn format_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

fn same_origin_and_path(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
        && left.path() == right.path()
}

fn classify_host(host: Host<&str>) -> NetworkScope {
    match host {
        Host::Domain(domain) => {
            let normalized = domain.trim_end_matches('.').to_ascii_lowercase();
            if normalized == "localhost" {
                NetworkScope::Local
            } else if normalized.ends_with(".local") {
                NetworkScope::Lan
            } else {
                NetworkScope::Cloud
            }
        }
        Host::Ipv4(address) => classify_ip(IpAddr::V4(address)),
        Host::Ipv6(address) => classify_ip(IpAddr::V6(address)),
    }
}

fn classify_ip(address: IpAddr) -> NetworkScope {
    if address.is_loopback() {
        return NetworkScope::Local;
    }
    match address {
        IpAddr::V4(value) if value.is_private() || value.is_link_local() => NetworkScope::Lan,
        IpAddr::V6(value) if is_private_or_link_local_v6(value) => NetworkScope::Lan,
        _ => NetworkScope::Cloud,
    }
}

fn is_private_or_link_local_v6(address: Ipv6Addr) -> bool {
    let first = address.segments()[0];
    (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permits_loopback_http_and_normalizes_trailing_slash() {
        let endpoint = validate_endpoint(ProviderKind::LlamaCpp, "http://127.0.0.1:8080/v1", false)
            .expect("loopback should be valid");
        assert_eq!(endpoint.normalized_url, "http://127.0.0.1:8080/v1/");
        assert_eq!(endpoint.scope, NetworkScope::Local);
    }

    #[test]
    fn requires_explicit_lan_http_acknowledgement() {
        assert!(
            validate_endpoint(ProviderKind::LlamaCpp, "http://192.168.1.10:8080/", false,).is_err()
        );
        assert_eq!(
            validate_endpoint(ProviderKind::LlamaCpp, "http://192.168.1.10:8080/", true,)
                .expect("acknowledged LAN URL should work")
                .scope,
            NetworkScope::Lan
        );
    }

    #[test]
    fn rejects_public_cleartext_and_url_credentials() {
        assert!(validate_endpoint(ProviderKind::LlamaCpp, "http://example.com/", true,).is_err());
        assert!(validate_endpoint(
            ProviderKind::LlamaCpp,
            "https://user:secret@example.com/",
            false,
        )
        .is_err());
    }

    #[test]
    fn keeps_openrouter_on_the_official_endpoint() {
        assert!(validate_endpoint(
            ProviderKind::Openrouter,
            "https://openrouter.ai/api/v1/",
            false,
        )
        .is_ok());
        assert!(validate_endpoint(
            ProviderKind::Openrouter,
            "https://proxy.example.com/",
            false,
        )
        .is_err());
    }

    #[test]
    fn keeps_deepseek_on_the_official_endpoint() {
        assert!(
            validate_endpoint(ProviderKind::Deepseek, "https://api.deepseek.com/", false,).is_ok()
        );
        assert!(
            validate_endpoint(ProviderKind::Deepseek, "https://proxy.example.com/", false,)
                .is_err()
        );
    }
}
