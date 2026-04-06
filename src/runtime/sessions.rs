use crate::runtime::state_store::StateStore;
use crate::runtime::types::{
    EventEnvelope, EventKind, SCHEMA_VERSION, SessionRecord, SessionStatus, WorkerKind,
    WorkerRecord, WorkerState,
};
use chrono::Utc;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use serde_json::Map;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum SessionCommand {
    SendStdin { data: String },
    SendRaw { data: String },
    Status,
    Stop,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub ok: bool,
    pub status: String,
    pub message: Option<String>,
}

pub struct SessionSpawnResult {
    pub session: SessionRecord,
}

pub fn spawn_session(
    store: &StateStore,
    run_id: &str,
    worker_id: &str,
    program: &str,
    args: &[String],
    child_env: &BTreeMap<String, String>,
    conductor_bin: &Path,
) -> Result<SessionSpawnResult, String> {
    let session_id = format!("session-{worker_id}");
    let session_dir = store.session_dir(run_id, &session_id);
    fs::create_dir_all(&session_dir).map_err(|err| err.to_string())?;
    let socket_root = std::env::temp_dir().join("conductor-kit-sessions");
    fs::create_dir_all(&socket_root).map_err(|err| err.to_string())?;
    let socket_path = socket_root.join(socket_file_name(run_id, worker_id));
    let stdout_path = session_dir.join("stdout.log");
    let stderr_path = session_dir.join("stderr.log");
    let host_stdout_path = session_dir.join("host.stdout.log");
    let host_stderr_path = session_dir.join("host.stderr.log");
    let _ = fs::remove_file(&socket_path);

    let mut command = Command::new(conductor_bin);
    command.arg("worker-host");
    command.arg(run_id);
    command.arg(worker_id);
    command.arg(&session_id);
    command.arg(&socket_path);
    command.arg(&stdout_path);
    command.arg(&stderr_path);
    command.arg(program);
    command.args(args);
    for (key, value) in child_env {
        command.env(format!("CONDUCTOR_CHILD_{key}"), value);
    }
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    command.stdin(Stdio::null());
    command.stdout(File::create(&host_stdout_path).map_err(|err| err.to_string())?);
    command.stderr(File::create(&host_stderr_path).map_err(|err| err.to_string())?);

    let child = command.spawn().map_err(|err| err.to_string())?;
    wait_for_socket(&socket_path)?;

    let now = Utc::now();
    let session = SessionRecord {
        run_id: run_id.to_string(),
        worker_id: worker_id.to_string(),
        session_id: session_id.clone(),
        socket_path: socket_path.display().to_string(),
        stdout_path: stdout_path.display().to_string(),
        stderr_path: stderr_path.display().to_string(),
        pid: child.id(),
        child_pid: None,
        program: program.to_string(),
        args: args.to_vec(),
        status: SessionStatus::Running,
        started_at: now,
        updated_at: now,
        exited_at: None,
        exit_code: None,
    };
    store.write_session(&session)?;
    store.upsert_worker(WorkerRecord {
        worker_id: worker_id.to_string(),
        run_id: run_id.to_string(),
        worker_kind: WorkerKind::Worker,
        session_ref: Some(session_id.clone()),
        state: WorkerState::Idle,
        current_task_id: None,
        current_summary: Some("session running".to_string()),
        terminal_label: Some(worker_id.to_string()),
        last_heartbeat_at: Some(now),
        last_stdout_at: None,
        last_event_at: Some(now),
        reason: None,
    })?;
    store.append_runtime_event(
        run_id,
        EventEnvelope {
            schema_version: SCHEMA_VERSION,
            event: EventKind::WorkerSessionStarted,
            timestamp: now,
            run_id: Some(run_id.to_string()),
            session_id: Some(session_id),
            source: "sessions".to_string(),
            worker: Some(worker_id.to_string()),
            task_id: None,
            message_id: None,
            reason: None,
            context: Map::new(),
        },
    )?;
    store.refresh_snapshot(run_id)?;
    Ok(SessionSpawnResult { session })
}

pub fn send_session_command(
    socket_path: &Path,
    command: &SessionCommand,
) -> Result<SessionResponse, String> {
    let mut stream = UnixStream::connect(socket_path).map_err(|err| err.to_string())?;
    let payload = serde_json::to_vec(command).map_err(|err| err.to_string())?;
    stream.write_all(&payload).map_err(|err| err.to_string())?;
    stream.write_all(b"\n").map_err(|err| err.to_string())?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|err| err.to_string())?;
    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .map_err(|err| err.to_string())?;
    serde_json::from_str(raw.trim()).map_err(|err| err.to_string())
}

