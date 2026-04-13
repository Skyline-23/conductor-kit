use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const SESSION_START_COMMAND: &str = "conductor codex-hook session-start";
const USER_PROMPT_COMMAND: &str = "conductor codex-hook user-prompt";
const STOP_COMMAND: &str = "conductor codex-hook stop";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HooksFile {
    #[serde(default)]
    hooks: BTreeMap<String, Vec<HookMatcherGroup>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HookMatcherGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    matcher: Option<String>,
    #[serde(default)]
    hooks: Vec<HookHandler>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HookHandler {
    #[serde(rename = "type")]
    kind: String,
    command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    statusMessage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeoutSec: Option<u64>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HooksHealth {
    pub hooks_path: String,
    pub config_path: String,
    pub feature_enabled: bool,
    pub managed_commands_present: bool,
}

pub fn codex_home_root() -> Result<PathBuf, String> {
    if let Ok(codex_home) = env::var("CODEX_HOME") {
        let trimmed = codex_home.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".codex"))
}

pub fn install_managed_codex_hooks() -> Result<serde_json::Value, String> {
    let codex_home = codex_home_root()?;
    let report = install_managed_codex_hooks_at(&codex_home)?;
    Ok(json!({
        "hooks_path": report.hooks_path.display().to_string(),
        "config_path": report.config_path.display().to_string(),
        "feature_updated": report.feature_updated,
        "commands": report.commands,
    }))
}

pub fn uninstall_managed_codex_hooks() -> Result<serde_json::Value, String> {
    let codex_home = codex_home_root()?;
    let report = uninstall_managed_codex_hooks_at(&codex_home)?;
    Ok(json!({
        "hooks_path": report.hooks_path.display().to_string(),
        "config_path": report.config_path.display().to_string(),
        "feature_enabled": report.feature_enabled,
        "removed_commands": report.commands,
    }))
}

pub fn managed_codex_hooks_health() -> Result<HooksHealth, String> {
    let codex_home = codex_home_root()?;
    let hooks_path = codex_home.join("hooks.json");
    let config_path = codex_home.join("config.toml");
    let feature_enabled = codex_hooks_feature_enabled_at(&config_path);
    let managed_commands_present = managed_commands_present_at(&hooks_path)?;
    Ok(HooksHealth {
        hooks_path: hooks_path.display().to_string(),
        config_path: config_path.display().to_string(),
        feature_enabled,
        managed_commands_present,
    })
}

#[derive(Debug)]
struct InstallReport {
    hooks_path: PathBuf,
    config_path: PathBuf,
    feature_updated: bool,
    commands: Vec<String>,
}

#[derive(Debug)]
struct UninstallReport {
    hooks_path: PathBuf,
    config_path: PathBuf,
    feature_enabled: bool,
    commands: Vec<String>,
}

fn install_managed_codex_hooks_at(codex_home: &Path) -> Result<InstallReport, String> {
    fs::create_dir_all(codex_home).map_err(|err| err.to_string())?;
    let hooks_path = codex_home.join("hooks.json");
    let config_path = codex_home.join("config.toml");
    let feature_updated = ensure_codex_hooks_feature_enabled_at(&config_path)?;
    let mut hooks_file = load_hooks_file(&hooks_path)?;
    strip_managed_hooks(&mut hooks_file);
    for (event_name, groups) in managed_hook_groups() {
        hooks_file.hooks.entry(event_name).or_default().extend(groups);
    }
    save_hooks_file(&hooks_path, &hooks_file)?;
    Ok(InstallReport {
        hooks_path,
        config_path,
        feature_updated,
        commands: managed_commands(),
    })
}

fn uninstall_managed_codex_hooks_at(codex_home: &Path) -> Result<UninstallReport, String> {
    let hooks_path = codex_home.join("hooks.json");
    let config_path = codex_home.join("config.toml");
    let mut hooks_file = load_hooks_file(&hooks_path)?;
    strip_managed_hooks(&mut hooks_file);
    if hooks_file.hooks.is_empty() {
        if hooks_path.exists() {
            fs::remove_file(&hooks_path).map_err(|err| err.to_string())?;
        }
    } else {
        save_hooks_file(&hooks_path, &hooks_file)?;
    }
    Ok(UninstallReport {
        hooks_path,
        config_path: config_path.clone(),
        feature_enabled: codex_hooks_feature_enabled_at(&config_path),
        commands: managed_commands(),
    })
}

fn managed_hook_groups() -> BTreeMap<String, Vec<HookMatcherGroup>> {
    BTreeMap::from([
        (
            "SessionStart".to_string(),
            vec![HookMatcherGroup {
                matcher: Some("startup".to_string()),
                hooks: vec![HookHandler {
                    kind: "command".to_string(),
                    command: SESSION_START_COMMAND.to_string(),
                    statusMessage: Some("Conductor: loading loop context".to_string()),
                    timeout: Some(5),
                    timeoutSec: None,
                    extra: BTreeMap::new(),
                }],
                extra: BTreeMap::new(),
            }],
        ),
        (
            "UserPromptSubmit".to_string(),
            vec![HookMatcherGroup {
                matcher: None,
                hooks: vec![HookHandler {
                    kind: "command".to_string(),
                    command: USER_PROMPT_COMMAND.to_string(),
                    statusMessage: Some("Conductor: refreshing loop context".to_string()),
                    timeout: Some(5),
                    timeoutSec: None,
                    extra: BTreeMap::new(),
                }],
                extra: BTreeMap::new(),
            }],
        ),
    ])
}

fn managed_commands() -> Vec<String> {
    vec![
        SESSION_START_COMMAND.to_string(),
        USER_PROMPT_COMMAND.to_string(),
    ]
}

fn known_managed_commands() -> Vec<String> {
    vec![
        SESSION_START_COMMAND.to_string(),
        USER_PROMPT_COMMAND.to_string(),
        STOP_COMMAND.to_string(),
    ]
}

fn is_managed_handler(handler: &HookHandler) -> bool {
    known_managed_commands()
        .iter()
        .any(|command| handler.kind == "command" && handler.command == *command)
}

fn strip_managed_hooks(hooks_file: &mut HooksFile) {
    for groups in hooks_file.hooks.values_mut() {
        groups.retain_mut(|group| {
            group.hooks.retain(|handler| !is_managed_handler(handler));
            !group.hooks.is_empty()
        });
    }
    hooks_file.hooks.retain(|_, groups| !groups.is_empty());
}

fn load_hooks_file(path: &Path) -> Result<HooksFile, String> {
    if !path.exists() {
        return Ok(HooksFile::default());
    }
    let raw = fs::read_to_string(path).map_err(|err| err.to_string())?;
    if raw.trim().is_empty() {
        return Ok(HooksFile::default());
    }
    serde_json::from_str(&raw).map_err(|err| err.to_string())
}

fn save_hooks_file(path: &Path, hooks_file: &HooksFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let rendered = serde_json::to_string_pretty(hooks_file).map_err(|err| err.to_string())?;
    fs::write(path, format!("{rendered}\n")).map_err(|err| err.to_string())
}

fn managed_commands_present_at(path: &Path) -> Result<bool, String> {
    let hooks_file = load_hooks_file(path)?;
    let managed = managed_commands();
    Ok(managed.iter().all(|command| {
        hooks_file.hooks.values().flatten().any(|group| {
            group
                .hooks
                .iter()
                .any(|handler| handler.kind == "command" && handler.command == *command)
        })
    }))
}

fn codex_hooks_feature_enabled_at(config_path: &Path) -> bool {
    let raw = match fs::read_to_string(config_path) {
        Ok(value) => value,
        Err(_) => return false,
    };
    lookup_toml_bool(&raw, "features", "codex_hooks").unwrap_or(false)
}

fn ensure_codex_hooks_feature_enabled_at(config_path: &Path) -> Result<bool, String> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    if lookup_toml_bool(&existing, "features", "codex_hooks") == Some(true) {
        return Ok(false);
    }
    let updated = upsert_toml_bool(&existing, "features", "codex_hooks", true);
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(config_path, updated).map_err(|err| err.to_string())?;
    Ok(true)
}

