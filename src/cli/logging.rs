use serde::Serialize;

pub fn print_help() {
    println!(
        "
conductor <command>

Commands:
  conductor           Open the current run, or initialize it if missing
  start               Start the lead Codex session and open the ops view
  init                Alias for start
  open                Open the ops view for the current or named run
  resume              Alias for open
  attach              Attach to a named worker in the current or named run
  settings            Open the conductor settings editor
  team                Start a configured team: team <count> <profile> [profile...]
  ralph               Start the wider ralph-style orchestration layout
  report              Report a worker finding back to the main conductor run
  ask                 Send a direct follow-up to a worker lane
  accept              Accept a worker lane result and close that lane
  close               Close a worker lane without relaunching it
  relaunch            Relaunch or retry a worker lane with a narrower follow-up
  help                Show this help
  version             Print version
  worker-log          Print recent session log output
  hud-view            Print a compact runtime HUD view
  next                Print the next suggested operator command for a run
  doctor              Validate config

Advanced:
  config-path         Print resolved config path
  status              Print config status payload
  runtime-init        Initialize runtime state for a run
  runtime-snapshot    Print runtime snapshot for a run
  runtime-refresh     Rebuild and persist snapshot for a run
  run-orchestrate     Run a minimal orchestration loop
  run-fanout          Run a multi-worker fan-out loop
  authority-renew     Renew authority lease for a run
  phase-set           Transition run phase
  task-claim          Acquire a task claim
  task-reclaim-expired Reclaim expired task claims
  task-release        Release a task claim
  task-approval       Mark task approval as pending, approved, rejected, or clear
  worker-upsert       Upsert worker state for a run
  worker-spawn-session Start a long-lived worker session host
  worker-adapter-spawn-session Start a configured worker adapter session
  worker-send         Send stdin to a worker session
  worker-send-raw     Send raw bytes to a worker session
  worker-attach       Attach the current terminal to a worker session
  worker-open-terminal Open a worker session in a new terminal window
  worker-session-status Query a worker session
  worker-stop-session Stop a worker session
  hud-open            Open the live HUD in a new terminal window
  ops-open            Open the HUD and worker sessions in one tmux layout
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