pub fn run_worker_host(
    _run_id: &str,
    _worker_id: &str,
    _session_id: &str,
    socket_path: &Path,
    stdout_path: &Path,
    stderr_path: &Path,
    program: &str,
    args: &[String],
) -> Result<(), String> {
    if socket_path.exists() {
        let _ = fs::remove_file(socket_path);
    }
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let stdout_file = File::create(stdout_path).map_err(|err| err.to_string())?;
    let _stderr_file = File::create(stderr_path).map_err(|err| err.to_string())?;

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| err.to_string())?;
    let mut builder = CommandBuilder::new(program);
    builder.args(args);
    for (key, value) in read_child_env() {
        builder.env(key, value);
    }
    let mut child = pair
        .slave
        .spawn_command(builder)
        .map_err(|err| err.to_string())?;
    let writer = pair.master.take_writer().map_err(|err| err.to_string())?;
    let writer = Arc::new(Mutex::new(writer));
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| err.to_string())?;
    pipe_stream_to_file(reader, stdout_file);

    let listener = UnixListener::bind(socket_path).map_err(|err| err.to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|err| err.to_string())?;

    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut raw = String::new();
                stream
                    .read_to_string(&mut raw)
                    .map_err(|err| err.to_string())?;
                let command: SessionCommand =
                    serde_json::from_str(raw.trim()).map_err(|err| err.to_string())?;
                let response = match command {
                    SessionCommand::SendStdin { data } | SessionCommand::SendRaw { data } => {
                        let child_exit_code = try_wait(&mut child)?;
                        if let Some(code) = child_exit_code {
                            SessionResponse {
                                ok: false,
                                status: "exited".to_string(),
                                message: Some(format!("exit_code={code}")),
                            }
                        } else {
                            let mut guard = writer
                                .lock()
                                .map_err(|_| "pty writer poisoned".to_string())?;
                            guard
                                .write_all(data.as_bytes())
                                .and_then(|_| guard.flush())
                                .map_err(|err| err.to_string())?;
                            SessionResponse {
                                ok: true,
                                status: "running".to_string(),
                                message: None,
                            }
                        }
                    }
                    SessionCommand::Status => match try_wait(&mut child)? {
                        Some(code) => SessionResponse {
                            ok: true,
                            status: "exited".to_string(),
                            message: Some(format!("exit_code={code}")),
                        },
                        None => SessionResponse {
                            ok: true,
                            status: "running".to_string(),
                            message: None,
                        },
                    },
                    SessionCommand::Stop => {
                        let _ = child.kill();
                        let _ = child.wait();
                        SessionResponse {
                            ok: true,
                            status: "stopped".to_string(),
                            message: None,
                        }
                    }
                };
                let encoded = serde_json::to_vec(&response).map_err(|err| err.to_string())?;
                stream.write_all(&encoded).map_err(|err| err.to_string())?;
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(err.to_string()),
        }
    }
}

fn pipe_stream_to_file<R: Read + Send + 'static>(reader: R, mut file: File) {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let _ = file.write_all(line.as_bytes());
                    let _ = file.flush();
                }
                Err(_) => break,
            }
        }
    });
}

fn wait_for_socket(socket_path: &Path) -> Result<(), String> {
    for _ in 0..100 {
        if socket_path.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "timed out waiting for socket {}",
        socket_path.display()
    ))
}

fn try_wait(child: &mut Box<dyn portable_pty::Child + Send + Sync>) -> Result<Option<i32>, String> {
    child
        .try_wait()
        .map(|status| status.map(|value| value.exit_code() as i32))
        .map_err(|err| err.to_string())
}

fn read_child_env() -> BTreeMap<String, String> {
    std::env::vars()
        .filter_map(|(key, value)| {
            key.strip_prefix("CONDUCTOR_CHILD_")
                .map(|suffix| (suffix.to_string(), value))
        })
        .collect()
}

fn socket_file_name(run_id: &str, worker_id: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    run_id.hash(&mut hasher);
    worker_id.hash(&mut hasher);
    let hash = hasher.finish();
    format!("ck-{hash:016x}.sock")
}
