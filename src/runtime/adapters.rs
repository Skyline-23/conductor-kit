use serde::Serialize;
use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct WorkerAdapterConfig {
    pub worker_type: String,
    pub cli: String,
    pub model: String,
    pub reasoning: Option<String>,
    pub description: String,
    pub launch_mode: String,
    pub base_args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct WorkerAdapterLaunch {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub stdin_payload: Option<String>,
    pub env: BTreeMap<String, String>,
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
    let override_args = env::var(format!("{prefix}_ARGS"))
        .ok()
        .map(|value| shell_words(&value))
        .transpose()?
        .unwrap_or_default();
    let mut args = if override_args.is_empty() {
        config.base_args.clone()
    } else {
        override_args
    };
    let cwd = env::var(format!("{prefix}_CWD"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let launch_mode = env::var(format!("{prefix}_MODE"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| config.launch_mode.clone());
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
    let payload_json = serde_json::to_string(&payload).map_err(|err| err.to_string())?;
    let stdin_payload = match launch_mode.as_str() {
        "stdin_json" => Some(format!("{payload_json}\n")),
        "stdin_text" => Some(format!("{}\n", prompt.unwrap_or(""))),
        "argv_prompt" => {
            args.push(prompt.unwrap_or("").to_string());
            None
        }
        "argv_json" => {
            args.push(payload_json.clone());
            None
        }
        other => return Err(format!("unsupported adapter launch mode: {other}")),
    };
    let mut launch_env = config.env.clone();
    launch_env.insert("CONDUCTOR_RUN_ID".to_string(), run_id.to_string());
    launch_env.insert("CONDUCTOR_WORKER_ID".to_string(), worker_id.to_string());
    launch_env.insert(
        "CONDUCTOR_WORKER_TYPE".to_string(),
        config.worker_type.clone(),
    );
    launch_env.insert("CONDUCTOR_MODEL".to_string(), config.model.clone());
    launch_env.insert("CONDUCTOR_CLI".to_string(), config.cli.clone());
    if let Some(task_id) = task_id {
        launch_env.insert("CONDUCTOR_TASK_ID".to_string(), task_id.to_string());
    }
    Ok(WorkerAdapterLaunch {
        program,
        args,
        cwd,
        stdin_payload,
        env: launch_env,
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
