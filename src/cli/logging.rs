use serde::Serialize;

pub fn print_help() {
    println!(
        "
conductor <command>

Commands:
  help                Show this help
  version             Print version
  config-path         Print resolved config path
  status              Print config status payload
  doctor              Validate config
  runtime-init        Initialize runtime state for a run
  runtime-snapshot    Print runtime snapshot for a run
  runtime-refresh     Rebuild and persist snapshot for a run
  run-orchestrate     Run a minimal orchestration loop
  run-fanout          Run a multi-worker fan-out loop
  authority-renew     Renew authority lease for a run
  phase-set           Transition run phase
  task-claim          Acquire a task claim
  task-release        Release a task claim
  worker-upsert       Upsert worker state for a run
  worker-spawn-session Start a long-lived worker session host
  worker-adapter-spawn-session Start a configured worker adapter session
  worker-send         Send stdin to a worker session
  worker-send-raw     Send raw bytes to a worker session
  worker-attach       Attach the current terminal to a worker session
  worker-open-terminal Open a worker session in a new terminal window
  worker-log          Print recent session log output
  worker-session-status Query a worker session
  worker-stop-session Stop a worker session
  dispatch-route      Deliver a queued dispatch to a worker session
  hud-view            Print a compact runtime HUD view
  hud-watch           Continuously render the runtime HUD
  events-list         Print runtime events
  hook-run            Run a hook command against matching events
  task-create         Create a task record
  dispatch-queue      Create a dispatch record
  dispatch-update     Update dispatch status
  mailbox-send        Append a mailbox message
  mailbox-update      Mark mailbox message notified or delivered
"
    );
}

pub fn print_json<T>(value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let rendered = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    println!("{rendered}");
    Ok(())
}