fn lookup_toml_bool(raw: &str, section_name: &str, key: &str) -> Option<bool> {
    let section_header = format!("[{section_name}]");
    let mut in_section = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == section_header;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((candidate_key, candidate_value)) = trimmed.split_once('=') else {
            continue;
        };
        if candidate_key.trim() != key {
            continue;
        }
        return match candidate_value.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
    }
    None
}

fn upsert_toml_bool(raw: &str, section_name: &str, key: &str, value: bool) -> String {
    let mut lines = raw.lines().map(|line| line.to_string()).collect::<Vec<_>>();
    let section_header = format!("[{section_name}]");
    let desired_line = format!("{key} = {}", if value { "true" } else { "false" });
    let mut section_start = None;
    let mut section_end = lines.len();

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
            continue;
        }
        if section_start.is_some() {
            section_end = index;
            break;
        }
        if trimmed == section_header {
            section_start = Some(index);
        }
    }

    if let Some(start) = section_start {
        let mut key_index = None;
        for (offset, line) in lines.iter().enumerate().take(section_end).skip(start + 1) {
            let trimmed = line.trim();
            let Some((candidate_key, _)) = trimmed.split_once('=') else {
                continue;
            };
            if candidate_key.trim() == key {
                key_index = Some(offset);
                break;
            }
        }
        if let Some(index) = key_index {
            lines[index] = desired_line;
        } else {
            lines.insert(section_end, desired_line);
        }
    } else {
        if !lines.is_empty() && !lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push(section_header);
        lines.push(desired_line);
    }

    let mut rendered = lines.join("\n");
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock drift")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }

    #[test]
    fn install_merges_managed_hooks_without_dropping_existing_entries() {
        let codex_home = unique_temp_dir("conductor-hooks-install");
        fs::create_dir_all(&codex_home).expect("failed to create codex home");
        let hooks_path = codex_home.join("hooks.json");
        fs::write(
            &hooks_path,
            r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "echo keep-me"
          }
        ]
      }
    ]
  }
}
"#,
        )
        .expect("failed to seed hooks.json");

        install_managed_codex_hooks_at(&codex_home).expect("install should succeed");

        let hooks_file = load_hooks_file(&hooks_path).expect("failed to reload hooks");
        assert!(hooks_file
            .hooks
            .get("SessionStart")
            .expect("missing SessionStart")
            .iter()
            .flat_map(|group| group.hooks.iter())
            .any(|handler| handler.command == "echo keep-me"));
        assert!(managed_commands_present_at(&hooks_path).expect("failed to inspect hooks"));
        assert!(codex_hooks_feature_enabled_at(&codex_home.join("config.toml")));
    }

    #[test]
    fn uninstall_removes_only_managed_hook_commands() {
        let codex_home = unique_temp_dir("conductor-hooks-uninstall");
        fs::create_dir_all(&codex_home).expect("failed to create codex home");
        let hooks_path = codex_home.join("hooks.json");
        save_hooks_file(
            &hooks_path,
            &HooksFile {
                hooks: BTreeMap::from([
                    (
                        "SessionStart".to_string(),
                        vec![HookMatcherGroup {
                            matcher: Some("startup".to_string()),
                            hooks: vec![
                                HookHandler {
                                    kind: "command".to_string(),
                                    command: "echo keep-me".to_string(),
                                    statusMessage: None,
                                    timeout: None,
                                    timeoutSec: None,
                                    extra: BTreeMap::new(),
                                },
                                HookHandler {
                                    kind: "command".to_string(),
                                    command: SESSION_START_COMMAND.to_string(),
                                    statusMessage: None,
                                    timeout: None,
                                    timeoutSec: None,
                                    extra: BTreeMap::new(),
                                },
                            ],
                            extra: BTreeMap::new(),
                        }],
                    ),
                ]),
            },
        )
        .expect("failed to write hooks");

        uninstall_managed_codex_hooks_at(&codex_home).expect("uninstall should succeed");

        let hooks_file = load_hooks_file(&hooks_path).expect("failed to reload hooks");
        let session_start = hooks_file
            .hooks
            .get("SessionStart")
            .expect("missing SessionStart after uninstall");
        assert_eq!(session_start.len(), 1);
        assert_eq!(session_start[0].hooks.len(), 1);
        assert_eq!(session_start[0].hooks[0].command, "echo keep-me");
        assert!(!hooks_file.hooks.contains_key("Stop"));
    }

    #[test]
    fn ensure_feature_flag_rewrites_false_value() {
        let codex_home = unique_temp_dir("conductor-hooks-feature");
        fs::create_dir_all(&codex_home).expect("failed to create codex home");
        let config_path = codex_home.join("config.toml");
        fs::write(
            &config_path,
            "[model]\nname = \"gpt-5.4\"\n\n[features]\ncodex_hooks = false\n",
        )
        .expect("failed to write config");

        let updated =
            ensure_codex_hooks_feature_enabled_at(&config_path).expect("feature update failed");
        let rendered = fs::read_to_string(&config_path).expect("failed to reread config");

        assert!(updated);
        assert!(rendered.contains("[features]"));
        assert!(rendered.contains("codex_hooks = true"));
    }
}
