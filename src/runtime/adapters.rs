use serde::Serialize;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct WorkerAdapterConfig {
    pub worker_type: String,
    pub cli: String,
    pub model: String,
    pub reasoning: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct WorkerAdapterLaunch {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub stdin_payload: Option<String>,
}

#[derive(Debug, Serialize)]
struct WorkerAdapterPayload<'a> {
    run_id: &'a str,
    worker_id: &'a str,
    worker_type: &'a str,
    task_id: Option<&'a str>,
    prompt: Option<&'a str>,
    cli: &'a str,
    model: &'a str,
    reasoning: Option<&'a str>,
    description: &'a str,
}

pub fn resolve_worker_adapter(
    config: &WorkerAdapterConfig,
    run_id: &str,
    worker_id: &str,
    task_id: Option<&str>,
    prompt: Option<&str>,
) -> Result<WorkerAdapterLaunch, String> {
    let prefix = format!(
        "CONDUCTOR_ADAPTER_{}",
        config.worker_type.replace('-', "_").to_ascii_uppercase()
    );
    let program = env::var(format!("{prefix}_PROGRAM"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| config.cli.clone());
    let args = env::var(format!("{prefix}_ARGS"))
        .ok()
        .map(|value| shell_words(&value))
        .transpose()?
        .unwrap_or_default();
    let cwd = env::var(format!("{prefix}_CWD"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let payload = WorkerAdapterPayload {
        run_id,
        worker_id,
        worker_type: &config.worker_type,
        task_id,
        prompt,
        cli: &config.cli,
        model: &config.model,
        reasoning: config.reasoning.as_deref(),
        description: &config.description,
    };
    let stdin_payload = Some(
        serde_json::to_string(&payload)
            .map_err(|err| err.to_string())
            .map(|value| format!("{value}\n"))?,
    );
    Ok(WorkerAdapterLaunch {
        program,
        args,
        cwd,
        stdin_payload,
    })
}

fn shell_words(input: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in input.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => quote = Some(ch),
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            (None, c) => current.push(c),
        }
    }
    if quote.is_some() {
        return Err("unterminated quoted adapter args".to_string());
    }
    if !current.is_empty() {
        parts.push(current);
    }
    Ok(parts)
}
