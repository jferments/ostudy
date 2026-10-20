//! Create one study guide headlessly and write what the run produced.
//!
//! cargo run --features guide-eval --bin guide-eval -- \
//!     --source chapter.pdf --out runs/chapter --provider deepseek \
//!     --base-url https://api.deepseek.com --model deepseek-chat
//!
//! The provider key, when the provider needs one, comes from
//! OSTUDY_EVAL_API_KEY. The run writes <out>/outcome.json and keeps the
//! isolated library in <out>/library for usage reports.

use std::{path::PathBuf, process::ExitCode};

use ostudy_lib::guide_eval::{self, GuideEvalProvider, GuideEvalRequest};

const USAGE: &str =
    "usage: guide-eval --source FILE --out DIR --provider llama_cpp|deepseek|openrouter \
--base-url URL --model ID [--reasoning EFFORT] [--timeout-ms N] [--allow-insecure-http]";

fn main() -> ExitCode {
    match parse(std::env::args().skip(1).collect()) {
        Ok((request, out)) => run(request, out),
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run(request: GuideEvalRequest, out: PathBuf) -> ExitCode {
    let outcome = match guide_eval::run(request) {
        Ok(outcome) => outcome,
        Err(message) => {
            eprintln!("guide-eval could not start: {message}");
            return ExitCode::FAILURE;
        }
    };
    let failed = outcome.error.is_some();
    let written = serde_json::to_vec_pretty(&outcome)
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            std::fs::write(out.join("outcome.json"), bytes).map_err(|error| error.to_string())
        });
    if let Err(message) = written {
        eprintln!("guide-eval could not write outcome.json: {message}");
        return ExitCode::FAILURE;
    }
    if let Some(error) = &outcome.error {
        eprintln!(
            "guide-eval failed at {}: {} ({})",
            error.stage, error.message, error.code
        );
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn parse(arguments: Vec<String>) -> Result<(GuideEvalRequest, PathBuf), String> {
    let mut source = None;
    let mut out = None;
    let mut kind = None;
    let mut base_url = None;
    let mut model_id = None;
    let mut reasoning_effort = "automatic".to_string();
    let mut request_timeout_ms = 300_000_i64;
    let mut allow_insecure_http = false;
    let mut arguments = arguments.into_iter();
    while let Some(flag) = arguments.next() {
        if flag == "--allow-insecure-http" {
            allow_insecure_http = true;
            continue;
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--source" => source = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--provider" => kind = Some(value),
            "--base-url" => base_url = Some(value),
            "--model" => model_id = Some(value),
            "--reasoning" => reasoning_effort = value,
            "--timeout-ms" => {
                request_timeout_ms = value
                    .parse()
                    .map_err(|_| format!("--timeout-ms must be a number, not {value}"))?
            }
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    let out = out.ok_or("--out is required")?;
    let source = source.ok_or("--source is required")?;
    if !source.is_file() {
        return Err(format!("{} is not a file", source.display()));
    }
    let library_dir = out.join("library");
    if library_dir.exists() {
        return Err(format!(
            "{} already exists; use a fresh --out directory",
            library_dir.display()
        ));
    }
    std::fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    let credential = std::env::var("OSTUDY_EVAL_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let request = GuideEvalRequest {
        source,
        library_dir,
        provider: GuideEvalProvider {
            kind: kind.ok_or("--provider is required")?,
            base_url: base_url.ok_or("--base-url is required")?,
            model_id: model_id.ok_or("--model is required")?,
            reasoning_effort,
            credential,
            allow_insecure_http,
            request_timeout_ms,
        },
        resource_dir: None,
    };
    Ok((request, out))
}
