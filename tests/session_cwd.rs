use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}"))
}

fn write_config(path: &Path) {
    let config = r#"{
  "defaults": {
    "idle_timeout_ms": 120000,
    "summary_only": true,
    "max_parallel": 4
  },
  "surface": {
    "cli": "codex",
    "description": "surface",
    "base_args": [],
    "env": {}
  },
  "runtime": {
    "transport": {
      "mode": "direct",
      "preferred": ["stdio", "unix_socket"],
      "allow_tmux_fallback": false
    },
    "loop": {
      "persist_runs": true,
      "resume_strategy": "ledger"
    },
    "memory": {
      "enabled": true,
      "ttl_hours": 24,
      "invalidate_on_git_head_change": true
    },
    "workers": {
      "max_workers": 6,
      "spawn_policy": "persistent",
      "continue_policy": "resume_when_possible"
    }
  },
    "workers": {
    "explore": {
      "cli": "/bin/pwd",
      "model": "test-model",
      "reasoning": "high",
      "description": "explore",
      "delivery_mode": "session",
      "launch_mode": "stdin_text",
      "base_args": [],
      "env": {}
    }
  }
}"#;
    fs::write(path, config).expect("failed to write config");
}

#[test]
fn worker_adapter_spawn_session_preserves_adapter_cwd() {
    let root = unique_temp_dir("conductor-session-cwd");
    let state_dir = root.join(".conductor");
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).expect("failed to create workspace");
    fs::create_dir_all(&state_dir).expect("failed to create state root");
    let config_path = root.join("conductor.json");
    write_config(&config_path);

    let output = Command::new(env!("CARGO_BIN_EXE_conductor"))
        .env("CONDUCTOR_CONFIG", &config_path)
        .env("CONDUCTOR_STATE_DIR", &state_dir)
        .env("CONDUCTOR_ADAPTER_EXPLORE_CWD", &workspace)
        .arg("worker-adapter-spawn-session")
        .arg("explore")
        .arg("demo-run")
        .arg("explore-1")
        .output()
        .expect("failed to run conductor");

    assert!(
        output.status.success(),
        "conductor failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout_log = state_dir
        .join("runs")
        .join("demo-run")
        .join("sessions")
        .join("session-explore-1")
        .join("stdout.log");

    for _ in 0..20 {
        if stdout_log.exists() {
            let rendered = fs::read_to_string(&stdout_log).unwrap_or_default();
            if rendered.contains(workspace.to_string_lossy().as_ref()) {
                return;
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    let rendered = fs::read_to_string(&stdout_log).unwrap_or_default();
    panic!(
        "stdout log did not contain adapter cwd {}\nlog:{}",
        workspace.display(),
        rendered
    );
}
