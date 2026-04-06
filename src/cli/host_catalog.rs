use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VendorCatalog {
    pub default_model: Option<String>,
    pub models: Vec<String>,
    pub reasoning_levels: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCatalog {
    pub codex: VendorCatalog,
    pub claude: VendorCatalog,
    pub gemini: VendorCatalog,
}

impl HostCatalog {
    pub fn vendor(&self, cli: &str) -> VendorCatalog {
        match cli {
            "codex" => self.codex.clone(),
            "claude" => self.claude.clone(),
            "gemini" => self.gemini.clone(),
            _ => VendorCatalog::default(),
        }
    }
}

pub fn catalog_path(state_root: &Path) -> PathBuf {
    state_root.join("host_catalog.json")
}

pub fn load_or_refresh_host_catalog(state_root: &Path) -> HostCatalog {
    let refreshed = detect_host_catalog();
    let path = catalog_path(state_root);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(rendered) = serde_json::to_string_pretty(&refreshed) {
        let _ = fs::write(&path, format!("{rendered}\n"));
    }
    refreshed
}

pub fn detect_host_catalog() -> HostCatalog {
    let home = env::var("HOME").ok();
    let codex = detect_codex_catalog(home.as_deref());
    let claude = detect_claude_catalog();
    let gemini = detect_gemini_catalog();
    HostCatalog {
        codex,
        claude,
        gemini,
    }
}

fn detect_codex_catalog(home: Option<&str>) -> VendorCatalog {
    let mut catalog = VendorCatalog::default();
    let Some(home) = home else {
        return catalog;
    };
    let config_path = Path::new(home).join(".codex").join("config.toml");
    catalog.default_model = read_codex_host_model(&config_path);

    let cache_path = Path::new(home).join(".codex").join("models_cache.json");
    let Ok(raw) = fs::read_to_string(cache_path) else {
        return catalog;
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return catalog;
    };
    let Some(models) = parsed["models"].as_array() else {
        return catalog;
    };

    for model in models {
        let Some(slug) = model["slug"].as_str().map(ToOwned::to_owned) else {
            continue;
        };
        if slug.trim().is_empty() {
            continue;
        }
        push_unique(&mut catalog.models, slug.clone());
        let levels = model["supported_reasoning_levels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|level| level["effort"].as_str().map(ToOwned::to_owned))
            .collect::<Vec<_>>();
        if !levels.is_empty() {
            catalog
                .reasoning_levels
                .insert(slug, normalize_reasoning_order(levels));
        }
    }

    if catalog.default_model.is_none() {
        catalog.default_model = catalog.models.first().cloned();
    }
    catalog
}

fn detect_claude_catalog() -> VendorCatalog {
    let mut catalog = VendorCatalog::default();
    let Some(output) = command_stdout("claude", &["--help"]) else {
        return catalog;
    };

    let effort_levels = parse_choice_list(&output, "--effort <level>");
    let model_examples = parse_quoted_tokens(&output, "--model <model>");
    for model in model_examples {
        if model_matches_cli("claude", &model) {
            push_unique(&mut catalog.models, model);
        }
    }
    catalog.default_model = catalog.models.first().cloned();
    for model in catalog.models.clone() {
        if !effort_levels.is_empty() {
            catalog
                .reasoning_levels
                .insert(model, normalize_reasoning_order(effort_levels.clone()));
        }
    }
    catalog
}

fn detect_gemini_catalog() -> VendorCatalog {
    let mut catalog = VendorCatalog::default();
    let Some(output) = command_stdout("gemini", &["--help"]) else {
        return catalog;
    };

    let models = parse_choice_list(&output, "-m, --model");
    for model in models {
        if model_matches_cli("gemini", &model) {
            push_unique(&mut catalog.models, model);
        }
    }
    if catalog.models.is_empty() {
        let quoted = parse_quoted_tokens(&output, "-m, --model");
        for model in quoted {
            if model_matches_cli("gemini", &model) {
                push_unique(&mut catalog.models, model);
            }
        }
    }
    catalog.default_model = catalog.models.first().cloned();
    catalog
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_choice_list(help: &str, marker: &str) -> Vec<String> {
    let mut values = Vec::new();
    let Some(line) = help.lines().find(|line| line.contains(marker)) else {
        return values;
    };
    let cleaned = if let Some(open) = line.rfind('(') {
        if let Some(close) = line[open + 1..].find(')') {
            line[open + 1..open + 1 + close].to_string()
        } else {
            String::new()
        }
    } else if let Some((_, tail)) = line.rsplit_once("choices:") {
        tail.trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .replace('"', "")
    } else {
        String::new()
    };
    if cleaned.is_empty() {
        return values;
    }
    for token in cleaned.split(',') {
        let value = token.trim();
        if !value.is_empty() {
            push_unique(&mut values, value.to_string());
        }
    }
    values
}

fn parse_quoted_tokens(help: &str, marker: &str) -> Vec<String> {
    let mut values = Vec::new();
    if !help.contains(marker) {
        return values;
    }
    let mut in_quote = false;
    let mut current = String::new();
    for ch in help.chars() {
        if ch == '\'' {
            if in_quote && !current.trim().is_empty() {
                push_unique(&mut values, current.trim().to_string());
                current.clear();
            }
            in_quote = !in_quote;
            continue;
        }
        if in_quote {
            current.push(ch);
        }
    }
    values
}

pub fn normalize_reasoning_order(values: Vec<String>) -> Vec<String> {
    let preferred = ["low", "medium", "high", "xhigh", "max"];
    let mut ordered = Vec::new();
    if values.iter().any(|candidate| candidate == "-") {
        ordered.push("-".to_string());
    }
    for value in preferred {
        if values.iter().any(|candidate| candidate == value) {
            ordered.push(value.to_string());
        }
    }
    for value in values {
        if !ordered.iter().any(|candidate| candidate == &value) {
            ordered.push(value);
        }
    }
    ordered
}

pub fn model_matches_cli(cli: &str, model: &str) -> bool {
    let model = model.trim().to_ascii_lowercase();
    match cli {
        "codex" => {
            model.starts_with("gpt-")
                || model.starts_with("o3")
                || model.starts_with("o4")
                || model.starts_with("codex")
        }
        "claude" => {
            model.starts_with("claude") || model == "sonnet" || model == "opus" || model == "haiku"
        }
        "gemini" => model.starts_with("gemini"),
        _ => false,
    }
}

pub fn preferred_model_for_cli(catalog: &HostCatalog, cli: &str) -> Option<String> {
    let vendor = catalog.vendor(cli);
    vendor
        .default_model
        .or_else(|| vendor.models.first().cloned())
}

pub fn reasoning_levels_for(catalog: &HostCatalog, cli: &str, model: Option<&str>) -> Vec<String> {
    let vendor = catalog.vendor(cli);
    if let Some(model) = model {
        if let Some(levels) = vendor.reasoning_levels.get(model) {
            return normalize_reasoning_order(levels.clone());
        }
    }
    let mut merged = Vec::new();
    for levels in vendor.reasoning_levels.values() {
        for value in levels {
            push_unique(&mut merged, value.to_string());
        }
    }
    normalize_reasoning_order(merged)
}

fn read_codex_host_model(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    raw.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("model =") {
            return None;
        }
        let value = trimmed.split_once('=')?.1.trim();
        Some(value.trim_matches('"').to_string()).filter(|model| !model.is_empty())
    })
}

fn push_unique(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if value.trim().is_empty() || values.iter().any(|existing| existing == &value) {
        return;
    }
    values.push(value);
}
