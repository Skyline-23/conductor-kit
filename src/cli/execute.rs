use crate::cli::args::{
    Config, StatusPayload, command_available, load_resolved_config, parse_dispatch_status,
    parse_run_phase, parse_worker_state, required_arg, resolve_config_path, resolve_state_root,
    save_config,
};
use crate::cli::host_catalog::{
    HostCatalog, load_or_refresh_host_catalog, model_matches_cli, normalize_reasoning_order,
    preferred_model_for_cli, reasoning_levels_for,
};
use crate::cli::logging::{print_help, print_json};
use crate::runtime::adapters::{WorkerAdapterConfig, resolve_worker_adapter};
use crate::runtime::authority::renew_authority;
use crate::runtime::claims::{acquire_claim, reclaim_expired_claims, release_claim};
use crate::runtime::hooks::{event_name_of, filter_events, watch_and_run_hooks};
use crate::runtime::phases::transition_phase;
use crate::runtime::sessions::{
    SessionCommand, run_worker_host, send_session_command, spawn_session,
};
use crate::runtime::state_store::StateStore;
use crate::runtime::types::{
    ApprovalStatus, DispatchStatus, EventEnvelope, EventKind, RunPhase, SCHEMA_VERSION,
    SessionStatus, TaskRecord, WorkerKind, WorkerRecord, WorkerState,
};
use crate::runtime::workers::{WorkerLaunchSpec, execute_worker};
use chrono::Utc;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    Clear as TerminalClear, ClearType as TerminalClearType, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::fs::File;
use std::io::{IsTerminal, Stdout, stdin, stdout};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

pub fn execute_command(args: &[String]) -> Result<(), String> {
    let cmd = args.get(0).map(String::as_str).unwrap_or("");

    match cmd {
        "" => run_default(),
        "install" => run_install(&args[1..]),
        "uninstall" => run_uninstall(&args[1..]),
        "sync-skills" => run_sync_skills(&args[1..]),
        "autoresearch" => run_autoresearch(&args[1..]),
        "init" => run_start(&args[1..]),
        "resume" => run_open(&args[1..]),
        "team" => run_team(&args[1..]),
        "team-nudge" => run_team_nudge(&args[1..]),
        "team-followup" => run_team_followup(&args[1..]),
        "ralph" => run_ralph(&args[1..]),
        "ralph-watch" => run_ralph_watch(&args[1..]),
        "report" => run_report(&args[1..]),
        "ask" => run_ask(&args[1..]),
        "handoff" => run_handoff(&args[1..]),
        "accept" => run_accept(&args[1..]),
        "close" => run_close(&args[1..]),
        "relaunch" => run_relaunch(&args[1..]),
        "start" => run_start(&args[1..]),
        "open" => run_open(&args[1..]),
        "attach" => run_attach_alias(&args[1..]),
        "settings" => run_settings(),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        "version" | "-v" | "--version" => {
            println!("conductor {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "config-path" => run_config_path(),
        "status" => run_status(),
        "doctor" => run_doctor(),
        "runtime-init" => run_runtime_init(&args[1..]),
        "runtime-snapshot" => run_runtime_snapshot(&args[1..]),
        "runtime-refresh" => run_runtime_refresh(&args[1..]),
        "run-orchestrate" => run_orchestrate(&args[1..]),
        "run-fanout" => run_fanout(&args[1..]),
        "authority-renew" => run_authority_renew(&args[1..]),
        "phase-set" => run_phase_set(&args[1..]),
        "task-claim" => run_task_claim(&args[1..]),
        "task-reclaim-expired" => run_task_reclaim_expired(&args[1..]),
        "task-release" => run_task_release(&args[1..]),
        "task-approval" => run_task_approval(&args[1..]),
        "worker-upsert" => run_worker_upsert(&args[1..]),
        "worker-exec" => run_worker_exec(&args[1..]),
        "worker-spawn-session" => run_worker_spawn_session(&args[1..]),
        "worker-adapter-exec" => run_worker_adapter_exec(&args[1..]),
        "worker-adapter-spawn-session" => run_worker_adapter_spawn_session(&args[1..]),
        "worker-send" => run_worker_send(&args[1..]),
        "worker-send-raw" => run_worker_send_raw(&args[1..]),
        "worker-attach" => run_worker_attach(&args[1..]),
        "worker-open-terminal" => run_worker_open_terminal(&args[1..]),
        "worker-log" => run_worker_log(&args[1..]),
        "worker-session-status" => run_worker_session_status(&args[1..]),
        "worker-stop-session" => run_worker_stop_session(&args[1..]),
        "hud-open" => run_hud_open(&args[1..]),
        "ops-open" => run_ops_open(&args[1..]),
        "next" => run_next(&args[1..]),
        "worker-host" => run_worker_host_command(&args[1..]),
        "dispatch-route" => run_dispatch_route(&args[1..]),
        "hud-view" => run_hud_view(&args[1..]),
        "hud-watch" => run_hud_watch(&args[1..]),
        "inbox" => run_inbox(&args[1..]),
        "hud-strip-once" => run_hud_strip_once(&args[1..]),
        "hud-strip-watch" => run_hud_strip_watch(&args[1..]),
        "events-list" => run_events_list(&args[1..]),
        "hook-run" => run_hook_run(&args[1..]),
        "task-create" => run_task_create(&args[1..]),
        "dispatch-queue" => run_dispatch_queue(&args[1..]),
        "dispatch-update" => run_dispatch_update(&args[1..]),
        "mailbox-send" => run_mailbox_send(&args[1..]),
        "mailbox-update" => run_mailbox_update(&args[1..]),
        _ => {
            print_help();
            Err("unknown command".to_string())
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TeamMode {
    Default,
    Ralph,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DefaultEntryAction {
    Start,
    AttachSurface,
}

#[derive(Clone, Copy)]
enum SettingsField {
    SurfaceCli,
    Cli,
    Model,
    Reasoning,
    Description,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsDepth {
    Entries,
    Fields,
    Choices,
    Text,
}

#[derive(Clone)]
enum SettingsEntry {
    Surface,
    Profile(String),
}

#[derive(Clone)]
struct SettingsChoice {
    label: String,
    value: String,
}

struct SettingsApp {
    config_path: PathBuf,
    cfg: Config,
    entries: Vec<SettingsEntry>,
    selected_entry: usize,
    selected_field: usize,
    depth: SettingsDepth,
    choices: Vec<SettingsChoice>,
    selected_choice: usize,
    pending_choice: Option<String>,
    input: String,
    status: String,
    host_catalog: HostCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutoresearchConfig {
    schema_version: u32,
    run_id: String,
    repo_root: String,
    branch: String,
    goal: String,
    metric_command: String,
    metric_regex: String,
    metric_direction: String,
    in_scope_files: Vec<String>,
    out_of_scope_files: Vec<String>,
    constraints: Vec<String>,
    max_experiments: Option<usize>,
    simplicity_policy: String,
    baseline_metric: f64,
    best_metric: f64,
    baseline_commit: String,
    best_commit: String,
    experiment_count: usize,
    started_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    stopped_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct AutoresearchSetupArgs {
    run_id: String,
    goal: String,
    metric_command: String,
    metric_regex: String,
    metric_direction: MetricDirection,
    in_scope_files: Vec<String>,
    out_of_scope_files: Vec<String>,
    constraints: Vec<String>,
    max_experiments: Option<usize>,
    simplicity_policy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricDirection {
    LowerIsBetter,
    HigherIsBetter,
}

impl MetricDirection {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "lower" | "lower_is_better" => Ok(Self::LowerIsBetter),
            "higher" | "higher_is_better" => Ok(Self::HigherIsBetter),
            _ => Err("--direction must be lower or higher".to_string()),
        }
    }

    fn from_stored(value: &str) -> Result<Self, String> {
        Self::parse(value)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::LowerIsBetter => "lower",
            Self::HigherIsBetter => "higher",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ExperimentRow {
    experiment: usize,
    commit: String,
    metric: String,
    status: String,
    description: String,
}

fn run_default() -> Result<(), String> {
    let run_id = default_run_id();
    let store = StateStore::new(resolve_state_root()?);
    let run_exists = store
        .root()
        .join("runs")
        .join(&run_id)
        .join("run.json")
        .exists();
    if run_exists {
        cleanup_legacy_ops_session_if_solo(&store, &run_id)?;
    }
    let surface_exists = tmux_session_exists(&surface_tmux_session_name(&run_id)).unwrap_or(false);
    match decide_default_entry_action(run_exists, surface_exists) {
        DefaultEntryAction::Start => run_start(&[]),
        DefaultEntryAction::AttachSurface => {
            attach_tmux_ops_session(&surface_tmux_session_name(&run_id))
        }
    }
}

fn decide_default_entry_action(run_exists: bool, surface_exists: bool) -> DefaultEntryAction {
    if surface_exists {
        DefaultEntryAction::AttachSurface
    } else {
        let _ = run_exists;
        DefaultEntryAction::Start
    }
}

fn run_settings() -> Result<(), String> {
    let (config_path, cfg) = load_resolved_config()?;
    let state_root = resolve_state_root()?;
    let host_catalog = load_or_refresh_host_catalog(&state_root);
    if !stdout().is_terminal() {
        return Err("settings requires an interactive terminal".to_string());
    }
    let mut app = SettingsApp {
        config_path,
        cfg,
        entries: Vec::new(),
        selected_entry: 0,
        selected_field: 0,
        depth: SettingsDepth::Entries,
        choices: Vec::new(),
        selected_choice: 0,
        pending_choice: None,
        input: String::new(),
        status: "Enter drills in. Space selects. Enter saves. Esc backs out. q quits.".to_string(),
        host_catalog,
    };
    normalize_loaded_profiles(&mut app);
    app.entries = settings_entries(&app.cfg);
    run_settings_tui(&mut app)
}

fn run_start(args: &[String]) -> Result<(), String> {
    let run_id = args
        .first()
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_run_id);
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, &run_id)?;
    cleanup_legacy_ops_session_if_solo(&store, &run_id)?;
    run_surface_ops_open(&run_id, false)
}

fn update_profile_field(
    cfg: &mut Config,
    profile_name: &str,
    field: SettingsField,
    value: &str,
) -> Result<(), String> {
    let profile = cfg
        .workers
        .get_mut(profile_name)
        .ok_or_else(|| format!("missing profile: {profile_name}"))?;
    match field {
        SettingsField::SurfaceCli => {
            return Err("surface cli must be edited from the surface menu".to_string());
        }
        SettingsField::Cli => {
            if value.trim().is_empty() {
                return Err("cli must not be empty".to_string());
            }
            profile.cli = value.trim().to_string();
        }
        SettingsField::Model => {
            if value.trim().is_empty() {
                return Err("model must not be empty".to_string());
            }
            profile.model = value.trim().to_string();
        }
        SettingsField::Reasoning => {
            if value.trim() == "-" || value.trim().is_empty() {
                profile.reasoning = None;
            } else {
                profile.reasoning = Some(value.trim().to_string());
            }
        }
        SettingsField::Description => {
            if value.trim().is_empty() {
                return Err("description must not be empty".to_string());
            }
            profile.description = value.trim().to_string();
        }
    }
    Ok(())
}

fn sync_profile_for_cli(app: &mut SettingsApp, profile_name: &str) {
    let Some(profile) = app.cfg.workers.get_mut(profile_name) else {
        return;
    };
    if !model_matches_cli(&profile.cli, &profile.model) {
        if let Some(model) = preferred_model_for_cli(&app.host_catalog, &profile.cli) {
            profile.model = model;
        } else {
            profile.model.clear();
        }
    }
    if let Some(reasoning) = profile.reasoning.clone() {
        let available = reasoning_levels_for(&app.host_catalog, &profile.cli, Some(&profile.model));
        if !available.is_empty() && !available.iter().any(|level| level == &reasoning) {
            profile.reasoning = available.first().cloned();
        }
    }
}

fn normalize_loaded_profiles(app: &mut SettingsApp) {
    let profile_names = app.cfg.workers.keys().cloned().collect::<Vec<_>>();
    let mut changed = false;
    for profile_name in profile_names {
        let before = app.cfg.workers.get(&profile_name).map(|worker| {
            (
                worker.cli.clone(),
                worker.model.clone(),
                worker.reasoning.clone(),
                worker.description.clone(),
            )
        });
        sync_profile_for_cli(app, &profile_name);
        let after = app.cfg.workers.get(&profile_name).map(|worker| {
            (
                worker.cli.clone(),
                worker.model.clone(),
                worker.reasoning.clone(),
                worker.description.clone(),
            )
        });
        if before != after {
            changed = true;
        }
    }
    if changed {
        let _ = save_config(&app.config_path, &app.cfg);
        app.status = format!(
            "Normalized mismatched model selections in {}.",
            app.config_path.display()
        );
    }
}

fn settings_entries(cfg: &Config) -> Vec<SettingsEntry> {
    let mut entries = vec![SettingsEntry::Surface];
    entries.extend(cfg.workers.keys().cloned().map(SettingsEntry::Profile));
    entries
}

fn selected_entry<'a>(app: &'a SettingsApp) -> &'a SettingsEntry {
    &app.entries[app.selected_entry]
}

fn entry_label(entry: &SettingsEntry) -> String {
    match entry {
        SettingsEntry::Surface => "surface".to_string(),
        SettingsEntry::Profile(name) => name.clone(),
    }
}

fn entry_fields(app: &SettingsApp) -> Vec<(SettingsField, String, String)> {
    match selected_entry(app) {
        SettingsEntry::Surface => vec![
            (
                SettingsField::SurfaceCli,
                "cli".to_string(),
                app.cfg.surface.cli.clone(),
            ),
            (
                SettingsField::Description,
                "description".to_string(),
                app.cfg.surface.description.clone(),
            ),
        ],
        SettingsEntry::Profile(name) => {
            let profile = app
                .cfg
                .workers
                .get(name)
                .expect("settings entry missing profile");
            vec![
                (SettingsField::Cli, "cli".to_string(), profile.cli.clone()),
                (
                    SettingsField::Model,
                    "model".to_string(),
                    profile.model.clone(),
                ),
                (
                    SettingsField::Reasoning,
                    "reasoning".to_string(),
                    profile.reasoning.clone().unwrap_or_else(|| "-".to_string()),
                ),
                (
                    SettingsField::Description,
                    "description".to_string(),
                    profile.description.clone(),
                ),
            ]
        }
    }
}

fn normalize_selected_field(app: &mut SettingsApp) {
    let field_count = entry_fields(app).len();
    if field_count == 0 {
        app.selected_field = 0;
    } else if app.selected_field >= field_count {
        app.selected_field = field_count - 1;
    }
}

fn current_field(app: &SettingsApp) -> (SettingsField, String, String) {
    let fields = entry_fields(app);
    fields.get(app.selected_field).cloned().unwrap_or((
        SettingsField::Description,
        String::new(),
        String::new(),
    ))
}

fn edit_hint(field: SettingsField) -> &'static str {
    match field {
        SettingsField::Reasoning => "Select a reasoning value, or use - to clear it.",
        SettingsField::SurfaceCli | SettingsField::Cli => "Use the installed CLI name to launch.",
        SettingsField::Model => "Set the exact model name for this profile.",
        SettingsField::Description => "Use a short operator-facing description.",
    }
}

fn begin_settings_text_edit(app: &mut SettingsApp) {
    let (_, _, value) = current_field(app);
    app.input = value;
    app.depth = SettingsDepth::Text;
    app.status = "Editing text. Enter saves. Esc cancels.".to_string();
}

fn apply_settings_value(app: &mut SettingsApp, value: &str) -> Result<(), String> {
    let (field, label, _) = current_field(app);
    match selected_entry(app).clone() {
        SettingsEntry::Surface => update_surface_field(&mut app.cfg, field, value)?,
        SettingsEntry::Profile(name) => {
            update_profile_field(&mut app.cfg, &name, field, value)?;
            if matches!(field, SettingsField::Cli) {
                sync_profile_for_cli(app, &name);
            }
        }
    }
    save_config(&app.config_path, &app.cfg)?;
    app.entries = settings_entries(&app.cfg);
    normalize_selected_field(app);
    app.depth = SettingsDepth::Fields;
    app.choices.clear();
    app.selected_choice = 0;
    app.pending_choice = None;
    app.input.clear();
    app.status = format!("Saved {label} to {}", app.config_path.display());
    Ok(())
}

fn apply_settings_text(app: &mut SettingsApp) -> Result<(), String> {
    let value = app.input.trim().to_string();
    apply_settings_value(app, &value)
}

fn current_cli_for_entry(app: &SettingsApp) -> String {
    match selected_entry(app) {
        SettingsEntry::Surface => app.cfg.surface.cli.clone(),
        SettingsEntry::Profile(name) => app
            .cfg
            .workers
            .get(name)
            .map(|worker| worker.cli.clone())
            .unwrap_or_else(|| app.cfg.surface.cli.clone()),
    }
}

fn cli_choices(current_value: &str) -> Vec<SettingsChoice> {
    let mut values = BTreeSet::new();
    for cli in ["codex", "claude", "gemini"] {
        if command_available(cli) {
            values.insert(cli.to_string());
        }
    }
    if !current_value.trim().is_empty() {
        values.insert(current_value.trim().to_string());
    }
    values
        .into_iter()
        .map(|value| SettingsChoice {
            label: value.clone(),
            value,
        })
        .collect()
}

fn model_choices(app: &SettingsApp, current_value: &str) -> Vec<SettingsChoice> {
    let cli = current_cli_for_entry(app);
    let vendor = app.host_catalog.vendor(&cli);
    let mut values = vendor.models;
    if model_matches_cli(&cli, current_value) {
        if !values
            .iter()
            .any(|existing| existing == current_value.trim())
        {
            values.insert(0, current_value.trim().to_string());
        }
    }
    if let Some(default_model) = vendor.default_model {
        if !values.iter().any(|existing| existing == &default_model) {
            values.insert(0, default_model);
        }
    }
    values
        .into_iter()
        .map(|value| SettingsChoice {
            label: value.clone(),
            value,
        })
        .collect()
}

fn reasoning_choices(app: &SettingsApp, current_value: &str) -> Vec<SettingsChoice> {
    let cli = current_cli_for_entry(app);
    let mut values = Vec::new();
    values.push("-".to_string());
    let model = match selected_entry(app) {
        SettingsEntry::Surface => None,
        SettingsEntry::Profile(name) => app
            .cfg
            .workers
            .get(name)
            .map(|worker| worker.model.as_str()),
    };
    for value in reasoning_levels_for(&app.host_catalog, &cli, model) {
        if !values.iter().any(|existing| existing == &value) {
            values.push(value);
        }
    }
    if !current_value.trim().is_empty()
        && !values
            .iter()
            .any(|existing| existing == current_value.trim())
    {
        values.push(current_value.trim().to_string());
    }
    let values = normalize_reasoning_order(values);
    values
        .into_iter()
        .map(|value| SettingsChoice {
            label: value.clone(),
            value,
        })
        .collect()
}

fn choice_options(app: &SettingsApp) -> Vec<SettingsChoice> {
    let (field, _, current_value) = current_field(app);
    match field {
        SettingsField::SurfaceCli | SettingsField::Cli => cli_choices(&current_value),
        SettingsField::Model => model_choices(app, &current_value),
        SettingsField::Reasoning => reasoning_choices(app, &current_value),
        SettingsField::Description => Vec::new(),
    }
}

fn settings_panel_style() -> Style {
    Style::default()
        .bg(Color::Rgb(17, 19, 33))
        .fg(Color::Rgb(231, 234, 243))
}

fn settings_muted_style() -> Style {
    Style::default().fg(Color::Rgb(145, 151, 174))
}

fn settings_accent_style() -> Style {
    Style::default()
        .fg(Color::Rgb(124, 242, 203))
        .add_modifier(Modifier::BOLD)
}

fn settings_focus_style(active: bool) -> Style {
    if active {
        Style::default()
            .bg(Color::Rgb(49, 56, 92))
            .fg(Color::Rgb(245, 247, 250))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Rgb(214, 218, 230))
            .add_modifier(Modifier::BOLD)
    }
}

fn host_default_spans(catalog: &HostCatalog) -> Vec<Span<'static>> {
    let codex = catalog
        .codex
        .default_model
        .clone()
        .unwrap_or_else(|| "-".to_string());
    let claude = catalog
        .claude
        .default_model
        .clone()
        .unwrap_or_else(|| "-".to_string());
    let gemini = catalog
        .gemini
        .default_model
        .clone()
        .unwrap_or_else(|| "-".to_string());

    vec![
        Span::styled(
            " host defaults ",
            Style::default()
                .fg(Color::Rgb(12, 14, 24))
                .bg(Color::Rgb(124, 242, 203))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            "codex",
            Style::default()
                .fg(Color::Rgb(120, 196, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" {codex}   ")),
        Span::styled(
            "claude",
            Style::default()
                .fg(Color::Rgb(255, 191, 114))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" {claude}   ")),
        Span::styled(
            "gemini",
            Style::default()
                .fg(Color::Rgb(194, 164, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" {gemini}")),
    ]
}

fn run_settings_tui(app: &mut SettingsApp) -> Result<(), String> {
    enable_raw_mode().map_err(|err| err.to_string())?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).map_err(|err| err.to_string())?;

    let result = settings_tui_loop(&mut terminal, app);

    disable_raw_mode().map_err(|err| err.to_string())?;
    execute!(
        terminal.backend_mut(),
        TerminalClear(TerminalClearType::All),
        MoveTo(0, 0)
    )
    .map_err(|err| err.to_string())?;
    terminal.show_cursor().map_err(|err| err.to_string())?;
    result
}

fn settings_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut SettingsApp,
) -> Result<(), String> {
    loop {
        terminal
            .draw(|frame| draw_settings(frame, app))
            .map_err(|err| err.to_string())?;

        if !event::poll(Duration::from_millis(250)).map_err(|err| err.to_string())? {
            continue;
        }

        let Event::Key(key) = event::read().map_err(|err| err.to_string())? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if app.depth == SettingsDepth::Text {
            match key.code {
                KeyCode::Esc => {
                    app.depth = SettingsDepth::Fields;
                    app.input.clear();
                    app.status = "Edit canceled.".to_string();
                }
                KeyCode::Enter => {
                    if let Err(err) = apply_settings_text(app) {
                        app.status = err;
                    }
                }
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Char(' ') => {
                    app.input.push(' ');
                }
                KeyCode::Char(ch) => {
                    app.input.push(ch);
                }
                _ => {}
            }
            continue;
        }

        match key.code {
            KeyCode::Esc => match app.depth {
                SettingsDepth::Entries => return Ok(()),
                SettingsDepth::Fields => {
                    app.depth = SettingsDepth::Entries;
                    app.status = "Back to entries.".to_string();
                }
                SettingsDepth::Choices => {
                    app.depth = SettingsDepth::Fields;
                    app.choices.clear();
                    app.pending_choice = None;
                    app.status = "Choice canceled.".to_string();
                }
                SettingsDepth::Text => {}
            },
            KeyCode::Up => match app.depth {
                SettingsDepth::Entries => {
                    if app.selected_entry > 0 {
                        app.selected_entry -= 1;
                        normalize_selected_field(app);
                    }
                }
                SettingsDepth::Fields => {
                    if app.selected_field > 0 {
                        app.selected_field -= 1;
                    }
                }
                SettingsDepth::Choices => {
                    if app.selected_choice > 0 {
                        app.selected_choice -= 1;
                    }
                }
                SettingsDepth::Text => {}
            },
            KeyCode::Down => match app.depth {
                SettingsDepth::Entries => {
                    if app.selected_entry + 1 < app.entries.len() {
                        app.selected_entry += 1;
                        normalize_selected_field(app);
                    }
                }
                SettingsDepth::Fields => {
                    let field_count = entry_fields(app).len();
                    if app.selected_field + 1 < field_count {
                        app.selected_field += 1;
                    }
                }
                SettingsDepth::Choices => {
                    if app.selected_choice + 1 < app.choices.len() {
                        app.selected_choice += 1;
                    }
                }
                SettingsDepth::Text => {}
            },
            KeyCode::Char(' ') => {
                if app.depth == SettingsDepth::Choices {
                    if let Some(choice) = app.choices.get(app.selected_choice) {
                        app.pending_choice = Some(choice.value.clone());
                        app.status = format!("Selected {}. Press Enter to save.", choice.label);
                    }
                }
            }
            _ => {}
        }

        if key.code == KeyCode::Enter {
            match app.depth {
                SettingsDepth::Entries => {
                    app.depth = SettingsDepth::Fields;
                    app.status = format!("Editing {}.", entry_label(selected_entry(app)));
                }
                SettingsDepth::Fields => {
                    let (field, _, _) = current_field(app);
                    if matches!(field, SettingsField::Description) {
                        begin_settings_text_edit(app);
                    } else {
                        app.choices = choice_options(app);
                        app.selected_choice = 0;
                        app.pending_choice = None;
                        app.depth = SettingsDepth::Choices;
                        app.status = "Use arrows, Space selects, Enter saves.".to_string();
                    }
                }
                SettingsDepth::Choices => {
                    let value = app.pending_choice.clone().or_else(|| {
                        app.choices
                            .get(app.selected_choice)
                            .map(|choice| choice.value.clone())
                    });
                    if let Some(value) = value {
                        if let Err(err) = apply_settings_value(app, &value) {
                            app.status = err;
                        }
                    }
                }
                SettingsDepth::Text => {}
            }
        }
    }
}

fn draw_settings(frame: &mut ratatui::Frame<'_>, app: &SettingsApp) {
    frame.render_widget(Clear, frame.area());
    let backdrop = Block::default().style(Style::default().bg(Color::Rgb(10, 11, 19)));
    frame.render_widget(backdrop, frame.area());

    let panel = centered_rect(96, 96, frame.area());
    let shell = Block::default()
        .borders(Borders::ALL)
        .title(" conductor settings ")
        .style(settings_panel_style())
        .border_style(Style::default().fg(Color::Rgb(64, 72, 108)));
    frame.render_widget(shell, panel);
    let panel_inner = Layout::default()
        .margin(1)
        .constraints([Constraint::Min(1)])
        .split(panel)[0];

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(panel_inner);

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Conductor settings",
                Style::default()
                    .fg(Color::Rgb(245, 247, 250))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                app.config_path.display().to_string(),
                settings_muted_style(),
            ),
        ]),
        Line::from(""),
        Line::from(host_default_spans(&app.host_catalog)),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Overview ")
            .style(settings_panel_style())
            .border_style(Style::default().fg(Color::Rgb(64, 72, 108))),
    )
    .wrap(Wrap { trim: true });
    frame.render_widget(header, areas[0]);

    let breadcrumb = match app.depth {
        SettingsDepth::Entries => vec![
            Span::styled("1 entries", settings_accent_style()),
            Span::styled("  /  select a section", settings_muted_style()),
        ],
        SettingsDepth::Fields => vec![
            Span::styled("1 entries", settings_muted_style()),
            Span::styled("  /  ", settings_muted_style()),
            Span::styled("2 fields", settings_accent_style()),
            Span::styled(
                format!("  /  {}", entry_label(selected_entry(app))),
                settings_muted_style(),
            ),
        ],
        SettingsDepth::Choices => {
            let (_, label, _) = current_field(app);
            vec![
                Span::styled("1 entries", settings_muted_style()),
                Span::styled("  /  ", settings_muted_style()),
                Span::styled("2 fields", settings_muted_style()),
                Span::styled("  /  ", settings_muted_style()),
                Span::styled("3 options", settings_accent_style()),
                Span::styled(format!("  /  {label}"), settings_muted_style()),
            ]
        }
        SettingsDepth::Text => {
            let (_, label, _) = current_field(app);
            vec![
                Span::styled("1 entries", settings_muted_style()),
                Span::styled("  /  ", settings_muted_style()),
                Span::styled("2 fields", settings_muted_style()),
                Span::styled("  /  ", settings_muted_style()),
                Span::styled("3 edit", settings_accent_style()),
                Span::styled(format!("  /  {label}"), settings_muted_style()),
            ]
        }
    };
    let trail = Paragraph::new(vec![Line::from(breadcrumb)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Flow ")
                .style(settings_panel_style())
                .border_style(Style::default().fg(Color::Rgb(64, 72, 108))),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(trail, areas[1]);

    let (list_title, list_items, selected_index, active_list) = match app.depth {
        SettingsDepth::Entries => (
            "Sections".to_string(),
            app.entries
                .iter()
                .map(|entry| {
                    let label = entry_label(entry);
                    let desc = if matches!(entry, SettingsEntry::Surface) {
                        "Primary operator surface".to_string()
                    } else {
                        "Team profile".to_string()
                    };
                    ListItem::new(vec![
                        Line::from(vec![Span::styled(label, settings_accent_style())]),
                        Line::from(vec![Span::styled(desc, settings_muted_style())]),
                    ])
                })
                .collect::<Vec<_>>(),
            app.selected_entry,
            true,
        ),
        SettingsDepth::Fields => (
            format!("Fields  {}", entry_label(selected_entry(app))),
            entry_fields(app)
                .iter()
                .map(|(_, label, value)| {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{label:<14}"), settings_accent_style()),
                        Span::raw(value.clone()),
                    ]))
                })
                .collect::<Vec<_>>(),
            app.selected_field,
            true,
        ),
        SettingsDepth::Choices => (
            {
                let (_, label, _) = current_field(app);
                format!("Options  {label}")
            },
            app.choices
                .iter()
                .map(|choice| {
                    let marker = if app.pending_choice.as_deref() == Some(choice.value.as_str()) {
                        "[selected] "
                    } else {
                        ""
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(marker, Style::default().fg(Color::Rgb(124, 242, 203))),
                        Span::raw(choice.label.clone()),
                    ]))
                })
                .collect::<Vec<_>>(),
            app.selected_choice,
            true,
        ),
        SettingsDepth::Text => (
            "Details".to_string(),
            {
                let (field, label, value) = current_field(app);
                vec![
                    ListItem::new(Line::from(vec![
                        Span::styled("field  ", settings_muted_style()),
                        Span::styled(label, settings_accent_style()),
                    ])),
                    ListItem::new(Line::from(vec![
                        Span::styled("value  ", settings_muted_style()),
                        Span::raw(value),
                    ])),
                    ListItem::new(Line::from("")),
                    ListItem::new(Line::from(edit_hint(field))),
                ]
            },
            0,
            false,
        ),
    };
    let mut list_state = ListState::default();
    if !list_items.is_empty() {
        list_state.select(Some(selected_index.min(list_items.len().saturating_sub(1))));
    }
    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {list_title} "))
                .style(settings_panel_style())
                .border_style(Style::default().fg(Color::Rgb(64, 72, 108))),
        )
        .highlight_style(if active_list {
            settings_focus_style(true)
        } else {
            Style::default().fg(Color::Rgb(222, 226, 239))
        })
        .highlight_symbol("  ");
    frame.render_stateful_widget(list, areas[2], &mut list_state);

    let footer = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("status  ", settings_muted_style()),
            Span::raw(app.status.clone()),
        ]),
        Line::from(vec![
            Span::styled("keys    ", settings_muted_style()),
            Span::raw(
                "Up/Down move  Enter drills in  Space selects  Enter saves  Esc backs out  q quits",
            ),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Status ")
            .style(settings_panel_style())
            .border_style(Style::default().fg(Color::Rgb(64, 72, 108))),
    )
    .wrap(Wrap { trim: true });
    frame.render_widget(footer, areas[3]);

    if app.depth == SettingsDepth::Text {
        let popup_area = centered_rect(64, 28, frame.area());
        let (field, label, _) = current_field(app);
        let popup = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                format!("{} / {}", entry_label(selected_entry(app)), label),
                settings_accent_style(),
            )]),
            Line::from(""),
            Line::from(edit_hint(field)),
            Line::from(""),
            Line::from(app.input.clone()),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Edit ")
                .style(settings_panel_style())
                .border_style(Style::default().fg(Color::Rgb(124, 242, 203))),
        )
        .wrap(Wrap { trim: false });
        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup, popup_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn update_surface_field(cfg: &mut Config, field: SettingsField, value: &str) -> Result<(), String> {
    match field {
        SettingsField::SurfaceCli => {
            if value.trim().is_empty() {
                return Err("surface cli must not be empty".to_string());
            }
            cfg.surface.cli = value.trim().to_string();
        }
        SettingsField::Description => {
            if value.trim().is_empty() {
                return Err("surface description must not be empty".to_string());
            }
            cfg.surface.description = value.trim().to_string();
        }
        _ => return Err("unsupported surface field".to_string()),
    }
    Ok(())
}

fn run_open(args: &[String]) -> Result<(), String> {
    let run_id = args
        .first()
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_run_id);
    let (_, cfg) = load_resolved_config()?;
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, &run_id)?;
    cleanup_legacy_ops_session_if_solo(&store, &run_id)?;
    if should_use_native_resume(&cfg) {
        cleanup_default_surface_state(&store, &run_id)?;
        cleanup_surface_tmux_session(&run_id)?;
        return run_native_surface_resume(&cfg, &run_id);
    }
    ensure_active_ralph_watch(&run_id)?;
    cleanup_default_surface_state(&store, &run_id)?;
    run_surface_ops_open(&run_id, true)
}

fn run_attach_alias(args: &[String]) -> Result<(), String> {
    let first = args.first().map(String::as_str).unwrap_or("");
    let (run_id, worker_name): (String, String) = if first.contains("session-")
        || first.starts_with("codex-")
        || first.starts_with("worker-")
        || first.starts_with("claude-")
        || first.starts_with("gemini-")
    {
        (default_run_id(), first.to_string())
    } else {
        let run = if first.trim().is_empty() {
            default_run_id()
        } else {
            first.to_string()
        };
        let worker = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "codex-1".to_string());
        (run, worker)
    };

    let session_id = if worker_name.starts_with("session-") {
        worker_name
    } else {
        format!("session-{worker_name}")
    };
    let attach_args = vec![run_id, session_id];
    run_worker_attach(&attach_args)
}

fn run_team(args: &[String]) -> Result<(), String> {
    let (_, cfg) = load_resolved_config()?;
    let available_profiles = configured_team_profiles(&cfg);
    let (run_id, team_size, agent_names, prompt) =
        parse_team_invocation(args, &available_profiles)?;

    if team_size == 0 {
        return Err("team count must be at least 1".to_string());
    }

    let active_tmux_session = current_tmux_session_hint()
        .filter(|session_name| tmux_session_exists(session_name).unwrap_or(false));

    if let Some(tmux_session_name) = active_tmux_session {
        open_direct_team_in_current_surface(
            &run_id,
            team_size,
            &agent_names,
            prompt.as_deref(),
            &tmux_session_name,
        )
    } else {
        ensure_explicit_team_sessions(&run_id, team_size, &agent_names)?;
        let tmux_session_name = default_tmux_session_name(&run_id);
        run_ops_open_with_filter(&run_id, &tmux_session_name, None)
    }
}

fn configured_team_profiles(cfg: &Config) -> Vec<String> {
    cfg.workers
        .keys()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
}

fn parse_team_invocation(
    args: &[String],
    available_profiles: &[String],
) -> Result<(String, usize, Vec<String>, Option<String>), String> {
    let mut positionals = Vec::new();
    let mut prompt = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--prompt" {
            prompt = args.get(index + 1..).map(|parts| parts.join(" "));
            break;
        }
        positionals.push(args[index].clone());
        index += 1;
    }

    let prompt = prompt
        .filter(|value| !value.trim().is_empty())
        .or_else(|| env::var("CONDUCTOR_TEAM_PROMPT").ok())
        .filter(|value| !value.trim().is_empty());

    let inferred = infer_team_shape(available_profiles, prompt.as_deref());
    match positionals.as_slice() {
        [] => Ok((
            default_run_id(),
            inferred.len(),
            inferred,
            prompt,
        )),
        [count] if count.parse::<usize>().is_ok() => {
            let team_size = count.parse::<usize>().map_err(|err| err.to_string())?;
            Ok((default_run_id(), team_size, infer_team_shape(available_profiles, prompt.as_deref()), prompt))
        }
        [run_id] => Ok((
            run_id.to_string(),
            inferred.len(),
            inferred,
            prompt,
        )),
        [run_id, count] if count.parse::<usize>().is_ok() => {
            let team_size = count.parse::<usize>().map_err(|err| err.to_string())?;
            Ok((
                run_id.to_string(),
                team_size,
                infer_team_shape(available_profiles, prompt.as_deref()),
                prompt,
            ))
        }
        [count, profiles @ ..] if count.parse::<usize>().is_ok() => {
            let team_size = count.parse::<usize>().map_err(|err| err.to_string())?;
            let agent_names = profiles
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            let agent_names = if agent_names.is_empty() {
                infer_team_shape(available_profiles, prompt.as_deref())
            } else {
                agent_names
            };
            Ok((default_run_id(), team_size, agent_names, prompt))
        }
        [run_id, count, profiles @ ..] if count.parse::<usize>().is_ok() => {
            let team_size = count.parse::<usize>().map_err(|err| err.to_string())?;
            let agent_names = profiles
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            let agent_names = if agent_names.is_empty() {
                infer_team_shape(available_profiles, prompt.as_deref())
            } else {
                agent_names
            };
            Ok((run_id.to_string(), team_size, agent_names, prompt))
        }
        _ => Err(
            "team accepts: no args, <count>, <run_id>, <run_id> <count>, <count> <profile>..., or <run_id> <count> <profile>..."
                .to_string(),
        ),
    }
}

fn infer_team_shape(available_profiles: &[String], prompt: Option<&str>) -> Vec<String> {
    let available = available_profiles
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>();
    let has = |name: &str| available.contains(&name);
    let mut inferred = Vec::new();
    let lower = prompt.unwrap_or("").to_ascii_lowercase();
    let reconnaissance = matches_any(
        &lower,
        &[
            "map",
            "structure",
            "repository",
            "repo",
            "codebase",
            "inspect",
            "analyze",
        ],
    );

    if has("explore") {
        inferred.push("explore".to_string());
    }
    if has("build")
        && !reconnaissance
        && !matches_any(
            &lower,
            &["review", "verify", "audit", "regression", "test-only"],
        )
    {
        inferred.push("build".to_string());
    }
    if has("review")
        && (reconnaissance
            || matches_any(&lower, &["review", "risk", "regression", "investigate"])
            || !has("build"))
    {
        inferred.push("review".to_string());
    }
    if has("verify")
        && matches_any(
            &lower,
            &[
                "verify",
                "validation",
                "test",
                "evidence",
                "confirm",
                "repro",
            ],
        )
    {
        inferred.push("verify".to_string());
    }

    if inferred.is_empty() {
        for candidate in ["explore", "build", "review", "verify"] {
            if has(candidate) {
                inferred.push(candidate.to_string());
            }
        }
    }

    if inferred.is_empty() {
        inferred.extend(available_profiles.iter().cloned());
    }

    inferred
}

fn matches_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn run_ralph(args: &[String]) -> Result<(), String> {
    let run_id = args
        .first()
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_run_id);
    let requested_width = args
        .get(1)
        .map(|value| value.parse::<usize>().map_err(|err| err.to_string()))
        .transpose()?;
    let (_, cfg) = load_resolved_config()?;
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, &run_id)?;
    reopen_run_for_ralph(&store, &run_id)?;
    ensure_surface_session(&store, &cfg, &run_id)?;
    let ralph_was_enabled = read_ralph_loop_state_from_root(store.root(), &run_id)
        .map(|state| state.enabled)
        .unwrap_or(false);
    enable_ralph_loop(&run_id)?;
    ensure_active_ralph_watch(&run_id)?;
    let tmux_session_name = surface_tmux_session_name(&run_id);
    if command_available("tmux") && tmux_session_exists(&tmux_session_name)? {
        let _ = configure_tmux_main_exit_hook(&tmux_session_name, false);
    }
    let should_prime = if requested_width.is_some() {
        true
    } else {
        should_prime_ralph_on_entry(&store, &run_id, ralph_was_enabled)
    };
    if requested_width.is_some() {
        ensure_team_sessions(&run_id, TeamMode::Ralph, requested_width)?;
        if should_prime {
            prime_ralph_operator_loop(&run_id);
        }
        return run_ops_open_with_filter(&run_id, &tmux_session_name, None);
    }
    if command_available("tmux") && tmux_session_exists(&tmux_session_name)? {
        collapse_tmux_surface_to_main(&tmux_session_name)?;
        if should_prime {
            prime_ralph_operator_loop(&run_id);
        }
        if current_tmux_session_hint().as_deref() == Some(tmux_session_name.as_str()) {
            return Ok(());
        }
        return attach_tmux_ops_session(&tmux_session_name);
    }
    run_surface_ops_open(&run_id, false)?;
    if should_prime {
        prime_ralph_operator_loop(&run_id);
    }
    Ok(())
}

fn reopen_run_for_ralph(store: &StateStore, run_id: &str) -> Result<(), String> {
    let mut run = store.read_run(run_id)?;
    if run.active && !matches!(run.current_phase, RunPhase::Complete | RunPhase::Failed | RunPhase::Cancelled) {
        return Ok(());
    }
    run.active = true;
    run.current_phase = RunPhase::Executing;
    run.updated_at = Utc::now();
    run.completed_at = None;
    run.stop_reason = None;
    store.write_run(&run)?;
    let _ = store.refresh_snapshot(run_id)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RalphLoopState {
    enabled: bool,
    watcher_pid: Option<u32>,
}

fn ralph_loop_file(run_id: &str) -> Result<PathBuf, String> {
    Ok(ralph_loop_file_for_root(&resolve_state_root()?, run_id))
}

fn ralph_loop_file_for_root(root: &Path, run_id: &str) -> PathBuf {
    root.join("runs").join(run_id).join("ralph_loop.json")
}

fn read_ralph_loop_state(run_id: &str) -> Result<RalphLoopState, String> {
    read_ralph_loop_state_from_root(&resolve_state_root()?, run_id)
}

fn read_ralph_loop_state_from_root(root: &Path, run_id: &str) -> Result<RalphLoopState, String> {
    let path = ralph_loop_file_for_root(root, run_id);
    if !path.exists() {
        return Ok(RalphLoopState::default());
    }
    let raw = fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&raw).map_err(|err| err.to_string())
}

fn write_ralph_loop_state(run_id: &str, state: &RalphLoopState) -> Result<(), String> {
    write_ralph_loop_state_to_root(&resolve_state_root()?, run_id, state)
}

fn write_ralph_loop_state_to_root(
    root: &Path,
    run_id: &str,
    state: &RalphLoopState,
) -> Result<(), String> {
    let path = ralph_loop_file_for_root(root, run_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let payload = serde_json::to_vec_pretty(state).map_err(|err| err.to_string())?;
    fs::write(path, payload).map_err(|err| err.to_string())
}

fn enable_ralph_loop(run_id: &str) -> Result<(), String> {
    let mut state = read_ralph_loop_state(run_id)?;
    state.enabled = true;
    write_ralph_loop_state(run_id, &state)
}

fn enable_ralph_loop_for_root(root: &Path, run_id: &str) -> Result<(), String> {
    let mut state = read_ralph_loop_state_from_root(root, run_id)?;
    state.enabled = true;
    write_ralph_loop_state_to_root(root, run_id, &state)
}

fn disable_ralph_loop(run_id: &str) -> Result<(), String> {
    let mut state = read_ralph_loop_state(run_id)?;
    state.enabled = false;
    state.watcher_pid = None;
    write_ralph_loop_state(run_id, &state)
}

fn disable_ralph_loop_for_root(root: &Path, run_id: &str) -> Result<(), String> {
    let mut state = read_ralph_loop_state_from_root(root, run_id)?;
    state.enabled = false;
    state.watcher_pid = None;
    write_ralph_loop_state_to_root(root, run_id, &state)
}

fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn ensure_active_ralph_watch(run_id: &str) -> Result<(), String> {
    let mut state = read_ralph_loop_state(run_id)?;
    if !state.enabled {
        return Ok(());
    }
    if state.watcher_pid.map(process_is_alive).unwrap_or(false) {
        return Ok(());
    }
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let state_root = resolve_state_root()?;
    let config_path = resolve_config_path().ok();
    let mut child = Command::new(conductor_bin);
    child
        .arg("ralph-watch")
        .arg(run_id)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env("CONDUCTOR_STATE_DIR", state_root);
    if let Some(config_path) = config_path {
        child.env("CONDUCTOR_CONFIG", config_path);
    }
    let child = child.spawn().map_err(|err| err.to_string())?;
    state.watcher_pid = Some(child.id());
    write_ralph_loop_state(run_id, &state)
}

fn run_ralph_watch(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "ralph-watch requires <run_id>")?;
    let store = StateStore::new(resolve_state_root()?);
    let mut cursor = 0usize;
    let mut last_prime_at: Option<chrono::DateTime<Utc>> = None;
    let mut last_stall_signature: Option<String> = None;

    loop {
        let state = read_ralph_loop_state(run_id)?;
        if !state.enabled {
            break;
        }
        let run = match store.read_run(run_id) {
            Ok(run) => run,
            Err(_) => break,
        };
        if !run.active
            || matches!(
                run.current_phase,
                RunPhase::Complete | RunPhase::Failed | RunPhase::Cancelled
            )
        {
            let _ = disable_ralph_loop(run_id);
            break;
        }
        let snapshot = match store.read_snapshot(run_id) {
            Ok(snapshot) => snapshot,
            Err(_) => break,
        };
        let events = store.read_events(run_id)?;
        let new_events = if cursor < events.len() {
            events[cursor..].to_vec()
        } else {
            Vec::new()
        };
        cursor = events.len();
        let wakeable_events = filter_events(new_events, None, true);
        let stall_signature = ralph_stall_signature(
            &snapshot,
            wakeable_events.iter().any(ralph_wake_event_counts),
        );
        let should_prime = stall_signature
            .as_ref()
            .filter(|signature| last_stall_signature.as_ref() != Some(*signature))
            .is_some()
            && last_prime_at
                .map(|last| Utc::now() - last >= chrono::Duration::seconds(8))
                .unwrap_or(true);
        if should_prime {
            prime_ralph_operator_loop(run_id);
            last_prime_at = Some(Utc::now());
            last_stall_signature = stall_signature;
        } else if stall_signature.is_none() {
            last_stall_signature = None;
        }
        thread::sleep(Duration::from_millis(1500));
    }

    Ok(())
}

fn ralph_wake_event_counts(event: &EventEnvelope) -> bool {
    matches!(
        event.event,
        EventKind::WorkerStateChanged
            | EventKind::MailboxMessageCreated
            | EventKind::MailboxMessageDelivered
            | EventKind::MailboxMessageNotified
            | EventKind::LeaderNotificationDeferred
            | EventKind::ClaimReclaimed
            | EventKind::HandoffNeeded
    )
}

fn snapshot_has_active_worker_progress(snapshot: &crate::runtime::types::RuntimeSnapshot) -> bool {
    snapshot.workers.iter().any(|worker| {
        worker.worker_id != "main"
            && worker.worker_id != "orchestrator-main"
            && matches!(
                worker.state,
                WorkerState::Working | WorkerState::AwaitingReport
            )
            && !worker
                .reason
                .as_deref()
                .map(|reason| {
                    reason == "stalled_non_reporting"
                        || reason.starts_with("awaiting_report_nudged")
                        || reason == "direct_team_bootstrapping"
                        || reason == "direct_team_pane_respawned"
                })
                .unwrap_or(false)
    })
}

fn snapshot_operator_recently_active(
    snapshot: &crate::runtime::types::RuntimeSnapshot,
    now: chrono::DateTime<Utc>,
) -> bool {
    snapshot.workers.iter().any(|worker| {
        (worker.worker_id == "main" || worker.worker_id == "orchestrator-main")
            && worker
                .last_heartbeat_at
                .map(|heartbeat| now - heartbeat < chrono::Duration::seconds(20))
                .unwrap_or(false)
    })
}

fn should_prime_ralph_on_entry(store: &StateStore, run_id: &str, was_enabled: bool) -> bool {
    if !was_enabled {
        return true;
    }
    let snapshot = store
        .refresh_snapshot(run_id)
        .or_else(|_| store.read_snapshot(run_id));
    match snapshot {
        Ok(snapshot) => ralph_stall_signature(&snapshot, false).is_some(),
        Err(_) => true,
    }
}

fn ralph_stall_signature(
    snapshot: &crate::runtime::types::RuntimeSnapshot,
    saw_wake_event: bool,
) -> Option<String> {
    let now = Utc::now();
    if snapshot.readiness.stale_operator && snapshot.monitor.pending_leader_notifications > 0 {
        return Some(format!(
            "stale-operator:{}:{}",
            snapshot.monitor.pending_leader_notifications, snapshot.mailbox.unread
        ));
    }

    if snapshot_has_active_worker_progress(snapshot) {
        return None;
    }

    let operator_recently_active = snapshot_operator_recently_active(snapshot, now);

    match snapshot.decision.next_action.as_str() {
        "reassign-or-close" if operator_recently_active => None,
        "review-approval" | "relaunch-stalled" | "unblock" | "accept-completion"
        | "verify-completion" | "reassign-or-close" => Some(format!(
            "{}|{}|{}",
            snapshot.decision.next_action,
            snapshot.decision.focus_worker.as_deref().unwrap_or(""),
            snapshot.decision.reason
        )),
        "read-inbox" if saw_wake_event || snapshot.monitor.pending_leader_notifications > 0 => {
            if operator_recently_active {
                None
            } else {
                Some(format!(
                    "read-inbox|{}|{}",
                    snapshot.mailbox.unread, snapshot.decision.reason
                ))
            }
        }
        _ => None,
    }
}

fn prime_ralph_operator_loop(run_id: &str) {
    let store = match resolve_state_root() {
        Ok(root) => StateStore::new(root),
        Err(_) => return,
    };
    let snapshot = match store.read_snapshot(run_id) {
        Ok(snapshot) => snapshot,
        Err(_) => return,
    };
    let prompt = build_ralph_operator_prompt(run_id, &snapshot);
    let _ = push_text_to_main_pane(run_id, &prompt);
}

fn build_ralph_operator_prompt(
    run_id: &str,
    snapshot: &crate::runtime::types::RuntimeSnapshot,
) -> String {
    let focus = snapshot
        .decision
        .focus_worker
        .as_deref()
        .unwrap_or("the most active lane");
    let why = snapshot.decision.reason.trim();
    let next = snapshot.decision.next_action.trim();
    let suggested = suggested_operator_command(run_id, snapshot, None)
        .map(|value| format!("\nSuggested command: {value}"))
        .unwrap_or_default();
    format!(
        "Ralph loop active for run {run_id}. Stay in the operator lane and keep iterating until one verified outcome is accepted or the run is explicitly cancelled. Do not widen into a team unless a worker count was explicitly requested. Current focus: {focus}. Why: {why}. Next: {next}.{suggested}"
    )
}

fn ensure_team_sessions(
    run_id: &str,
    mode: TeamMode,
    requested_width: Option<usize>,
) -> Result<(), String> {
    let (_, cfg) = load_resolved_config()?;
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;

    ensure_surface_session(&store, &cfg, run_id)?;

    let snapshot = store.read_snapshot(run_id)?;
    for (worker_id, adapter_kind) in plan_team_roster(&snapshot, mode, requested_width) {
        let adapter = worker_adapter_config(&cfg, &adapter_kind)?;
        ensure_adapter_session(&store, &adapter, run_id, &worker_id, None)?;
    }

    Ok(())
}

fn ensure_explicit_team_sessions(
    run_id: &str,
    team_size: usize,
    agent_names: &[String],
) -> Result<(), String> {
    let (_, cfg) = load_resolved_config()?;
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;

    ensure_surface_session(&store, &cfg, run_id)?;

    let mut stem_counts = BTreeMap::<String, usize>::new();
    for index in 0..team_size {
        let agent_name = &agent_names[index % agent_names.len()];
        let (worker_type, worker_stem) = resolve_team_agent(&cfg, agent_name)?;
        let adapter = worker_adapter_config(&cfg, &worker_type)?;
        let counter = stem_counts.entry(worker_stem.clone()).or_insert(0);
        *counter += 1;
        let worker_id = format!("{worker_stem}-{counter}");
        ensure_adapter_session(&store, &adapter, run_id, &worker_id, None)?;
    }

    Ok(())
}

fn ensure_surface_session(store: &StateStore, cfg: &Config, run_id: &str) -> Result<(), String> {
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let launch = resolve_surface_launch(cfg, run_id, false)?;
    let desired_kind = WorkerKind::Orchestrator;
    let worker_id = "main";
    if let Ok(existing) = store.read_session(run_id, &format!("session-{worker_id}")) {
        let mut worker = store.read_worker(run_id, worker_id)?;
        if worker.worker_kind != desired_kind {
            worker.worker_kind = desired_kind.clone();
            let _ = store.upsert_worker(worker)?;
        }
        if existing.status == SessionStatus::Running || existing.status == SessionStatus::Starting {
            if existing.program == launch.program && existing.args == launch.args {
                return Ok(());
            }
            let _ = send_session_command(Path::new(&existing.socket_path), &SessionCommand::Stop);
        }
    }
    let result = spawn_session(
        store,
        run_id,
        worker_id,
        &launch.program,
        &launch.args,
        launch.cwd.as_deref(),
        &launch.env,
        &conductor_bin,
    )?;
    let mut worker = store.read_worker(run_id, worker_id)?;
    if worker.worker_kind != desired_kind {
        worker.worker_kind = desired_kind.clone();
        let _ = store.upsert_worker(worker)?;
    }
    let _ = result;
    Ok(())
}

fn should_use_native_resume(cfg: &Config) -> bool {
    cfg.surface.cli == "codex" && !command_available("tmux")
}

fn worker_host_pids_for_run(run_id: &str) -> Result<Vec<u32>, String> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,command="])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err("failed to inspect worker-host processes".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let current_pid = std::process::id();
    let pids = stdout
        .lines()
        .filter_map(|line| parse_worker_host_pid(line, run_id))
        .filter(|pid| *pid != current_pid)
        .collect::<Vec<_>>();
    Ok(pids)
}

fn parse_worker_host_pid(line: &str, run_id: &str) -> Option<u32> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let pid = parts.next()?.trim().parse::<u32>().ok()?;
    let command = parts.next()?.trim();
    if command.contains("conductor worker-host ")
        && command.contains(&format!("worker-host {run_id} "))
    {
        return Some(pid);
    }
    None
}

fn process_exists(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn terminate_pid(pid: u32) {
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn force_kill_pid(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn cleanup_orphaned_worker_hosts(run_id: &str) -> Result<(), String> {
    let pids = worker_host_pids_for_run(run_id)?;
    for pid in &pids {
        if process_exists(*pid) {
            terminate_pid(*pid);
        }
    }
    if !pids.is_empty() {
        thread::sleep(Duration::from_millis(250));
    }
    for pid in pids {
        if process_exists(pid) {
            force_kill_pid(pid);
        }
    }
    Ok(())
}

fn cleanup_surface_tmux_session(run_id: &str) -> Result<(), String> {
    let session_name = surface_tmux_session_name(run_id);
    if tmux_session_exists(&session_name)? {
        run_tmux(["kill-session", "-t", &session_name])?;
    }
    Ok(())
}

fn cleanup_legacy_ops_session_if_solo(store: &StateStore, run_id: &str) -> Result<(), String> {
    let legacy_session = default_tmux_session_name(run_id);
    if !tmux_session_exists(&legacy_session)? {
        return Ok(());
    }
    if !legacy_ops_session_is_solo(store, run_id)? {
        return Ok(());
    }
    run_tmux(["kill-session", "-t", &legacy_session])?;
    Ok(())
}

fn legacy_ops_session_is_solo(store: &StateStore, run_id: &str) -> Result<bool, String> {
    let snapshot = match store.read_snapshot(run_id) {
        Ok(snapshot) => snapshot,
        Err(_) => return Ok(true),
    };
    let has_live_non_main_lane = snapshot.workers.iter().any(|worker| {
        worker.worker_id != "main"
            && worker.worker_id != "orchestrator-main"
            && matches!(
                worker.state,
                WorkerState::Idle
                    | WorkerState::Working
                    | WorkerState::AwaitingReport
                    | WorkerState::Blocked
                    | WorkerState::Done
                    | WorkerState::DonePendingVerification
            )
    });
    Ok(!has_live_non_main_lane)
}

fn run_native_surface_resume(cfg: &Config, run_id: &str) -> Result<(), String> {
    if !stdin().is_terminal() || !stdout().is_terminal() {
        return Err("codex resume requires an interactive terminal".to_string());
    }
    let mut launch = resolve_surface_launch(cfg, run_id, true)?;
    launch.env.remove("CONDUCTOR_TMUX_SESSION");
    let mut command = Command::new(&launch.program);
    command.args(&launch.args);
    if let Some(cwd) = launch.cwd.as_deref() {
        command.current_dir(cwd);
    }
    command.envs(&launch.env);
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());
    let status = command.status().map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "native codex resume exited with status {}",
            status
                .code()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ))
    }
}

#[derive(Debug, Clone)]
struct OpsPaneSpec {
    title: String,
    command: String,
    starter_prompt: Option<String>,
}

fn open_direct_team_in_current_surface(
    run_id: &str,
    team_size: usize,
    agent_names: &[String],
    team_prompt: Option<&str>,
    tmux_session_name: &str,
) -> Result<(), String> {
    let (_, cfg) = load_resolved_config()?;
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;
    stop_worker_session_if_present(&store, run_id, "main");
    let cwd = env::current_dir().map_err(|err| err.to_string())?;

    let pane_specs = prepare_direct_team_panes(
        &store,
        &cfg,
        run_id,
        team_size,
        agent_names,
        team_prompt,
        &cwd,
    )?;

    rebuild_direct_team_surface(tmux_session_name, &pane_specs)?;
    schedule_team_report_nudge(run_id, tmux_session_name)?;
    schedule_team_followup_loop(run_id, tmux_session_name)?;
    Ok(())
}

fn prepare_direct_team_panes(
    store: &StateStore,
    cfg: &Config,
    run_id: &str,
    team_size: usize,
    agent_names: &[String],
    team_prompt: Option<&str>,
    cwd: &Path,
) -> Result<Vec<OpsPaneSpec>, String> {
    let mut stem_counts = BTreeMap::<String, usize>::new();
    let mut pane_specs = Vec::new();
    for index in 0..team_size {
        let agent_name = &agent_names[index % agent_names.len()];
        let (worker_type, worker_stem) = resolve_team_agent(cfg, agent_name)?;
        let adapter = worker_adapter_config(cfg, &worker_type)?;
        let counter = stem_counts.entry(worker_stem.clone()).or_insert(0);
        *counter += 1;
        let worker_id = format!("{worker_stem}-{counter}");
        let launch = resolve_worker_adapter(&adapter, run_id, &worker_id, None, None)?;
        stop_worker_session_if_present(store, run_id, &worker_id);
        let summary = Some(format!("direct {} pane ready", worker_type));
        let starter_prompt = render_team_starter_prompt(
            run_id,
            &worker_id,
            &worker_type,
            team_prompt,
            Some((&adapter.cli, &adapter.model)),
        );
        let use_inline_prompt = launch_uses_inline_prompt(&launch);
        store.upsert_worker(WorkerRecord {
            worker_id: worker_id.clone(),
            run_id: run_id.to_string(),
            worker_kind: worker_kind_for_type(&worker_type, &worker_id),
            session_ref: None,
            state: WorkerState::AwaitingReport,
            current_task_id: None,
            current_summary: summary,
            terminal_label: Some(worker_id.clone()),
            last_heartbeat_at: Some(Utc::now()),
            last_stdout_at: None,
            last_event_at: Some(Utc::now()),
            reason: Some("direct_team_bootstrapping".to_string()),
        })?;
        let _ = store.append_runtime_event(
            run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: EventKind::WorkerBootstrapStarted,
                timestamp: Utc::now(),
                run_id: Some(run_id.to_string()),
                session_id: None,
                source: "team".to_string(),
                worker: Some(worker_id.clone()),
                task_id: None,
                message_id: None,
                reason: Some("direct_team_bootstrapping".to_string()),
                context: serde_json::Map::from_iter([
                    ("worker_type".to_string(), json!(worker_type)),
                    ("model".to_string(), json!(adapter.model)),
                ]),
            },
        );
        pane_specs.push(OpsPaneSpec {
            title: worker_id.clone(),
            command: build_direct_launch_shell_command(
                cwd,
                &launch,
                use_inline_prompt.then_some(starter_prompt.as_str()),
            ),
            starter_prompt: (!use_inline_prompt).then_some(starter_prompt),
        });
    }
    Ok(pane_specs)
}

fn stop_worker_session_if_present(store: &StateStore, run_id: &str, worker_id: &str) {
    let session_id = format!("session-{worker_id}");
    if let Ok(session) = store.read_session(run_id, &session_id) {
        let _ = send_session_command(Path::new(&session.socket_path), &SessionCommand::Stop);
    }
}

fn cleanup_default_surface_state(store: &StateStore, run_id: &str) -> Result<(), String> {
    for worker_id in store.list_worker_ids(run_id)? {
        if worker_id == "main" || worker_id == "orchestrator-main" {
            continue;
        }
        stop_worker_session_if_present(store, run_id, &worker_id);
        store.delete_worker(run_id, &worker_id)?;
    }

    for session_id in store.list_session_ids(run_id)? {
        if session_id != "session-main" {
            store.delete_session(run_id, &session_id)?;
        }
    }

    stop_worker_session_if_present(store, run_id, "main");
    store.delete_session(run_id, "session-main")?;
    cleanup_orphaned_worker_hosts(run_id)?;
    store.refresh_snapshot(run_id)?;
    Ok(())
}

fn render_team_starter_prompt(
    run_id: &str,
    worker_id: &str,
    worker_type: &str,
    team_prompt: Option<&str>,
    provider: Option<(&str, &str)>,
) -> String {
    let first_step = match worker_type {
        "explore" => "Map dirs, entry points, boundaries, and likely files. Stay read-only.",
        "build" => "Find the edit surface, CLI entry, and owning modules.",
        "review" => "Check docs, config, contracts, and risky seams first.",
        "verify" => "Check tests, commands, evidence, and obvious gaps first.",
        _ => "Take the smallest concrete next step in your lane.",
    };
    let current_task = match team_prompt {
        Some(prompt) => format!("Current task: {prompt}"),
        None => "Current task: inspect the repository and produce a fast situational summary."
            .to_string(),
    };
    let base = format!(
        "You are {worker_id} in conductor run {run_id}. Profile: {worker_type}.\n\
{current_task}\n\
{first_step}\n\
Stay in your lane. Do not use built-in sub-agent tools. Do not act like the operator.\n\
Report progress fast with `conductor report {worker_id} \"<short result>\"`.\n\
After reporting, continue assigned work or the next feasible task.\n\
If blocked, report `blocked: <reason>`. When truly finished, report `done: <result>`."
    );
    let Some((cli, model)) = provider else {
        return base;
    };
    if cli != "codex" {
        return base;
    }
    if let Some(followup) = build_provider_bootstrap_followup(worker_id, model) {
        return format!("{followup}\n{base}");
    }
    base
}

fn build_team_report_nudge_prompt(worker_id: &str) -> String {
    format!(
        "Act now. Continue your lane, report progress with `conductor report {worker_id} \"<short result>\"`, report `blocked: <reason>` if stuck, or `done: <result>` when truly finished."
    )
}

fn build_provider_bootstrap_followup(worker_id: &str, model: &str) -> Option<String> {
    match model {
        "gpt-5.3-codex-spark" | "gpt-5.4-mini" => Some(format!(
            "First reply: one short ack and the first concrete finding. Then continue the lane and use `conductor report {worker_id} \"<short result>\"` early."
        )),
        _ => None,
    }
}

fn advance_team_nudge_reason(reason: Option<&str>) -> (&'static str, bool) {
    match reason {
        Some("awaiting_report_nudged") => ("awaiting_report_nudged_twice", false),
        Some("awaiting_report_nudged_twice") | Some("stalled_non_reporting") => {
            ("stalled_non_reporting", true)
        }
        _ => ("awaiting_report_nudged", false),
    }
}

fn build_stalled_worker_prompt(worker_id: &str) -> String {
    format!("{worker_id}: stalled waiting for a progress report")
}

fn build_all_workers_idle_prompt(run_id: &str, summaries: &[(String, String)]) -> String {
    let lines = summaries
        .iter()
        .map(|(worker_id, summary)| format!("- {worker_id}: {summary}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "All team workers in run {run_id} are idle. Review the latest lane reports below, decide the next assignments, relaunch team workers if needed, or conclude the run.\n\n{lines}"
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkerReportKind {
    Progress,
    Blocked,
    Done,
    Handoff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockedKind {
    Dependency,
    Approval,
    Evidence,
    Scope,
    Generic,
}

fn classify_worker_report(summary: &str) -> (WorkerState, String, WorkerReportKind) {
    let normalized = summary.trim_start().to_ascii_lowercase();
    if normalized.starts_with("blocked:") || normalized.starts_with("blocked ") {
        let blocked_kind = infer_blocked_kind(&normalized);
        (
            WorkerState::Blocked,
            format!(
                "blocked_{}_reported_to_operator",
                blocked_kind.as_reason_suffix()
            ),
            WorkerReportKind::Blocked,
        )
    } else if normalized.starts_with("done:")
        || normalized.starts_with("done ")
        || normalized.starts_with("complete:")
        || normalized.starts_with("complete ")
        || normalized.starts_with("completed:")
        || normalized.starts_with("completed ")
        || normalized.starts_with("final:")
        || normalized.starts_with("final ")
    {
        (
            WorkerState::DonePendingVerification,
            "completion_reported_to_operator".to_string(),
            WorkerReportKind::Done,
        )
    } else {
        (
            WorkerState::Working,
            "reported_progress_continuing".to_string(),
            WorkerReportKind::Progress,
        )
    }
}

fn classify_structured_worker_report(
    kind: WorkerReportKind,
    summary: &str,
) -> (WorkerState, String, WorkerReportKind) {
    match kind {
        WorkerReportKind::Progress => (
            WorkerState::Working,
            "reported_progress_continuing".to_string(),
            WorkerReportKind::Progress,
        ),
        WorkerReportKind::Blocked => {
            let blocked_kind = infer_blocked_kind(&summary.to_ascii_lowercase());
            (
                WorkerState::Blocked,
                format!(
                    "blocked_{}_reported_to_operator",
                    blocked_kind.as_reason_suffix()
                ),
                WorkerReportKind::Blocked,
            )
        }
        WorkerReportKind::Done => (
            WorkerState::DonePendingVerification,
            "completion_reported_to_operator".to_string(),
            WorkerReportKind::Done,
        ),
        WorkerReportKind::Handoff => (
            WorkerState::Working,
            "handoff_requested_from_lane".to_string(),
            WorkerReportKind::Handoff,
        ),
    }
}

impl BlockedKind {
    fn as_reason_suffix(self) -> &'static str {
        match self {
            BlockedKind::Dependency => "dependency",
            BlockedKind::Approval => "approval",
            BlockedKind::Evidence => "evidence",
            BlockedKind::Scope => "scope",
            BlockedKind::Generic => "generic",
        }
    }
}

fn infer_blocked_kind(summary: &str) -> BlockedKind {
    if summary.contains("approval")
        || summary.contains("approve")
        || summary.contains("signoff")
        || summary.contains("reviewer")
    {
        BlockedKind::Approval
    } else if summary.contains("evidence")
        || summary.contains("proof")
        || summary.contains("repro")
        || summary.contains("test")
        || summary.contains("verify")
    {
        BlockedKind::Evidence
    } else if summary.contains("scope")
        || summary.contains("unclear")
        || summary.contains("missing spec")
        || summary.contains("need context")
    {
        BlockedKind::Scope
    } else if summary.contains("dependency")
        || summary.contains("waiting on")
        || summary.contains("token")
        || summary.contains("credential")
        || summary.contains("api")
        || summary.contains("mcp")
        || summary.contains("service")
    {
        BlockedKind::Dependency
    } else {
        BlockedKind::Generic
    }
}

fn build_operator_followup_prompt(
    worker_id: &str,
    summary: &str,
    report_kind: WorkerReportKind,
) -> Option<String> {
    match report_kind {
        WorkerReportKind::Progress => None,
        WorkerReportKind::Handoff => None,
        WorkerReportKind::Blocked => {
            let blocked_kind = infer_blocked_kind(&summary.to_ascii_lowercase());
            let action = match blocked_kind {
                BlockedKind::Dependency => "unblock it or reroute the dependency",
                BlockedKind::Approval => "review the pending approval or reroute the lane",
                BlockedKind::Evidence => "ask for stronger evidence or redirect verification",
                BlockedKind::Scope => "clarify the scope and restart the lane narrowly",
                BlockedKind::Generic => "unblock it, reroute the lane, or stop the branch",
            };
            Some(format!(
                "{worker_id} is blocked. Decide whether to {action}. Use `conductor ask {worker_id} \"<follow-up>\"` if you need a narrower retry.\n\nLatest report: {summary}"
            ))
        }
        WorkerReportKind::Done => {
            if worker_id.starts_with("verify-") {
                Some(format!(
                    "{worker_id} finished verification. Decide whether to accept the branch, request one more check, or close the run. Use `conductor ask {worker_id} \"<follow-up>\"` if you need a narrower verification retry.\n\nLatest report: {summary}"
                ))
            } else {
                Some(format!(
                    "{worker_id} finished its lane. Decide whether to accept it, ask for stronger evidence, hand it off to another lane, or close that branch. Use `conductor ask {worker_id} \"<follow-up>\"` if you need a narrower follow-up.\n\nLatest report: {summary}"
                ))
            }
        }
    }
}

fn build_verify_handoff_prompt(worker_id: &str, summary: &str) -> String {
    format!(
        "Verify the completion report from {worker_id}. Check commands, evidence, and obvious gaps. Report upward with `conductor report verify-1 \"<short result>\"`.\n\nCompletion report: {summary}"
    )
}

fn build_verify_blocker_prompt(worker_id: &str, summary: &str) -> String {
    format!(
        "Inspect the evidence gap reported by {worker_id}. Check tests, commands, and observable proof. Report upward with `conductor report verify-1 \"<short result>\"`.\n\nBlocked report: {summary}"
    )
}

fn build_dependency_handoff_prompt(worker_id: &str, summary: &str) -> String {
    format!(
        "Unblock the dependency reported by {worker_id}. Inspect the external seam, missing token, service, API, or MCP path, then report upward with `conductor report explore-1 \"<short result>\"`.\n\nBlocked report: {summary}"
    )
}

fn build_scope_reset_prompt(worker_id: &str, summary: &str) -> String {
    format!(
        "Narrow the scope for {worker_id}. Pick one concrete seam, one boundary, or one file cluster only. Report upward with `conductor report {worker_id} \"<short result>\"` after you have a tighter plan.\n\nBlocked report: {summary}"
    )
}

fn infer_worker_profile(worker_id: &str) -> &str {
    worker_id
        .rsplit_once('-')
        .map(|(stem, _)| stem)
        .unwrap_or(worker_id)
}

fn build_relaunch_prompt_for_worker(worker: &WorkerRecord) -> String {
    let summary = worker.current_summary.as_deref().unwrap_or("").trim();
    match worker.reason.as_deref() {
        Some("blocked_scope_reported_to_operator") => {
            build_scope_reset_prompt(&worker.worker_id, summary)
        }
        Some("blocked_dependency_reported_to_operator") => {
            build_dependency_handoff_prompt(&worker.worker_id, summary)
        }
        Some("blocked_evidence_reported_to_operator") => {
            build_verify_blocker_prompt(&worker.worker_id, summary)
        }
        Some("completion_reported_to_operator")
            if !matches!(worker.worker_kind, WorkerKind::Verifier) =>
        {
            build_verify_handoff_prompt(&worker.worker_id, summary)
        }
        Some("handoff_requested_from_lane")
        | Some("handoff_received")
        | Some("handoff_received_for_dependency")
        | Some("handoff_received_for_evidence")
        | Some("handoff_received_for_verification")
        | Some("operator_followup_sent")
        | Some("operator_relaunch_requested")
        | Some("scope_retry_requested") => {
            if summary.is_empty() {
                render_team_starter_prompt(
                    &worker.run_id,
                    &worker.worker_id,
                    infer_worker_profile(&worker.worker_id),
                    None,
                    None,
                )
            } else {
                summary.to_string()
            }
        }
        _ => render_team_starter_prompt(
            &worker.run_id,
            &worker.worker_id,
            infer_worker_profile(&worker.worker_id),
            if summary.is_empty() {
                None
            } else {
                Some(summary)
            },
            None,
        ),
    }
}

fn run_report(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err(
            "report requires <worker_id> <summary> or <run_id> <worker_id> <summary>".to_string(),
        );
    }

    let (report_kind_override, args) = parse_report_kind(args)?;

    let (run_id, worker_id, summary) = if args.len() >= 3 {
        let first = &args[0];
        let second = &args[1];
        if first.chars().all(|ch| ch.is_ascii_digit()) {
            let run_id = default_run_id();
            (run_id, first.clone(), args[1..].join(" "))
        } else if second.contains('-') || second == "main" || second == "surface" {
            (first.clone(), second.clone(), args[2..].join(" "))
        } else {
            let run_id = default_run_id();
            (run_id, first.clone(), args[1..].join(" "))
        }
    } else {
        let run_id = default_run_id();
        (run_id, args[0].clone(), args[1].clone())
    };

    if summary.trim().is_empty() {
        return Err("report summary must not be empty".to_string());
    }

    let store = StateStore::new(resolve_state_root()?);
    let payload = report_to_main(&store, &run_id, &worker_id, &summary, report_kind_override)?;
    print_json(&payload)
}

fn run_ask(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err(
            "ask requires <worker_id> <prompt> or <run_id> <worker_id> <prompt>".to_string(),
        );
    }

    let (run_id, worker_id, prompt) = if args.len() >= 3 {
        let first = &args[0];
        let second = &args[1];
        if second.contains('-')
            || second == "main"
            || second == "surface"
            || second == "orchestrator-main"
        {
            (first.clone(), second.clone(), args[2..].join(" "))
        } else {
            (default_run_id(), first.clone(), args[1..].join(" "))
        }
    } else {
        (default_run_id(), args[0].clone(), args[1].clone())
    };

    if prompt.trim().is_empty() {
        return Err("ask prompt must not be empty".to_string());
    }

    let store = StateStore::new(resolve_state_root()?);
    let payload = ask_worker(&store, &run_id, &worker_id, &prompt)?;
    print_json(&payload)
}

fn run_handoff(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "handoff requires <from_worker> <to_worker> <prompt> or <run_id> <from_worker> <to_worker> <prompt>"
                .to_string(),
        );
    }
    let (run_id, from_worker, to_worker, prompt) = if args.len() >= 4 {
        let first = &args[0];
        let second = &args[1];
        let third = &args[2];
        if second.contains('-') || second == "main" || second == "surface" {
            (
                first.clone(),
                second.clone(),
                third.clone(),
                args[3..].join(" "),
            )
        } else {
            (
                default_run_id(),
                first.clone(),
                second.clone(),
                args[2..].join(" "),
            )
        }
    } else {
        (
            default_run_id(),
            args[0].clone(),
            args[1].clone(),
            args[2].clone(),
        )
    };
    if prompt.trim().is_empty() {
        return Err("handoff prompt must not be empty".to_string());
    }
    let store = StateStore::new(resolve_state_root()?);
    let payload = handoff_worker_lane(&store, &run_id, &from_worker, &to_worker, &prompt)?;
    print_json(&payload)
}

fn parse_report_kind(args: &[String]) -> Result<(Option<WorkerReportKind>, &[String]), String> {
    if args.first().map(String::as_str) != Some("--kind") {
        return Ok((None, args));
    }
    let kind = match args.get(1).map(String::as_str) {
        Some("progress") => WorkerReportKind::Progress,
        Some("blocked") => WorkerReportKind::Blocked,
        Some("done") => WorkerReportKind::Done,
        Some("handoff") => WorkerReportKind::Handoff,
        Some(other) => {
            return Err(format!(
                "report --kind must be progress, blocked, done, or handoff; got {other}"
            ));
        }
        None => return Err("report --kind requires a value".to_string()),
    };
    Ok((Some(kind), &args[2..]))
}

fn run_accept(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err(
            "accept requires <worker_id> <reason> or <run_id> <worker_id> <reason>".to_string(),
        );
    }
    let (run_id, worker_id, reason) = parse_worker_message_args(args, "accept")?;
    if reason.trim().is_empty() {
        return Err("accept reason must not be empty".to_string());
    }
    let store = StateStore::new(resolve_state_root()?);
    let payload = accept_worker_lane(&store, &run_id, &worker_id, &reason)?;
    print_json(&payload)
}

fn run_close(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err(
            "close requires <worker_id> <reason> or <run_id> <worker_id> <reason>".to_string(),
        );
    }
    let (run_id, worker_id, reason) = parse_worker_message_args(args, "close")?;
    if reason.trim().is_empty() {
        return Err("close reason must not be empty".to_string());
    }
    let store = StateStore::new(resolve_state_root()?);
    let payload = close_worker_lane(&store, &run_id, &worker_id, &reason)?;
    print_json(&payload)
}

fn run_relaunch(args: &[String]) -> Result<(), String> {
    let store = StateStore::new(resolve_state_root()?);
    let (run_id, worker_id, prompt) = parse_relaunch_args(&store, args)?;
    let payload = relaunch_worker_lane(&store, &run_id, &worker_id, &prompt)?;
    print_json(&payload)
}

fn parse_relaunch_args(
    store: &StateStore,
    args: &[String],
) -> Result<(String, String, String), String> {
    if args.is_empty() {
        return Err(
            "relaunch requires <worker_id> [prompt] or <run_id> <worker_id> [prompt]".to_string(),
        );
    }
    let default_run = default_run_id();
    let (run_id, worker_id, prompt) = match args {
        [worker_id] => {
            let worker = store.read_worker(&default_run, worker_id)?;
            (
                default_run,
                worker_id.clone(),
                build_relaunch_prompt_for_worker(&worker),
            )
        }
        [run_id, worker_id]
            if worker_id.contains('-')
                || worker_id == "main"
                || worker_id == "surface"
                || worker_id == "orchestrator-main" =>
        {
            let worker = store.read_worker(run_id, worker_id)?;
            (
                run_id.clone(),
                worker_id.clone(),
                build_relaunch_prompt_for_worker(&worker),
            )
        }
        _ => parse_worker_message_args(args, "relaunch")?,
    };
    if prompt.trim().is_empty() {
        return Err("relaunch prompt must not be empty".to_string());
    }
    Ok((run_id, worker_id, prompt))
}

fn parse_worker_message_args(
    args: &[String],
    command_name: &str,
) -> Result<(String, String, String), String> {
    if args.len() < 2 {
        return Err(format!(
            "{command_name} requires <worker_id> <text> or <run_id> <worker_id> <text>"
        ));
    }
    let (run_id, worker_id, text) = if args.len() >= 3 {
        let first = &args[0];
        let second = &args[1];
        if second.contains('-')
            || second == "main"
            || second == "surface"
            || second == "orchestrator-main"
        {
            (first.clone(), second.clone(), args[2..].join(" "))
        } else {
            (default_run_id(), first.clone(), args[1..].join(" "))
        }
    } else {
        (default_run_id(), args[0].clone(), args[1].clone())
    };
    Ok((run_id, worker_id, text))
}

fn run_team_nudge(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "team-nudge requires <run_id> <tmux_session_name>")?;
    let tmux_session_name =
        required_arg(args, 1, "team-nudge requires <run_id> <tmux_session_name>")?;
    let store = StateStore::new(resolve_state_root()?);
    if !tmux_session_exists(tmux_session_name)? {
        return Ok(());
    }
    let panes = run_tmux_capture([
        "list-panes",
        "-t",
        &format!("{tmux_session_name}:0"),
        "-F",
        "#{pane_id}\t#{pane_title}",
    ])?;
    let pane_map = panes
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let pane_id = parts.next()?.trim().to_string();
            let title = parts.next()?.trim().to_string();
            Some((title, pane_id))
        })
        .collect::<BTreeMap<_, _>>();
    for worker_id in store.list_worker_ids(run_id)? {
        if worker_id == "main" || worker_id == "orchestrator-main" {
            continue;
        }
        let worker = match store.read_worker(run_id, &worker_id) {
            Ok(worker) => worker,
            Err(_) => continue,
        };
        let should_nudge = worker.state == WorkerState::Working
            && worker
                .reason
                .as_deref()
                .map(|reason| reason == "direct_team_pane")
                .unwrap_or(false)
            && worker
                .current_summary
                .as_deref()
                .map(|summary| summary.starts_with("direct "))
                .unwrap_or(false);
        if !should_nudge {
            continue;
        }
        if recently_emitted_reason(&store, run_id, Some(&worker_id), "worker_report_nudged", 20)? {
            continue;
        }
        if let Some(pane_id) = pane_map.get(&worker_id) {
            let prompt = build_team_report_nudge_prompt(&worker_id);
            let _ = send_prompt_to_tmux_pane(pane_id, &prompt);
            let mut worker = worker;
            let (next_reason, stalled) = advance_team_nudge_reason(worker.reason.as_deref());
            worker.reason = Some(next_reason.to_string());
            worker.last_event_at = Some(Utc::now());
            let _ = store.upsert_worker(worker);
            let _ = store.append_runtime_event(
                run_id,
                EventEnvelope {
                    schema_version: SCHEMA_VERSION,
                    event: EventKind::WorkerStateChanged,
                    timestamp: Utc::now(),
                    run_id: Some(run_id.to_string()),
                    session_id: None,
                    source: "team-nudge".to_string(),
                    worker: Some(worker_id.clone()),
                    task_id: None,
                    message_id: None,
                    reason: Some("worker_report_nudged".to_string()),
                    context: serde_json::Map::from_iter([("prompt".to_string(), json!(prompt))]),
                },
            );
            if stalled {
                if !recently_emitted_reason(
                    &store,
                    run_id,
                    Some(&worker_id),
                    "worker_stalled_waiting_for_report",
                    60,
                )? {
                    let _ =
                        push_text_to_main_pane(run_id, &build_stalled_worker_prompt(&worker_id));
                    let _ = store.append_runtime_event(
                        run_id,
                        EventEnvelope {
                            schema_version: SCHEMA_VERSION,
                            event: EventKind::WorkerStateChanged,
                            timestamp: Utc::now(),
                            run_id: Some(run_id.to_string()),
                            session_id: None,
                            source: "team-nudge".to_string(),
                            worker: Some(worker_id.clone()),
                            task_id: None,
                            message_id: None,
                            reason: Some("worker_stalled_waiting_for_report".to_string()),
                            context: serde_json::Map::new(),
                        },
                    );
                }
            }
        }
    }
    Ok(())
}

fn run_team_followup(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "team-followup requires <run_id> <tmux_session_name>",
    )?;
    let tmux_session_name = required_arg(
        args,
        1,
        "team-followup requires <run_id> <tmux_session_name>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    if !tmux_session_exists(tmux_session_name)? {
        return Ok(());
    }

    let _ = reclaim_expired_claims(&store, run_id);
    run_team_nudge(args)?;

    maybe_prompt_all_workers_idle(&store, run_id, "team-followup")
}

fn report_to_main(
    store: &StateStore,
    run_id: &str,
    worker_id: &str,
    summary: &str,
    report_kind_override: Option<WorkerReportKind>,
) -> Result<serde_json::Value, String> {
    let mut worker = store.read_worker(&run_id, &worker_id)?;
    let now = Utc::now();
    let (next_state, next_reason, report_kind) = report_kind_override
        .map(|kind| classify_structured_worker_report(kind, summary))
        .unwrap_or_else(|| classify_worker_report(summary));
    let next_state = match (report_kind, worker.worker_kind.clone()) {
        (WorkerReportKind::Done, WorkerKind::Verifier) => WorkerState::VerifiedComplete,
        _ => next_state,
    };
    worker.current_summary = Some(summary.to_string());
    worker.state = next_state;
    worker.last_event_at = Some(now);
    worker.last_stdout_at = Some(now);
    worker.reason = Some(next_reason);
    let worker = store.upsert_worker(worker)?;

    let message_id = format!("report-{}-{}", worker_id, now.timestamp_millis());
    let message =
        store.create_mailbox_message(&run_id, &message_id, &worker_id, "main", &summary)?;
    let _ = store.update_mailbox_status(&run_id, "main", &message_id, false)?;
    let event = EventEnvelope {
        schema_version: SCHEMA_VERSION,
        event: EventKind::MailboxMessageCreated,
        timestamp: now,
        run_id: Some(run_id.to_string()),
        session_id: None,
        source: "report".to_string(),
        worker: Some(worker_id.to_string()),
        task_id: worker.current_task_id.clone(),
        message_id: Some(message_id.clone()),
        reason: Some("worker_reported_to_main".to_string()),
        context: serde_json::Map::from_iter([
            ("summary".to_string(), json!(summary)),
            ("to_worker".to_string(), json!("main")),
        ]),
    };
    store.append_runtime_event(&run_id, event)?;
    if push_worker_report_to_main_pane(run_id, worker_id, summary).unwrap_or(false) {
        let _ = store.update_mailbox_status(&run_id, "main", &message_id, true)?;
        if let Some(followup) = build_operator_followup_prompt(worker_id, summary, report_kind) {
            let _ = push_text_to_main_pane(run_id, &followup);
        }
    } else {
        store.append_runtime_event(
            &run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: EventKind::LeaderNotificationDeferred,
                timestamp: now,
                run_id: Some(run_id.to_string()),
                session_id: None,
                source: "report".to_string(),
                worker: Some(worker_id.to_string()),
                task_id: worker.current_task_id.clone(),
                message_id: Some(message_id.clone()),
                reason: Some("main_pane_unavailable".to_string()),
                context: serde_json::Map::from_iter([
                    ("summary".to_string(), json!(summary)),
                    ("to_worker".to_string(), json!("main")),
                ]),
            },
        )?;
    }
    match report_kind {
        WorkerReportKind::Done => {
            let _ = maybe_handoff_completion_to_verify(store, run_id, &worker, summary);
        }
        WorkerReportKind::Blocked => {
            let _ = maybe_handoff_blocked_lane(store, run_id, &worker, summary);
        }
        WorkerReportKind::Handoff => {}
        WorkerReportKind::Progress => {}
    }
    let _ = maybe_prompt_all_workers_idle(store, run_id, "report");

    Ok(json!({
        "run_id": run_id,
        "worker_id": worker_id,
        "summary": summary,
        "message_id": message.message_id,
        "mailbox_target": "main"
    }))
}

fn maybe_handoff_blocked_lane(
    store: &StateStore,
    run_id: &str,
    blocked_worker: &WorkerRecord,
    summary: &str,
) -> Result<bool, String> {
    match infer_blocked_kind(&summary.to_ascii_lowercase()) {
        BlockedKind::Approval => {
            if let Some(task_id) = blocked_worker.current_task_id.as_deref() {
                let _ = store.update_task_approval(
                    run_id,
                    task_id,
                    Some(ApprovalStatus::Pending),
                    None,
                    Some(summary.to_string()),
                )?;
            }
            let _ = push_text_to_main_pane(
                run_id,
                &format!("{} -> operator: approval needed", blocked_worker.worker_id),
            );
            Ok(true)
        }
        BlockedKind::Evidence => {
            let verify_worker_id = store
                .list_worker_ids(run_id)?
                .into_iter()
                .find(|worker_id| worker_id.starts_with("verify-"));
            let Some(verify_worker_id) = verify_worker_id else {
                return Ok(false);
            };
            let prompt = build_verify_blocker_prompt(&blocked_worker.worker_id, summary);
            let payload = ask_worker(store, run_id, &verify_worker_id, &prompt)?;
            let delivered = payload
                .get("delivered")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let _ = append_handoff_event(
                store,
                run_id,
                &blocked_worker.worker_id,
                &verify_worker_id,
                "blocked_evidence_handoff",
                &prompt,
            );
            if delivered {
                let _ = mark_worker_reason(
                    store,
                    run_id,
                    &verify_worker_id,
                    "handoff_received_for_evidence",
                );
            }
            let _ = push_text_to_main_pane(
                run_id,
                &format!(
                    "{} -> {}: inspect the missing evidence",
                    blocked_worker.worker_id, verify_worker_id
                ),
            );
            Ok(delivered)
        }
        BlockedKind::Dependency => {
            let explore_worker_id = store
                .list_worker_ids(run_id)?
                .into_iter()
                .find(|worker_id| {
                    worker_id.starts_with("explore-") && worker_id != &blocked_worker.worker_id
                });
            let Some(explore_worker_id) = explore_worker_id else {
                return Ok(false);
            };
            let prompt = build_dependency_handoff_prompt(&blocked_worker.worker_id, summary);
            let payload = ask_worker(store, run_id, &explore_worker_id, &prompt)?;
            let delivered = payload
                .get("delivered")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let _ = append_handoff_event(
                store,
                run_id,
                &blocked_worker.worker_id,
                &explore_worker_id,
                "blocked_dependency_handoff",
                &prompt,
            );
            if delivered {
                let _ = mark_worker_reason(
                    store,
                    run_id,
                    &explore_worker_id,
                    "handoff_received_for_dependency",
                );
            }
            let _ = push_text_to_main_pane(
                run_id,
                &format!(
                    "{} -> {}: inspect the blocked dependency",
                    blocked_worker.worker_id, explore_worker_id
                ),
            );
            Ok(delivered)
        }
        BlockedKind::Scope => {
            let prompt = build_scope_reset_prompt(&blocked_worker.worker_id, summary);
            let payload = ask_worker(store, run_id, &blocked_worker.worker_id, &prompt)?;
            let delivered = payload
                .get("delivered")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if delivered {
                let _ = mark_worker_reason(
                    store,
                    run_id,
                    &blocked_worker.worker_id,
                    "scope_retry_requested",
                );
            }
            let _ = push_text_to_main_pane(
                run_id,
                &format!(
                    "{} -> {}: narrow the lane scope and retry",
                    blocked_worker.worker_id, blocked_worker.worker_id
                ),
            );
            Ok(delivered)
        }
        _ => Ok(false),
    }
}

fn maybe_handoff_completion_to_verify(
    store: &StateStore,
    run_id: &str,
    completed_worker: &WorkerRecord,
    summary: &str,
) -> Result<bool, String> {
    if matches!(completed_worker.worker_kind, WorkerKind::Verifier) {
        return Ok(false);
    }

    let verify_worker_id = store
        .list_worker_ids(run_id)?
        .into_iter()
        .find(|worker_id| worker_id.starts_with("verify-"));
    let Some(verify_worker_id) = verify_worker_id else {
        return Ok(false);
    };

    let prompt = build_verify_handoff_prompt(&completed_worker.worker_id, summary);
    let payload = ask_worker(store, run_id, &verify_worker_id, &prompt)?;
    let delivered = payload
        .get("delivered")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let _ = append_handoff_event(
        store,
        run_id,
        &completed_worker.worker_id,
        &verify_worker_id,
        "completion_verification_handoff",
        &prompt,
    );
    if delivered {
        let _ = mark_worker_reason(
            store,
            run_id,
            &verify_worker_id,
            "handoff_received_for_verification",
        );
    }
    let _ = push_text_to_main_pane(
        run_id,
        &format!(
            "{} -> {}: verify this completion",
            completed_worker.worker_id, verify_worker_id
        ),
    );
    Ok(delivered)
}

fn ask_worker(
    store: &StateStore,
    run_id: &str,
    worker_id: &str,
    prompt: &str,
) -> Result<serde_json::Value, String> {
    let mut worker = store.read_worker(run_id, worker_id)?;
    let now = Utc::now();
    worker.last_event_at = Some(now);
    if !matches!(worker.state, WorkerState::Failed | WorkerState::Stopped) {
        worker.state = WorkerState::AwaitingReport;
    }
    worker.reason = Some("operator_followup_sent".to_string());
    let worker = store.upsert_worker(worker)?;

    let message_id = format!("ask-{}-{}", worker_id, now.timestamp_millis());
    let message = store.create_mailbox_message(run_id, &message_id, "main", worker_id, prompt)?;
    let _ = store.update_mailbox_status(run_id, worker_id, &message_id, false)?;

    let delivered = deliver_operator_followup(store, run_id, &worker, prompt)?;
    if delivered {
        let _ = store.update_mailbox_status(run_id, worker_id, &message_id, true)?;
    } else {
        store.append_runtime_event(
            run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: EventKind::LeaderNotificationDeferred,
                timestamp: now,
                run_id: Some(run_id.to_string()),
                session_id: worker.session_ref.clone(),
                source: "ask".to_string(),
                worker: Some(worker_id.to_string()),
                task_id: worker.current_task_id.clone(),
                message_id: Some(message_id.clone()),
                reason: Some("worker_lane_unavailable".to_string()),
                context: serde_json::Map::from_iter([
                    ("prompt".to_string(), json!(prompt)),
                    ("to_worker".to_string(), json!(worker_id)),
                ]),
            },
        )?;
    }

    Ok(json!({
        "run_id": run_id,
        "worker_id": worker_id,
        "prompt": prompt,
        "message_id": message.message_id,
        "delivered": delivered
    }))
}

fn accept_worker_lane(
    store: &StateStore,
    run_id: &str,
    worker_id: &str,
    reason: &str,
) -> Result<serde_json::Value, String> {
    let worker = store.read_worker(run_id, worker_id)?;
    let now = Utc::now();
    if let Some(task_id) = worker.current_task_id.as_deref() {
        let _ = store.complete_task(
            run_id,
            task_id,
            reason,
            json!({
                "accepted_by": "operator",
                "worker_id": worker_id,
                "reason": reason,
            }),
        );
    }
    let closed = settle_worker_lane(store, run_id, &worker, "accepted_by_operator", Some(reason))?;
    let _ = maybe_complete_run_after_operator_settlement(store, run_id, worker_id);
    let _ = push_text_to_main_pane(run_id, &format!("{worker_id}: accepted ({reason})"));
    Ok(json!({
        "run_id": run_id,
        "worker_id": worker_id,
        "accepted": true,
        "closed": closed,
        "reason": reason,
        "timestamp": now,
    }))
}

fn close_worker_lane(
    store: &StateStore,
    run_id: &str,
    worker_id: &str,
    reason: &str,
) -> Result<serde_json::Value, String> {
    let worker = store.read_worker(run_id, worker_id)?;
    let now = Utc::now();
    let closed = settle_worker_lane(store, run_id, &worker, "closed_by_operator", Some(reason))?;
    let _ = maybe_complete_run_after_operator_settlement(store, run_id, worker_id);
    let _ = push_text_to_main_pane(run_id, &format!("{worker_id}: closed ({reason})"));
    Ok(json!({
        "run_id": run_id,
        "worker_id": worker_id,
        "closed": closed,
        "reason": reason,
        "timestamp": now,
    }))
}

fn relaunch_worker_lane(
    store: &StateStore,
    run_id: &str,
    worker_id: &str,
    prompt: &str,
) -> Result<serde_json::Value, String> {
    let mut payload = ask_worker(store, run_id, worker_id, prompt)?;
    let delivered = payload
        .get("delivered")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if !delivered {
        let respawned = respawn_direct_team_worker_pane(store, run_id, worker_id, prompt)?;
        if respawned {
            payload["delivered"] = json!(true);
            payload["respawned"] = json!(true);
        }
    }
    if let Ok(mut worker) = store.read_worker(run_id, worker_id) {
        worker.reason = Some("operator_relaunch_requested".to_string());
        worker.last_event_at = Some(Utc::now());
        let _ = store.upsert_worker(worker);
    }
    let _ = push_text_to_main_pane(run_id, &format!("{worker_id}: relaunched"));
    Ok(payload)
}

fn handoff_worker_lane(
    store: &StateStore,
    run_id: &str,
    from_worker: &str,
    to_worker: &str,
    prompt: &str,
) -> Result<serde_json::Value, String> {
    let handoff_prompt = format!(
        "Handoff from {from_worker}. Stay in your lane, pick up only this narrow follow-up, and report upward with `conductor report {to_worker} \"<short result>\"`.\n\n{prompt}"
    );
    let payload = ask_worker(store, run_id, to_worker, &handoff_prompt)?;
    let delivered = payload
        .get("delivered")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let _ = append_handoff_event(
        store,
        run_id,
        from_worker,
        to_worker,
        "worker_to_worker_handoff",
        prompt,
    );
    if delivered {
        let _ = mark_worker_reason(store, run_id, to_worker, "handoff_received");
    }
    let _ = push_text_to_main_pane(run_id, &format!("{from_worker} -> {to_worker}: handoff"));
    Ok(json!({
        "run_id": run_id,
        "from_worker": from_worker,
        "to_worker": to_worker,
        "prompt": prompt,
        "delivered": delivered
    }))
}

fn mark_worker_reason(
    store: &StateStore,
    run_id: &str,
    worker_id: &str,
    reason: &str,
) -> Result<(), String> {
    let mut worker = store.read_worker(run_id, worker_id)?;
    worker.reason = Some(reason.to_string());
    worker.last_event_at = Some(Utc::now());
    store.upsert_worker(worker)?;
    Ok(())
}

fn append_handoff_event(
    store: &StateStore,
    run_id: &str,
    from_worker: &str,
    to_worker: &str,
    reason: &str,
    prompt: &str,
) -> Result<(), String> {
    store.append_runtime_event(
        run_id,
        EventEnvelope {
            schema_version: SCHEMA_VERSION,
            event: EventKind::HandoffNeeded,
            timestamp: Utc::now(),
            run_id: Some(run_id.to_string()),
            session_id: None,
            source: "handoff".to_string(),
            worker: Some(to_worker.to_string()),
            task_id: None,
            message_id: None,
            reason: Some(reason.to_string()),
            context: serde_json::Map::from_iter([
                ("from_worker".to_string(), json!(from_worker)),
                ("to_worker".to_string(), json!(to_worker)),
                ("prompt".to_string(), json!(prompt)),
            ]),
        },
    )
}

fn settle_worker_lane(
    store: &StateStore,
    run_id: &str,
    worker: &WorkerRecord,
    reason_tag: &str,
    summary_override: Option<&str>,
) -> Result<bool, String> {
    let now = Utc::now();
    let mut next = worker.clone();
    next.state = WorkerState::Stopped;
    next.last_event_at = Some(now);
    next.reason = Some(reason_tag.to_string());
    if let Some(summary) = summary_override {
        next.current_summary = Some(summary.to_string());
    }
    next.current_task_id = None;
    store.upsert_worker(next)?;

    if let Some(pane_id) = find_worker_tmux_pane_id(run_id, &worker.worker_id)? {
        run_tmux(["kill-pane", "-t", &pane_id])?;
        return Ok(true);
    }

    if let Some(session_id) = worker.session_ref.as_deref() {
        if let Ok(session) = store.read_session(run_id, session_id) {
            let response =
                send_session_command(Path::new(&session.socket_path), &SessionCommand::Stop)?;
            return Ok(response.ok);
        }
    }

    Ok(false)
}

fn maybe_complete_run_after_operator_settlement(
    store: &StateStore,
    run_id: &str,
    worker_id: &str,
) -> Result<bool, String> {
    let snapshot = store.refresh_snapshot(run_id)?;
    let has_live_lane = snapshot.workers.iter().any(|worker| {
        worker.worker_id != "main"
            && worker.worker_id != "orchestrator-main"
            && matches!(
                worker.state,
                WorkerState::Working
                    | WorkerState::AwaitingReport
                    | WorkerState::Blocked
                    | WorkerState::Idle
            )
    });
    if has_live_lane
        || snapshot.tasks.pending > 0
        || snapshot.tasks.blocked > 0
        || snapshot.tasks.in_progress > 0
        || snapshot.readiness.pending_approvals > 0
    {
        return Ok(false);
    }

    let ralph_enabled = read_ralph_loop_state_from_root(store.root(), run_id)
        .map(|state| state.enabled)
        .unwrap_or(false);
    if ralph_enabled {
        let settled = store.read_worker(run_id, worker_id).ok();
        let settled_reason = settled
            .as_ref()
            .and_then(|worker| worker.reason.as_deref())
            .unwrap_or_default();
        if settled_reason != "closed_by_operator" {
            let _ = push_text_to_main_pane(
                run_id,
                "ralph: lanes settled, but the loop stays active until you explicitly close the run",
            );
            return Ok(false);
        }
    }

    let mut run = store.read_run(run_id)?;
    run.active = false;
    run.current_phase = RunPhase::Complete;
    run.updated_at = Utc::now();
    run.completed_at = Some(Utc::now());
    run.stop_reason = Some(format!(
        "all lanes settled after operator closed {worker_id}"
    ));
    store.write_run(&run)?;
    let _ = disable_ralph_loop_for_root(store.root(), run_id);
    let _ = store.refresh_snapshot(run_id)?;
    let _ = push_text_to_main_pane(
        run_id,
        "run: all lanes settled; closing the orchestration loop",
    );
    Ok(true)
}

fn respawn_direct_team_worker_pane(
    store: &StateStore,
    run_id: &str,
    worker_id: &str,
    prompt: &str,
) -> Result<bool, String> {
    let worker = store.read_worker(run_id, worker_id)?;
    let worker_stem = worker_id
        .rsplit_once('-')
        .map(|(stem, _)| stem)
        .unwrap_or(worker_id);
    let reason = worker.reason.as_deref().unwrap_or("");
    let terminal_label = worker.terminal_label.as_deref().unwrap_or("");
    if !reason.contains("direct") && terminal_label != worker_id {
        return Ok(false);
    }

    let tmux_session_name = current_tmux_session_hint()
        .filter(|session_name| tmux_session_exists(session_name).unwrap_or(false))
        .unwrap_or_else(|| surface_tmux_session_name(run_id));
    if !tmux_session_exists(&tmux_session_name)? {
        return Ok(false);
    }

    let (_, cfg) = load_resolved_config()?;
    let Some(worker_type) = cfg
        .workers
        .keys()
        .find(|profile| sanitize_worker_stem(profile) == worker_stem)
    else {
        return Ok(false);
    };
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let launch = resolve_worker_adapter(&adapter, run_id, worker_id, None, None)?;
    let use_inline_prompt = launch_uses_inline_prompt(&launch);
    let pane_command =
        build_direct_launch_shell_command(&cwd, &launch, use_inline_prompt.then_some(prompt));
    let new_pane_id = run_tmux_capture([
        "split-window",
        "-h",
        "-d",
        "-P",
        "-F",
        "#{pane_id}",
        "-t",
        &format!("{tmux_session_name}:0.0"),
        &pane_command,
    ])?;
    let new_pane_id = new_pane_id.trim().to_string();
    run_tmux(["select-pane", "-t", &new_pane_id, "-T", worker_id])?;
    if !use_inline_prompt {
        send_prompt_to_tmux_pane(&new_pane_id, prompt)?;
    }

    let mut next = worker;
    next.state = WorkerState::AwaitingReport;
    next.last_event_at = Some(Utc::now());
    next.last_heartbeat_at = Some(Utc::now());
    next.reason = Some("direct_team_pane_respawned".to_string());
    next.current_summary = Some(format!("direct {} pane relaunched", worker_type));
    next.terminal_label = Some(worker_id.to_string());
    store.upsert_worker(next)?;
    let _ = store.append_runtime_event(
        run_id,
        EventEnvelope {
            schema_version: SCHEMA_VERSION,
            event: EventKind::WorkerBootstrapStarted,
            timestamp: Utc::now(),
            run_id: Some(run_id.to_string()),
            session_id: None,
            source: "relaunch".to_string(),
            worker: Some(worker_id.to_string()),
            task_id: None,
            message_id: None,
            reason: Some("direct_team_pane_respawned".to_string()),
            context: serde_json::Map::from_iter([
                ("worker_type".to_string(), json!(worker_type)),
                ("prompt".to_string(), json!(prompt)),
            ]),
        },
    );
    rebalance_tmux_team_layout(&tmux_session_name)?;
    Ok(true)
}

fn deliver_operator_followup(
    store: &StateStore,
    run_id: &str,
    worker: &WorkerRecord,
    prompt: &str,
) -> Result<bool, String> {
    if let Some(pane_id) = find_worker_tmux_pane_id(run_id, &worker.worker_id)? {
        send_prompt_to_tmux_pane(&pane_id, prompt)?;
        return Ok(true);
    }

    if let Some(session_id) = worker.session_ref.as_deref() {
        let session = store.read_session(run_id, session_id)?;
        let response = send_session_command(
            Path::new(&session.socket_path),
            &SessionCommand::SendStdin {
                data: format!("{prompt}\n"),
            },
        )?;
        return Ok(response.ok);
    }

    Ok(false)
}

fn push_worker_report_to_main_pane(
    run_id: &str,
    worker_id: &str,
    summary: &str,
) -> Result<bool, String> {
    let prompt = build_operator_report_prompt(worker_id, summary);
    push_text_to_main_pane(run_id, &prompt)
}

fn push_text_to_main_pane(run_id: &str, prompt: &str) -> Result<bool, String> {
    let session_name = current_tmux_session_hint()
        .filter(|session_name| tmux_session_exists(session_name).unwrap_or(false))
        .unwrap_or_else(|| surface_tmux_session_name(run_id));
    if !tmux_session_exists(&session_name)? {
        return Ok(false);
    }
    let panes = run_tmux_capture([
        "list-panes",
        "-t",
        &format!("{session_name}:0"),
        "-F",
        "#{pane_id}\t#{pane_index}\t#{pane_title}\t#{pane_current_command}",
    ])?;
    let main_pane_id = find_main_pane_id(&session_name, &panes)?;
    send_prompt_to_tmux_pane(&main_pane_id, &prompt)?;
    Ok(true)
}

fn build_operator_report_prompt(worker_id: &str, summary: &str) -> String {
    format!("{worker_id}: {summary}")
}

fn find_worker_tmux_pane_id(run_id: &str, worker_id: &str) -> Result<Option<String>, String> {
    let session_name = current_tmux_session_hint()
        .filter(|session_name| tmux_session_exists(session_name).unwrap_or(false))
        .unwrap_or_else(|| surface_tmux_session_name(run_id));
    if !tmux_session_exists(&session_name)? {
        return Ok(None);
    }
    let panes = run_tmux_capture([
        "list-panes",
        "-t",
        &format!("{session_name}:0"),
        "-F",
        "#{pane_id}\t#{pane_title}",
    ])?;
    Ok(panes.lines().find_map(|line| {
        let mut parts = line.splitn(2, '\t');
        let pane_id = parts.next()?.trim().to_string();
        let pane_title = parts.next()?.trim();
        if pane_title == worker_id {
            Some(pane_id)
        } else {
            None
        }
    }))
}

fn schedule_team_report_nudge(run_id: &str, tmux_session_name: &str) -> Result<(), String> {
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let state_root = resolve_state_root()?;
    let config_path = resolve_config_path().ok();
    let mut env_parts = vec![format!(
        "CONDUCTOR_STATE_DIR={}",
        shell_quote_str(&state_root.display().to_string())
    )];
    if let Some(config_path) = config_path {
        env_parts.push(format!(
            "CONDUCTOR_CONFIG={}",
            shell_quote_str(&config_path.display().to_string())
        ));
    }
    let command = format!(
        "cd {} && {} {} team-nudge {} {}",
        shell_quote(&cwd),
        env_parts.join(" "),
        shell_quote(&conductor_bin),
        shell_quote_str(run_id),
        shell_quote_str(tmux_session_name),
    );
    for delay in [20_u64, 50_u64, 80_u64] {
        let script = format!("sleep {delay}; {command}");
        run_tmux(["run-shell", "-b", &script])?;
    }
    Ok(())
}

fn schedule_team_followup_loop(run_id: &str, tmux_session_name: &str) -> Result<(), String> {
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let state_root = resolve_state_root()?;
    let config_path = resolve_config_path().ok();
    let mut env_parts = vec![format!(
        "CONDUCTOR_STATE_DIR={}",
        shell_quote_str(&state_root.display().to_string())
    )];
    if let Some(config_path) = config_path {
        env_parts.push(format!(
            "CONDUCTOR_CONFIG={}",
            shell_quote_str(&config_path.display().to_string())
        ));
    }
    let command = format!(
        "cd {} && {} {} team-followup {} {}",
        shell_quote(&cwd),
        env_parts.join(" "),
        shell_quote(&conductor_bin),
        shell_quote_str(run_id),
        shell_quote_str(tmux_session_name),
    );
    for delay in [25_u64, 55_u64, 85_u64, 115_u64, 145_u64] {
        let script = format!("sleep {delay}; {command}");
        run_tmux(["run-shell", "-b", &script])?;
    }
    Ok(())
}

fn recently_prompted_all_idle(
    store: &StateStore,
    run_id: &str,
    cooldown_secs: i64,
) -> Result<bool, String> {
    recently_emitted_reason(
        store,
        run_id,
        Some("main"),
        "all_workers_idle_prompted",
        cooldown_secs,
    )
}

fn recently_emitted_reason(
    store: &StateStore,
    run_id: &str,
    worker_id: Option<&str>,
    reason: &str,
    cooldown_secs: i64,
) -> Result<bool, String> {
    let cutoff = Utc::now() - chrono::Duration::seconds(cooldown_secs);
    Ok(store.read_events(run_id)?.iter().rev().any(|event| {
        event.timestamp >= cutoff
            && event.reason.as_deref() == Some(reason)
            && worker_id
                .map(|target| event.worker.as_deref() == Some(target))
                .unwrap_or(true)
    }))
}

fn maybe_prompt_all_workers_idle(
    store: &StateStore,
    run_id: &str,
    source: &str,
) -> Result<(), String> {
    let workers = store
        .list_worker_ids(run_id)?
        .into_iter()
        .filter(|worker_id| worker_id != "main" && worker_id != "orchestrator-main")
        .filter_map(|worker_id| store.read_worker(run_id, &worker_id).ok())
        .collect::<Vec<_>>();
    if workers.is_empty() {
        return Ok(());
    }

    let all_idle = workers.iter().all(|worker| {
        matches!(
            worker.state,
            WorkerState::Idle
                | WorkerState::Done
                | WorkerState::DonePendingVerification
                | WorkerState::VerifiedComplete
                | WorkerState::Stopped
                | WorkerState::Unknown
        )
    });
    if !all_idle || recently_prompted_all_idle(store, run_id, 60)? {
        return Ok(());
    }

    let summaries = workers
        .iter()
        .filter_map(|worker| {
            let summary = worker.current_summary.as_deref()?.trim();
            if summary.is_empty() || summary.starts_with("direct ") {
                return None;
            }
            Some((worker.worker_id.clone(), summary.to_string()))
        })
        .collect::<Vec<_>>();
    if summaries.is_empty() {
        return Ok(());
    }

    let prompt = build_all_workers_idle_prompt(run_id, &summaries);
    let now = Utc::now();
    let message_id = format!("all-idle-{}", now.timestamp_millis());
    let message = store.create_mailbox_message(run_id, &message_id, "runtime", "main", &prompt)?;
    let _ = store.update_mailbox_status(run_id, "main", &message_id, false)?;
    let event = EventEnvelope {
        schema_version: SCHEMA_VERSION,
        event: EventKind::MailboxMessageCreated,
        timestamp: now,
        run_id: Some(run_id.to_string()),
        session_id: None,
        source: source.to_string(),
        worker: Some("main".to_string()),
        task_id: None,
        message_id: Some(message.message_id.clone()),
        reason: Some("all_workers_idle_prompted".to_string()),
        context: serde_json::Map::new(),
    };
    store.append_runtime_event(run_id, event)?;
    if push_text_to_main_pane(run_id, &prompt).unwrap_or(false) {
        let _ = store.update_mailbox_status(run_id, "main", &message_id, true)?;
    } else {
        store.append_runtime_event(
            run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: EventKind::LeaderNotificationDeferred,
                timestamp: now,
                run_id: Some(run_id.to_string()),
                session_id: None,
                source: source.to_string(),
                worker: Some("main".to_string()),
                task_id: None,
                message_id: Some(message_id.clone()),
                reason: Some("main_pane_unavailable".to_string()),
                context: serde_json::Map::from_iter([(
                    "deferred_prompt".to_string(),
                    json!(prompt),
                )]),
            },
        )?;
    }
    Ok(())
}

fn rebuild_direct_team_surface(
    session_name: &str,
    pane_specs: &[OpsPaneSpec],
) -> Result<(), String> {
    let panes = run_tmux_capture([
        "list-panes",
        "-t",
        &format!("{session_name}:0"),
        "-F",
        "#{pane_id}\t#{pane_index}\t#{pane_title}\t#{pane_current_command}",
    ])?;
    let main_pane_id = find_main_pane_id(session_name, &panes)?;
    let mut extra_panes = Vec::new();
    for line in panes.lines() {
        let mut parts = line.splitn(4, '\t');
        let pane_id = parts.next().unwrap_or_default().trim().to_string();
        if pane_id != main_pane_id && !pane_id.is_empty() {
            extra_panes.push(pane_id);
        }
    }

    for pane_id in extra_panes {
        let _ = run_tmux(["kill-pane", "-t", &pane_id]);
    }
    if pane_specs.is_empty() {
        run_tmux(["select-pane", "-t", &main_pane_id])?;
        return Ok(());
    }

    let window_width = run_tmux_capture([
        "display-message",
        "-p",
        "-t",
        &format!("{session_name}:0"),
        "#{window_width}",
    ])?
    .trim()
    .parse::<u64>()
    .unwrap_or(0);
    let worker_width = if window_width > 0 {
        std::cmp::max(36_u64, (window_width * 38) / 100)
    } else {
        48
    };

    let first_pane_id = run_tmux_capture([
        "split-window",
        "-h",
        "-d",
        "-P",
        "-F",
        "#{pane_id}",
        "-l",
        &worker_width.to_string(),
        "-t",
        &main_pane_id,
        &pane_specs[0].command,
    ])?
    .trim()
    .to_string();
    run_tmux([
        "select-pane",
        "-t",
        &first_pane_id,
        "-T",
        &pane_specs[0].title,
    ])?;
    if let Some(prompt) = &pane_specs[0].starter_prompt {
        send_prompt_to_tmux_pane(&first_pane_id, prompt)?;
    }

    let mut worker_pane_ids = vec![first_pane_id.clone()];
    for spec in pane_specs.iter().skip(1) {
        let split_target =
            tallest_tmux_pane(&worker_pane_ids)?.unwrap_or_else(|| first_pane_id.clone());
        let current_target_height = tmux_pane_height(&split_target)?;
        let desired_height = if current_target_height > 0 {
            std::cmp::max(3_u64, current_target_height / 2)
        } else {
            8
        };
        let new_pane_id = run_tmux_capture([
            "split-window",
            "-v",
            "-d",
            "-P",
            "-F",
            "#{pane_id}",
            "-l",
            &desired_height.to_string(),
            "-t",
            &split_target,
            &spec.command,
        ])?
        .trim()
        .to_string();
        run_tmux(["select-pane", "-t", &new_pane_id, "-T", &spec.title])?;
        if let Some(prompt) = &spec.starter_prompt {
            send_prompt_to_tmux_pane(&new_pane_id, prompt)?;
        }
        worker_pane_ids.push(new_pane_id);
    }

    run_tmux(["select-pane", "-t", &main_pane_id])?;
    Ok(())
}

fn tmux_pane_height(pane_id: &str) -> Result<u64, String> {
    run_tmux_capture(["display-message", "-p", "-t", pane_id, "#{pane_height}"])?
        .trim()
        .parse::<u64>()
        .map_err(|err| err.to_string())
}

fn tallest_tmux_pane(pane_ids: &[String]) -> Result<Option<String>, String> {
    let mut tallest = None::<(String, u64)>;
    for pane_id in pane_ids {
        let height = tmux_pane_height(pane_id)?;
        match &tallest {
            Some((_, current_height)) if *current_height >= height => {}
            _ => tallest = Some((pane_id.clone(), height)),
        }
    }
    Ok(tallest.map(|(pane_id, _)| pane_id))
}

fn run_surface_ops_open(run_id: &str, resume_surface: bool) -> Result<(), String> {
    let tmux_session_name = surface_tmux_session_name(run_id);
    let (_, cfg) = load_resolved_config()?;
    let store = StateStore::new(resolve_state_root()?);
    stop_worker_session_if_present(&store, run_id, "main");
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let state_root = store.root().to_path_buf();
    let config_path = resolve_config_path().ok();
    let hud_cmd = build_hud_shell_command(
        &cwd,
        &conductor_bin,
        &state_root,
        config_path.as_deref(),
        run_id,
    );
    let hud_status_cmd = build_hud_status_command(
        &cwd,
        &conductor_bin,
        &state_root,
        config_path.as_deref(),
        run_id,
    );
    let launch = resolve_surface_launch(&cfg, run_id, resume_surface)?;
    let uses_native_codex_resume = resume_surface && cfg.surface.cli == "codex";
    let surface_cmd = build_launch_shell_command(&cwd, &launch);
    let pane_specs = vec![OpsPaneSpec {
        title: "main".to_string(),
        command: surface_cmd,
        starter_prompt: if uses_native_codex_resume {
            None
        } else {
            maybe_resume_context_prompt(&store, run_id, resume_surface)
        },
    }];
    if tmux_session_exists(&tmux_session_name)? {
        run_tmux(["kill-session", "-t", &tmux_session_name])?;
    }
    if command_available("tmux")
        && !has_live_tmux_client()
        && env::var("CONDUCTOR_OPS_NO_ATTACH").ok().as_deref() != Some("1")
    {
        return run_surface_attached_tmux_session(
            &tmux_session_name,
            &hud_status_cmd,
            pane_specs[0].title.as_str(),
            pane_specs[0].command.as_str(),
            pane_specs[0].starter_prompt.as_deref(),
        );
    }
    ensure_tmux_ops_session(&tmux_session_name, &hud_cmd, &pane_specs)?;
    attach_tmux_ops_session(&tmux_session_name)
}

fn run_surface_attached_tmux_session(
    session_name: &str,
    hud_status_cmd: &str,
    main_title: &str,
    surface_cmd: &str,
    starter_prompt: Option<&str>,
) -> Result<(), String> {
    run_tmux([
        "new-session",
        "-d",
        "-s",
        session_name,
        "-n",
        "ops",
        surface_cmd,
    ])?;
    run_tmux(["set-option", "-t", session_name, "mouse", "on"])?;
    run_tmux(["set-option", "-t", session_name, "set-clipboard", "on"])?;
    let main_pane_id = run_tmux_capture([
        "display-message",
        "-p",
        "-t",
        &format!("{session_name}:0.0"),
        "#{pane_id}",
    ])?;
    run_tmux(["set-option", "-t", session_name, "status", "on"])?;
    run_tmux([
        "set-option",
        "-t",
        session_name,
        "status-position",
        "bottom",
    ])?;
    run_tmux(["set-option", "-t", session_name, "status-justify", "left"])?;
    run_tmux([
        "set-option",
        "-t",
        session_name,
        "status-left-length",
        "240",
    ])?;
    run_tmux(["set-option", "-t", session_name, "status-right", ""])?;
    run_tmux(["set-option", "-t", session_name, "status-interval", "1"])?;
    run_tmux([
        "set-option",
        "-w",
        "-t",
        &format!("{session_name}:0"),
        "@conductor_main_pane",
        main_pane_id.trim(),
    ])?;
    run_tmux([
        "set-option",
        "-t",
        session_name,
        "status-left",
        &format!("#({hud_status_cmd})"),
    ])?;
    run_tmux(["select-pane", "-t", main_pane_id.trim(), "-T", main_title])?;
    configure_tmux_main_exit_hook(session_name, true)?;
    if let Some(prompt) = starter_prompt {
        send_prompt_to_tmux_pane(main_pane_id.trim(), prompt)?;
    }
    attach_tmux_ops_session(session_name)
}

fn resolve_surface_launch(
    cfg: &Config,
    run_id: &str,
    resume_surface: bool,
) -> Result<crate::runtime::adapters::WorkerAdapterLaunch, String> {
    let surface = &cfg.surface;
    let mut adapter = WorkerAdapterConfig {
        worker_type: "surface".to_string(),
        cli: surface.cli.clone(),
        model: String::new(),
        reasoning: None,
        description: surface.description.clone(),
        delivery_mode: "session".to_string(),
        launch_mode: "stdin_text".to_string(),
        base_args: surface.base_args.clone().unwrap_or_default(),
        env: surface.env.clone().unwrap_or_default(),
    };
    adapter.env.insert(
        "CONDUCTOR_TMUX_SESSION".to_string(),
        surface_tmux_session_name(run_id),
    );
    let mut launch = resolve_worker_adapter(&adapter, run_id, "main", None, None)?;
    if resume_surface && adapter.cli == "codex" {
        launch.args.splice(0..0, ["resume".to_string()]);
        launch.stdin_payload = None;
    }
    Ok(launch)
}

fn maybe_resume_context_prompt(
    store: &StateStore,
    run_id: &str,
    resume_surface: bool,
) -> Option<String> {
    if !resume_surface {
        return None;
    }
    let snapshot = store.read_snapshot(run_id).ok()?;
    let next = snapshot.decision.next_action.trim();
    if next.is_empty() || next == "monitor" {
        return None;
    }
    let focus = snapshot.decision.focus_worker.as_deref().unwrap_or("-");
    let why = snapshot.decision.reason.trim();
    let focus_reason = snapshot
        .workers
        .iter()
        .find(|worker| worker.worker_id == focus)
        .and_then(|worker| worker.reason.as_deref());
    let mut lane_lines = snapshot
        .workers
        .iter()
        .filter(|worker| worker.worker_id != "main" && worker.worker_id != "orchestrator-main")
        .filter_map(|worker| {
            let summary = worker.current_summary.as_deref()?.trim();
            if summary.is_empty() {
                return None;
            }
            match worker.state {
                WorkerState::Blocked
                | WorkerState::Done
                | WorkerState::DonePendingVerification
                | WorkerState::VerifiedComplete
                | WorkerState::AwaitingReport
                | WorkerState::Working => Some(format!(
                    "- {} ({:?}): {}",
                    worker.worker_id, worker.state, summary
                )),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    lane_lines.sort_by_key(|line| {
        let lower = line.to_ascii_lowercase();
        if lower.contains("(blocked)") {
            0
        } else if lower.contains("(awaitingreport)") {
            1
        } else if lower.contains("(donependingverification)") {
            2
        } else if lower.contains("(verifiedcomplete)") {
            3
        } else if lower.contains("(done)") {
            4
        } else if lower.contains("(working)") {
            5
        } else {
            6
        }
    });
    lane_lines.truncate(4);
    let lane_block = if lane_lines.is_empty() {
        String::new()
    } else {
        format!("\n\nActive lane context:\n{}", lane_lines.join("\n"))
    };
    let triage_block = build_resume_triage_block(&snapshot);
    let suggested_command = suggested_operator_command(run_id, &snapshot, focus_reason)
        .map(|command| format!("\nSuggested command: {command}"))
        .unwrap_or_default();
    Some(format!(
        "Resume orchestration context for run {run_id}. Next: {next}. Focus: {focus}. Why: {why}.{triage_block} Re-enter as the operator only, integrate the latest lane reports, and decide the next orchestration step without redoing lane work.{lane_block}{suggested_command}"
    ))
}

fn build_resume_triage_block(snapshot: &crate::runtime::types::RuntimeSnapshot) -> String {
    let mut items = Vec::new();
    if snapshot.readiness.pending_approvals > 0 {
        items.push(format!(
            "pending approvals: {}",
            snapshot.readiness.pending_approvals
        ));
    }
    if snapshot.monitor.verification_gaps > 0 {
        items.push(format!(
            "verification gaps: {}",
            snapshot.monitor.verification_gaps
        ));
    }
    let stalled = snapshot
        .workers
        .iter()
        .filter(|worker| worker.reason.as_deref() == Some("stalled_non_reporting"))
        .count();
    if stalled > 0 {
        items.push(format!("stalled lanes: {stalled}"));
    }
    let direct_handoffs = snapshot
        .workers
        .iter()
        .filter(|worker| {
            matches!(
                worker.state,
                WorkerState::Working | WorkerState::AwaitingReport
            ) && worker
                .reason
                .as_deref()
                .map(|reason| {
                    reason == "handoff_requested_from_lane" || reason == "handoff_received"
                })
                .unwrap_or(false)
        })
        .count();
    let pending_handoffs = snapshot.monitor.pending_handoffs.max(direct_handoffs);
    if pending_handoffs > 0 {
        let detail = snapshot
            .monitor
            .active_handoff
            .as_deref()
            .unwrap_or("pending handoff");
        items.push(format!(
            "handoffs in flight: {} ({detail})",
            pending_handoffs
        ));
    }
    if snapshot.mailbox.unread > 0 {
        items.push(format!("unread reports: {}", snapshot.mailbox.unread));
    }
    if items.is_empty() {
        String::new()
    } else {
        format!("\nTriage: {}.", items.join("; "))
    }
}

fn suggested_operator_command(
    run_id: &str,
    snapshot: &crate::runtime::types::RuntimeSnapshot,
    focus_reason: Option<&str>,
) -> Option<String> {
    match snapshot.decision.next_action.as_str() {
        "resume-operator-now" | "resume-operator" => return Some("conductor resume".to_string()),
        _ => {}
    }

    let focus_worker = snapshot.decision.focus_worker.as_deref()?;
    let focus_task_id = snapshot
        .workers
        .iter()
        .find(|worker| worker.worker_id == focus_worker)
        .and_then(|worker| worker.current_task_id.as_deref());
    match snapshot.decision.next_action.as_str() {
        "unblock" => match focus_reason {
            Some("blocked_approval_reported_to_operator") => Some(format!(
                "conductor task-approval {run_id} {} approved <reviewer> \"<reason>\"",
                focus_task_id.unwrap_or("<task_id>")
            )),
            Some("blocked_evidence_reported_to_operator") => Some(format!(
                "conductor ask {focus_worker} \"state the missing proof in one line and say what would satisfy it\""
            )),
            Some("blocked_scope_reported_to_operator") => Some(format!(
                "conductor ask {focus_worker} \"pick one seam only and restate the lane in one sentence\""
            )),
            _ => Some(format!(
                "conductor ask {focus_worker} \"narrow the blocker or confirm the missing dependency\""
            )),
        },
        "accept-completion" | "verify-completion" => Some(format!(
            "conductor accept {focus_worker} \"accepted after verification\""
        )),
        "review-approval" => Some(format!(
            "conductor task-approval {run_id} {} approved <reviewer> \"<reason>\"",
            focus_task_id.unwrap_or("<task_id>")
        )),
        "relaunch-stalled" => Some(format!(
            "conductor relaunch {focus_worker} \"report progress now or declare blocked\""
        )),
        "watch-bootstraps" => Some(format!(
            "conductor ask {focus_worker} \"reply with one short ack and the first concrete finding\""
        )),
        "poke-silent" => Some(format!(
            "conductor ask {focus_worker} \"report progress now or declare blocked in one line\""
        )),
        "read-inbox" => Some(format!(
            "conductor handoff main {focus_worker} \"pick up the newest report and keep the lane moving\""
        )),
        "watch-handoffs" => Some("conductor inbox".to_string()),
        "reassign-or-close" => Some(format!(
            "conductor close {focus_worker} \"closing this lane after operator review\""
        )),
        _ => None,
    }
}

fn plan_team_roster(
    snapshot: &crate::runtime::types::RuntimeSnapshot,
    mode: TeamMode,
    requested_width: Option<usize>,
) -> Vec<(String, String)> {
    if mode == TeamMode::Default {
        return Vec::new();
    }

    if let Some(width) = requested_width {
        return (1..=width)
            .map(|index| (format!("build-{index}"), "build".to_string()))
            .collect();
    }

    let pressure = snapshot.tasks.pending + snapshot.tasks.blocked + snapshot.tasks.in_progress;
    let mut roster = vec![
        ("explore-1".to_string(), "explore".to_string()),
        ("build-1".to_string(), "build".to_string()),
    ];

    if pressure >= 2 || mode == TeamMode::Ralph {
        roster.push(("build-2".to_string(), "build".to_string()));
    }
    if pressure >= 5 || mode == TeamMode::Ralph {
        roster.push(("review-1".to_string(), "review".to_string()));
    }
    roster.push(("verify-1".to_string(), "verify".to_string()));

    roster
}

fn resolve_team_agent(cfg: &Config, agent_name: &str) -> Result<(String, String), String> {
    let profile = agent_name.trim();
    if profile.is_empty() {
        return Err("team agent names must not be empty".to_string());
    }
    if !cfg.workers.contains_key(profile) {
        let available = cfg.workers.keys().cloned().collect::<Vec<_>>().join(", ");
        return Err(format!(
            "unknown team agent profile '{profile}'. Available profiles: {available}"
        ));
    }
    Ok((profile.to_string(), sanitize_worker_stem(profile)))
}

fn sanitize_worker_stem(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "agent".to_string()
    } else {
        sanitized
    }
}

fn run_config_path() -> Result<(), String> {
    let path = resolve_config_path()?;
    println!("{}", path.display());
    Ok(())
}

fn run_status() -> Result<(), String> {
    let (path, cfg) = load_resolved_config()?;
    let payload = StatusPayload {
        config_path: path.display().to_string(),
        transport: json!({
            "mode": cfg.runtime.transport.mode,
            "preferred": cfg.runtime.transport.preferred,
            "allow_tmux_fallback": cfg.runtime.transport.allow_tmux_fallback
        }),
        defaults: json!({
            "idle_timeout_ms": cfg.defaults.idle_timeout_ms,
            "summary_only": cfg.defaults.summary_only,
            "max_parallel": cfg.defaults.max_parallel
        }),
        workers: json!({
            "max_workers": cfg.runtime.workers.max_workers,
            "spawn_policy": cfg.runtime.workers.spawn_policy,
            "continue_policy": cfg.runtime.workers.continue_policy
        }),
        worker_types: cfg.workers.keys().cloned().collect(),
        ok: true,
    };
    print_json(&payload)
}

fn run_doctor() -> Result<(), String> {
    let (path, cfg) = load_resolved_config()?;
    let issues = validate_config(&cfg);
    let payload = json!({
        "config_path": path.display().to_string(),
        "issues": issues,
        "ok": issues.is_empty()
    });
    print_json(&payload)?;
    if issues.is_empty() {
        Ok(())
    } else {
        Err("config validation failed".to_string())
    }
}

fn run_install(args: &[String]) -> Result<(), String> {
    run_sync_skills(args)
}

fn run_uninstall(args: &[String]) -> Result<(), String> {
    run_remove_skills(args)
}

fn run_sync_skills(_args: &[String]) -> Result<(), String> {
    let target_root = codex_skills_root()?;
    fs::create_dir_all(&target_root).map_err(|err| err.to_string())?;
    let source_root = repo_skills_root();
    let mut installed = Vec::new();

    for skill in managed_skill_names() {
        let source = source_root.join(skill);
        let target = target_root.join(skill);
        if !source.join("SKILL.md").is_file() {
            return Err(format!(
                "missing managed skill source: {}",
                source.join("SKILL.md").display()
            ));
        }
        if target.exists() || target.symlink_metadata().is_ok() {
            remove_existing_skill_target(&target)?;
        }
        std::os::unix::fs::symlink(&source, &target).map_err(|err| {
            format!(
                "failed to link {} -> {}: {err}",
                target.display(),
                source.display()
            )
        })?;
        installed.push(json!({
            "name": skill,
            "source": source.display().to_string(),
            "target": target.display().to_string(),
        }));
    }

    print_json(&json!({
        "ok": true,
        "skills_root": target_root.display().to_string(),
        "installed": installed,
    }))
}

fn run_remove_skills(_args: &[String]) -> Result<(), String> {
    let target_root = codex_skills_root()?;
    let mut removed = Vec::new();

    for skill in managed_skill_names() {
        let target = target_root.join(skill);
        if target.symlink_metadata().is_ok() {
            remove_existing_skill_target(&target)?;
            removed.push(json!({
                "name": skill,
                "target": target.display().to_string(),
            }));
        }
    }

    print_json(&json!({
        "ok": true,
        "skills_root": target_root.display().to_string(),
        "removed": removed,
    }))
}

fn run_autoresearch(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        let run_id = default_run_id();
        return match read_autoresearch_config(&run_id) {
            Ok(_) => run_autoresearch_status(&run_id),
            Err(_) => run_autoresearch_wizard(&run_id),
        };
    }
    let subcommand = args.first().map(String::as_str).unwrap_or("").trim();
    match subcommand {
        "setup" => run_autoresearch_setup(&args[1..]),
        "step" => run_autoresearch_step(&args[1..]),
        "continue" => run_autoresearch_continue(&args[1..]),
        "status" | "summary" => run_autoresearch_summary(&args[1..]),
        "stop" => run_autoresearch_stop(&args[1..]),
        _ => {
            Err("autoresearch requires setup, step, continue, status, summary, or stop".to_string())
        }
    }
}

fn run_autoresearch_setup(args: &[String]) -> Result<(), String> {
    let parsed = parse_autoresearch_setup_args(args)?;
    let repo_root = git_repo_root()?;
    ensure_git_worktree_clean(&repo_root)?;
    let branch = ensure_autoresearch_branch(&repo_root)?;
    ensure_autoresearch_excludes(&repo_root)?;
    ensure_results_header(&repo_root)?;

    let baseline = run_metric_command(&repo_root, &parsed.metric_command)?;
    let baseline_metric = extract_metric(&baseline.output, &parsed.metric_regex)?
        .ok_or_else(|| "metric regex did not match the baseline output".to_string())?;
    let baseline_commit = git_head_commit(&repo_root)?;
    let now = Utc::now();

    let cfg = AutoresearchConfig {
        schema_version: 1,
        run_id: parsed.run_id.clone(),
        repo_root: repo_root.display().to_string(),
        branch,
        goal: parsed.goal.clone(),
        metric_command: parsed.metric_command.clone(),
        metric_regex: parsed.metric_regex.clone(),
        metric_direction: parsed.metric_direction.as_str().to_string(),
        in_scope_files: parsed.in_scope_files.clone(),
        out_of_scope_files: parsed.out_of_scope_files.clone(),
        constraints: parsed.constraints.clone(),
        max_experiments: parsed.max_experiments,
        simplicity_policy: parsed.simplicity_policy.clone(),
        baseline_metric,
        best_metric: baseline_metric,
        baseline_commit: baseline_commit.clone(),
        best_commit: baseline_commit.clone(),
        experiment_count: 0,
        started_at: now,
        updated_at: now,
        stopped_at: None,
    };
    write_autoresearch_config(&cfg)?;
    append_results_row(
        &repo_root,
        &ExperimentRow {
            experiment: 0,
            commit: baseline_commit.clone(),
            metric: format_metric_value(baseline_metric),
            status: "baseline".to_string(),
            description: "baseline".to_string(),
        },
    )?;

    print_json(&json!({
        "ok": true,
        "run_id": cfg.run_id,
        "branch": cfg.branch,
        "repo_root": cfg.repo_root,
        "baseline_metric": cfg.baseline_metric,
        "baseline_commit": baseline_commit,
        "results_tsv": repo_root.join("results.tsv").display().to_string(),
        "run_log": repo_root.join("run.log").display().to_string(),
    }))
}

fn run_autoresearch_step(args: &[String]) -> Result<(), String> {
    let (run_id, description) = parse_autoresearch_step_args(args)?;
    let mut cfg = read_autoresearch_config(&run_id)?;
    let repo_root = PathBuf::from(&cfg.repo_root);
    let changed_files = git_changed_files(&repo_root)?;
    if changed_files.is_empty() {
        return Err("autoresearch step requires local code changes before measuring".to_string());
    }
    validate_autoresearch_scope(&cfg, &changed_files)?;
    if let Some(max_experiments) = cfg.max_experiments {
        if cfg.experiment_count >= max_experiments {
            return Err("autoresearch experiment budget already reached".to_string());
        }
    }

    git_add_all(&repo_root)?;
    let commit_message = format!("experiment: {description}");
    git_commit_all(&repo_root, &commit_message)?;
    let commit = git_head_commit(&repo_root)?;
    let experiment_number = cfg.experiment_count + 1;
    let outcome = run_metric_command(&repo_root, &cfg.metric_command)?;
    let metric = extract_metric(&outcome.output, &cfg.metric_regex)?;

    let (status, kept, metric_value) = match (outcome.success, metric) {
        (true, Some(value))
            if metric_improved(
                value,
                cfg.best_metric,
                MetricDirection::from_stored(&cfg.metric_direction)?,
            ) =>
        {
            cfg.best_metric = value;
            cfg.best_commit = commit.clone();
            ("keep".to_string(), true, Some(value))
        }
        (true, Some(value)) => {
            git_reset_head(&repo_root)?;
            ("discard".to_string(), false, Some(value))
        }
        _ => {
            git_reset_head(&repo_root)?;
            ("crash".to_string(), false, None)
        }
    };

    cfg.experiment_count = experiment_number;
    cfg.updated_at = Utc::now();
    cfg.stopped_at = None;
    write_autoresearch_config(&cfg)?;
    append_results_row(
        &repo_root,
        &ExperimentRow {
            experiment: experiment_number,
            commit,
            metric: metric_value
                .map(format_metric_value)
                .unwrap_or_else(|| "n/a".to_string()),
            status: status.clone(),
            description: description.clone(),
        },
    )?;

    print_json(&json!({
        "ok": true,
        "run_id": cfg.run_id,
        "status": status,
        "kept": kept,
        "best_metric": cfg.best_metric,
        "best_commit": cfg.best_commit,
        "experiment": experiment_number,
    }))
}

fn run_autoresearch_summary(args: &[String]) -> Result<(), String> {
    let run_id = parse_optional_run_id(args).unwrap_or_else(default_run_id);
    run_autoresearch_status(&run_id)
}

fn run_autoresearch_status(run_id: &str) -> Result<(), String> {
    let cfg = read_autoresearch_config(&run_id)?;
    let repo_root = PathBuf::from(&cfg.repo_root);
    let rows = read_results_rows(&repo_root)?;
    let delta = cfg.best_metric - cfg.baseline_metric;
    let direction = MetricDirection::from_stored(&cfg.metric_direction)?;
    let improved = metric_improved(cfg.best_metric, cfg.baseline_metric, direction);
    let next_action = autoresearch_next_action(&cfg);
    print_json(&json!({
        "ok": true,
        "run_id": cfg.run_id,
        "repo_root": cfg.repo_root,
        "branch": cfg.branch,
        "goal": cfg.goal,
        "metric_command": cfg.metric_command,
        "metric_direction": cfg.metric_direction,
        "baseline_metric": cfg.baseline_metric,
        "best_metric": cfg.best_metric,
        "delta": delta,
        "improved": improved,
        "best_commit": cfg.best_commit,
        "experiment_count": cfg.experiment_count,
        "max_experiments": cfg.max_experiments,
        "stopped_at": cfg.stopped_at,
        "next_action": next_action,
        "rows": rows,
    }))
}

fn autoresearch_next_action(cfg: &AutoresearchConfig) -> &'static str {
    if cfg.stopped_at.is_some() {
        "resume with `conductor autoresearch continue` after deciding the next experiment"
    } else if cfg.experiment_count == 0 {
        "make one focused change in scope, then run `conductor autoresearch continue`"
    } else {
        "inspect the latest result, try one bounded change, then run `conductor autoresearch continue`"
    }
}

fn run_autoresearch_continue(args: &[String]) -> Result<(), String> {
    if args.is_empty() && stdin().is_terminal() {
        let run_id = default_run_id();
        let description = prompt_required_line(
            "Experiment description",
            Some("try the smallest plausible change"),
        )?;
        return run_autoresearch_step(&["--run".to_string(), run_id, description]);
    }
    run_autoresearch_step(args)
}

fn run_autoresearch_stop(args: &[String]) -> Result<(), String> {
    let run_id = parse_optional_run_id(args).unwrap_or_else(default_run_id);
    let mut cfg = read_autoresearch_config(&run_id)?;
    cfg.stopped_at = Some(Utc::now());
    cfg.updated_at = Utc::now();
    write_autoresearch_config(&cfg)?;
    run_autoresearch_status(&run_id)
}

fn run_autoresearch_wizard(run_id: &str) -> Result<(), String> {
    if !stdin().is_terminal() {
        return Err(
            "autoresearch is not initialized; run `conductor autoresearch setup ...`".to_string(),
        );
    }
    let goal = prompt_required_line("Goal", None)?;
    let metric_command = prompt_required_line("Metric command", None)?;
    let metric_regex = prompt_required_line("Metric regex", Some("([0-9]+(?:\\.[0-9]+)?)"))?;
    let direction = prompt_required_line("Direction (lower|higher)", Some("lower"))?;
    let in_scope = prompt_required_line("In-scope path", Some("src"))?;
    let out_of_scope = prompt_optional_line("Out-of-scope path (optional)")?;
    let constraints = prompt_optional_line("Constraint (optional)")?;
    let max_experiments = prompt_optional_line("Max experiments (optional)")?;
    let simplicity_policy = prompt_required_line(
        "Simplicity policy",
        Some("Prefer the smallest experiment that materially improves the metric."),
    )?;

    let mut setup_args = vec![
        "--run".to_string(),
        run_id.to_string(),
        "--goal".to_string(),
        goal,
        "--metric-command".to_string(),
        metric_command,
        "--metric-regex".to_string(),
        metric_regex,
        "--direction".to_string(),
        direction,
        "--in-scope".to_string(),
        in_scope,
        "--simplicity-policy".to_string(),
        simplicity_policy,
    ];
    if let Some(value) = out_of_scope {
        setup_args.extend(["--out-of-scope".to_string(), value]);
    }
    if let Some(value) = constraints {
        setup_args.extend(["--constraint".to_string(), value]);
    }
    if let Some(value) = max_experiments {
        setup_args.extend(["--max-experiments".to_string(), value]);
    }
    run_autoresearch_setup(&setup_args)
}

fn prompt_required_line(label: &str, default: Option<&str>) -> Result<String, String> {
    let value = prompt_line(label, default)?;
    if value.trim().is_empty() {
        return Err(format!("{label} is required"));
    }
    Ok(value)
}

fn prompt_optional_line(label: &str) -> Result<Option<String>, String> {
    let value = prompt_line(label, None)?;
    if value.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn prompt_line(label: &str, default: Option<&str>) -> Result<String, String> {
    let mut prompt = format!("{label}");
    if let Some(default) = default {
        prompt.push_str(&format!(" [{default}]"));
    }
    prompt.push_str(": ");
    print!("{prompt}");
    stdout().flush().map_err(|err| err.to_string())?;
    let mut buffer = String::new();
    stdin()
        .read_line(&mut buffer)
        .map_err(|err| err.to_string())?;
    let trimmed = buffer.trim().to_string();
    if trimmed.is_empty() {
        Ok(default.unwrap_or("").to_string())
    } else {
        Ok(trimmed)
    }
}

fn parse_autoresearch_setup_args(args: &[String]) -> Result<AutoresearchSetupArgs, String> {
    let mut idx = 0;
    let mut run_id = default_run_id();
    let mut goal = None;
    let mut metric_command = None;
    let mut metric_regex = None;
    let mut metric_direction = None;
    let mut in_scope_files = Vec::new();
    let mut out_of_scope_files = Vec::new();
    let mut constraints = Vec::new();
    let mut max_experiments = None;
    let mut simplicity_policy =
        "Prefer the smallest experiment that materially improves the metric.".to_string();

    while idx < args.len() {
        match args[idx].as_str() {
            "--run" => {
                run_id = required_arg(args, idx + 1, "--run requires a value")?.to_string();
                idx += 2;
            }
            "--goal" => {
                goal = Some(required_arg(args, idx + 1, "--goal requires a value")?.to_string());
                idx += 2;
            }
            "--metric-command" => {
                metric_command = Some(
                    required_arg(args, idx + 1, "--metric-command requires a value")?.to_string(),
                );
                idx += 2;
            }
            "--metric-regex" => {
                metric_regex = Some(
                    required_arg(args, idx + 1, "--metric-regex requires a value")?.to_string(),
                );
                idx += 2;
            }
            "--direction" => {
                metric_direction = Some(MetricDirection::parse(required_arg(
                    args,
                    idx + 1,
                    "--direction requires lower|higher",
                )?)?);
                idx += 2;
            }
            "--in-scope" => {
                in_scope_files
                    .push(required_arg(args, idx + 1, "--in-scope requires a path")?.to_string());
                idx += 2;
            }
            "--out-of-scope" => {
                out_of_scope_files.push(
                    required_arg(args, idx + 1, "--out-of-scope requires a path")?.to_string(),
                );
                idx += 2;
            }
            "--constraint" => {
                constraints
                    .push(required_arg(args, idx + 1, "--constraint requires text")?.to_string());
                idx += 2;
            }
            "--max-experiments" => {
                let raw = required_arg(args, idx + 1, "--max-experiments requires a value")?;
                max_experiments = if raw.eq_ignore_ascii_case("unlimited") {
                    None
                } else {
                    Some(raw.parse::<usize>().map_err(|_| {
                        "--max-experiments must be a positive integer or unlimited".to_string()
                    })?)
                };
                idx += 2;
            }
            "--simplicity-policy" => {
                simplicity_policy =
                    required_arg(args, idx + 1, "--simplicity-policy requires text")?.to_string();
                idx += 2;
            }
            unknown => {
                return Err(format!("unknown autoresearch setup flag '{unknown}'"));
            }
        }
    }

    if in_scope_files.is_empty() {
        return Err("autoresearch setup requires at least one --in-scope path".to_string());
    }

    Ok(AutoresearchSetupArgs {
        run_id,
        goal: goal.ok_or_else(|| "--goal is required".to_string())?,
        metric_command: metric_command.ok_or_else(|| "--metric-command is required".to_string())?,
        metric_regex: metric_regex.ok_or_else(|| "--metric-regex is required".to_string())?,
        metric_direction: metric_direction.ok_or_else(|| "--direction is required".to_string())?,
        in_scope_files,
        out_of_scope_files,
        constraints,
        max_experiments,
        simplicity_policy,
    })
}

fn parse_autoresearch_step_args(args: &[String]) -> Result<(String, String), String> {
    if args.is_empty() {
        return Err(
            "autoresearch step requires <description> or --run <run_id> <description>".to_string(),
        );
    }
    let mut idx = 0;
    let mut run_id = default_run_id();
    if args.first().map(String::as_str) == Some("--run") {
        run_id = required_arg(args, 1, "--run requires a value")?.to_string();
        idx = 2;
    }
    let description = args[idx..].join(" ").trim().to_string();
    if description.is_empty() {
        return Err("autoresearch step requires a non-empty experiment description".to_string());
    }
    Ok((run_id, description))
}

fn parse_optional_run_id(args: &[String]) -> Option<String> {
    match args {
        [flag, run_id, ..] if flag == "--run" && !run_id.trim().is_empty() => Some(run_id.clone()),
        [run_id, ..] if !run_id.trim().is_empty() => Some(run_id.clone()),
        _ => None,
    }
}

fn autoresearch_config_path(run_id: &str) -> Result<PathBuf, String> {
    Ok(resolve_state_root()?
        .join("runs")
        .join(run_id)
        .join("autoresearch.json"))
}

fn write_autoresearch_config(cfg: &AutoresearchConfig) -> Result<(), String> {
    let path = autoresearch_config_path(&cfg.run_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let rendered = serde_json::to_string_pretty(cfg).map_err(|err| err.to_string())?;
    fs::write(&path, rendered).map_err(|err| err.to_string())
}

fn read_autoresearch_config(run_id: &str) -> Result<AutoresearchConfig, String> {
    let path = autoresearch_config_path(run_id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&raw).map_err(|err| err.to_string())
}

fn git_repo_root() -> Result<PathBuf, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err("autoresearch requires a git repository".to_string());
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        Err("git did not return a repository root".to_string())
    } else {
        Ok(PathBuf::from(root))
    }
}

fn repo_skills_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills")
}

fn managed_skill_names() -> &'static [&'static str] {
    &[
        "conductor",
        "autoresearch",
        "team",
        "ralph",
        "plan",
        "implement",
        "review",
        "symphony",
    ]
}

fn codex_skills_root() -> Result<PathBuf, String> {
    if let Ok(codex_home) = env::var("CODEX_HOME") {
        let trimmed = codex_home.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join("skills"));
        }
    }
    let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".codex").join("skills"))
}

fn remove_existing_skill_target(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|err| err.to_string())?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path).map_err(|err| err.to_string())
    } else {
        fs::remove_file(path).map_err(|err| err.to_string())
    }
}

fn ensure_git_worktree_clean(repo_root: &Path) -> Result<(), String> {
    let changed = git_changed_files(repo_root)?;
    if changed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "autoresearch requires a clean git worktree before setup; dirty paths: {}",
            changed.join(", ")
        ))
    }
}

fn git_changed_files(repo_root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["status", "--porcelain"])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err("failed to read git status".to_string());
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    Ok(raw.lines().filter_map(parse_porcelain_path).collect())
}

fn parse_porcelain_path(line: &str) -> Option<String> {
    let trimmed = line.trim_end();
    if trimmed.len() < 4 {
        return None;
    }
    let path = trimmed[3..].trim();
    if let Some((_, to)) = path.split_once(" -> ") {
        Some(to.trim().to_string())
    } else {
        Some(path.to_string())
    }
}

fn ensure_autoresearch_branch(repo_root: &Path) -> Result<String, String> {
    let branch = format!("feat/autoresearch-{}", Utc::now().format("%Y%m%d"));
    let verify = Command::new("git")
        .current_dir(repo_root)
        .args(["rev-parse", "--verify", "--quiet", &branch])
        .status()
        .map_err(|err| err.to_string())?;
    let status = if verify.success() {
        Command::new("git")
            .current_dir(repo_root)
            .args(["checkout", &branch])
            .status()
            .map_err(|err| err.to_string())?
    } else {
        Command::new("git")
            .current_dir(repo_root)
            .args(["checkout", "-b", &branch])
            .status()
            .map_err(|err| err.to_string())?
    };
    if !status.success() {
        return Err(format!(
            "failed to switch to autoresearch branch '{branch}'"
        ));
    }
    Ok(branch)
}

fn ensure_autoresearch_excludes(repo_root: &Path) -> Result<(), String> {
    let exclude_path = repo_root.join(".git").join("info").join("exclude");
    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    let mut lines = existing
        .lines()
        .map(|line| line.trim().to_string())
        .collect::<BTreeSet<_>>();
    let mut updated = existing;
    for entry in ["results.tsv", "run.log"] {
        if !lines.contains(entry) {
            if !updated.ends_with('\n') && !updated.is_empty() {
                updated.push('\n');
            }
            updated.push_str(entry);
            updated.push('\n');
            lines.insert(entry.to_string());
        }
    }
    fs::write(&exclude_path, updated).map_err(|err| err.to_string())
}

fn ensure_results_header(repo_root: &Path) -> Result<(), String> {
    let path = repo_root.join("results.tsv");
    if path.exists() {
        return Ok(());
    }
    fs::write(path, "experiment\tcommit\tmetric\tstatus\tdescription\n")
        .map_err(|err| err.to_string())
}

fn append_results_row(repo_root: &Path, row: &ExperimentRow) -> Result<(), String> {
    let path = repo_root.join("results.tsv");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| err.to_string())?;
    writeln!(
        file,
        "{}\t{}\t{}\t{}\t{}",
        row.experiment,
        sanitize_tsv(&row.commit),
        sanitize_tsv(&row.metric),
        sanitize_tsv(&row.status),
        sanitize_tsv(&row.description)
    )
    .map_err(|err| err.to_string())
}

fn read_results_rows(repo_root: &Path) -> Result<Vec<ExperimentRow>, String> {
    let path = repo_root.join("results.tsv");
    let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let mut rows = Vec::new();
    for line in raw.lines().skip(1) {
        let columns = line.split('\t').collect::<Vec<_>>();
        if columns.len() != 5 {
            continue;
        }
        let experiment = columns[0].parse::<usize>().unwrap_or(0);
        rows.push(ExperimentRow {
            experiment,
            commit: columns[1].to_string(),
            metric: columns[2].to_string(),
            status: columns[3].to_string(),
            description: columns[4].to_string(),
        });
    }
    Ok(rows)
}

fn sanitize_tsv(value: &str) -> String {
    value
        .replace('\t', " ")
        .replace('\n', " ")
        .trim()
        .to_string()
}

fn git_head_commit(repo_root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err("failed to read HEAD commit".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_add_all(repo_root: &Path) -> Result<(), String> {
    let status = Command::new("git")
        .current_dir(repo_root)
        .args(["add", "-A"])
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("git add -A failed".to_string())
    }
}

fn git_commit_all(repo_root: &Path, subject: &str) -> Result<(), String> {
    let status = Command::new("git")
        .current_dir(repo_root)
        .args(["commit", "-m", subject])
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("git commit failed".to_string())
    }
}

fn git_reset_head(repo_root: &Path) -> Result<(), String> {
    let status = Command::new("git")
        .current_dir(repo_root)
        .args(["reset", "--hard", "HEAD~1"])
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("git reset --hard HEAD~1 failed".to_string())
    }
}

struct MetricRunOutcome {
    success: bool,
    output: String,
}

fn run_metric_command(repo_root: &Path, command: &str) -> Result<MetricRunOutcome, String> {
    let output = Command::new("/bin/zsh")
        .current_dir(repo_root)
        .args(["-lc", command])
        .output()
        .map_err(|err| err.to_string())?;
    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&output.stdout));
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    fs::write(repo_root.join("run.log"), &combined).map_err(|err| err.to_string())?;
    Ok(MetricRunOutcome {
        success: output.status.success(),
        output: combined,
    })
}

fn extract_metric(output: &str, pattern: &str) -> Result<Option<f64>, String> {
    let regex = Regex::new(pattern).map_err(|err| err.to_string())?;
    if let Some(captures) = regex.captures(output) {
        if let Some(group) = captures.get(1).or_else(|| captures.get(0)) {
            return parse_metric_value(group.as_str()).map(Some);
        }
    }
    Ok(None)
}

fn parse_metric_value(value: &str) -> Result<f64, String> {
    let normalized = value.trim().replace(',', "");
    normalized
        .parse::<f64>()
        .map_err(|_| format!("failed to parse metric value '{value}'"))
}

fn validate_autoresearch_scope(
    cfg: &AutoresearchConfig,
    changed_files: &[String],
) -> Result<(), String> {
    let invalid = changed_files
        .iter()
        .filter(|path| !path_allowed(cfg, path))
        .cloned()
        .collect::<Vec<_>>();
    if invalid.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "out-of-scope changes detected: {}",
            invalid.join(", ")
        ))
    }
}

fn path_allowed(cfg: &AutoresearchConfig, path: &str) -> bool {
    let in_scope = cfg
        .in_scope_files
        .iter()
        .any(|prefix| path_matches_scope(path, prefix));
    let out_of_scope = cfg
        .out_of_scope_files
        .iter()
        .any(|prefix| path_matches_scope(path, prefix));
    in_scope && !out_of_scope
}

fn path_matches_scope(path: &str, scope: &str) -> bool {
    let cleaned = scope.trim().trim_start_matches("./");
    path == cleaned
        || path.starts_with(&format!("{cleaned}/"))
        || (cleaned == "." && !path.is_empty())
}

fn metric_improved(candidate: f64, current_best: f64, direction: MetricDirection) -> bool {
    match direction {
        MetricDirection::LowerIsBetter => candidate < current_best,
        MetricDirection::HigherIsBetter => candidate > current_best,
    }
}

fn format_metric_value(value: f64) -> String {
    format!("{value:.6}")
}

fn run_runtime_init(args: &[String]) -> Result<(), String> {
    let run_id = args
        .first()
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("run-1");
    let owner = args
        .get(1)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("orchestrator-1");
    let store = StateStore::new(resolve_state_root()?);
    let run = store.init_run(run_id, owner)?;
    print_json(&json!({
        "ok": true,
        "state_dir": store.root().display().to_string(),
        "run": run
    }))
}

fn run_runtime_snapshot(args: &[String]) -> Result<(), String> {
    let run_id = args
        .first()
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "runtime-snapshot requires <run_id>".to_string())?;
    let store = StateStore::new(resolve_state_root()?);
    let snapshot = if store
        .root()
        .join("runs")
        .join(run_id)
        .join("snapshot.json")
        .exists()
    {
        store.read_snapshot(run_id)?
    } else {
        store.capture_snapshot(run_id)?
    };
    print_json(&snapshot)
}

fn run_runtime_refresh(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "runtime-refresh requires <run_id>")?;
    let store = StateStore::new(resolve_state_root()?);
    let snapshot = store.refresh_snapshot(run_id)?;
    print_json(&snapshot)
}

fn run_orchestrate(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "run-orchestrate requires <run_id> <worker_type> <prompt> [worker_id]".to_string(),
        );
    }
    let run_id = &args[0];
    let worker_type = &args[1];
    let prompt = &args[2];
    let worker_id = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| format!("{worker_type}-1"));

    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;

    let (_, cfg) = load_resolved_config()?;
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let task_id = format!("task-{worker_id}");
    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Discovering,
        Some("orchestration_start".to_string()),
    )?;
    let task = match store.read_task(run_id, &task_id) {
        Ok(existing) => existing,
        Err(_) => store.create_task(run_id, &task_id, prompt, Some(prompt.clone()))?,
    };
    let _ = acquire_claim(&store, run_id, &task.task_id, &worker_id, 10)?;
    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Spawning,
        Some("worker_session_start".to_string()),
    )?;

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Executing,
        Some("dispatch_prompt".to_string()),
    )?;
    let dispatch_id = format!("dispatch-{worker_id}");
    let message_id = format!("message-{worker_id}");
    let routed = dispatch_prompt_to_adapter(
        &store,
        &adapter,
        run_id,
        &worker_id,
        &task.task_id,
        "orchestrator-main",
        &dispatch_id,
        &message_id,
        prompt,
    )?;
    let ok = routed.ok;
    let session_id = routed.session_id.clone();
    let response = routed.response;

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Verifying,
        Some("result_check".to_string()),
    )?;
    if adapter.delivery_mode == "session" && ok {
        let _ = store.complete_task(
            run_id,
            &task_id,
            "orchestration dispatch delivered",
            json!({
                "session_id": session_id,
                "dispatch_id": dispatch_id,
                "message_id": message_id
            }),
        )?;
    } else if adapter.delivery_mode == "session" {
        let _ = store.fail_task(run_id, &task_id, "dispatch routing failed")?;
    }
    let _ = transition_phase(
        &store,
        run_id,
        if ok {
            RunPhase::Complete
        } else {
            RunPhase::Failed
        },
        Some("orchestration_end".to_string()),
    )?;
    let snapshot = store.read_snapshot(run_id)?;
    print_json(&json!({
        "ok": ok,
        "run_id": run_id,
        "worker_id": worker_id,
        "task_id": task_id,
        "session_id": session_id,
        "response": response,
        "snapshot": snapshot
    }))
}

fn run_fanout(args: &[String]) -> Result<(), String> {
    if args.len() < 4 {
        return Err(
            "run-fanout requires <run_id> <worker_type> <prompt> <worker_id> [worker_id...]"
                .to_string(),
        );
    }
    let run_id = &args[0];
    let worker_type = &args[1];
    let prompt = &args[2];
    let worker_ids = args[3..].to_vec();

    let (_, cfg) = load_resolved_config()?;
    let max_parallel = std::cmp::min(cfg.defaults.max_parallel, cfg.runtime.workers.max_workers);
    if worker_ids.len() as i64 > max_parallel {
        return Err(format!(
            "worker count exceeds configured max_parallel={max_parallel}"
        ));
    }

    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let invocation_id = Utc::now().timestamp_millis();

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Discovering,
        Some("fanout_start".to_string()),
    )?;

    let mut task_specs = Vec::new();
    for worker_id in &worker_ids {
        let task_id = format!("task-{worker_id}-{invocation_id}");
        let task = store.create_task(
            run_id,
            &task_id,
            &format!("fanout {worker_type} task for {worker_id}"),
            Some(prompt.clone()),
        )?;
        let _ = acquire_claim(&store, run_id, &task.task_id, worker_id, 10)?;
        task_specs.push((worker_id.clone(), task.task_id));
    }

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Spawning,
        Some("fanout_worker_sessions".to_string()),
    )?;
    for (worker_id, task_id) in &task_specs {
        if adapter.delivery_mode == "session" {
            ensure_adapter_session(&store, &adapter, run_id, worker_id, Some(task_id))?;
        }
    }

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Executing,
        Some("fanout_dispatch".to_string()),
    )?;
    let mut routed_results = Vec::new();
    for (worker_id, task_id) in &task_specs {
        let dispatch_id = format!("dispatch-{worker_id}-{invocation_id}");
        let message_id = format!("message-{worker_id}-{invocation_id}");
        routed_results.push((
            worker_id.clone(),
            task_id.clone(),
            dispatch_id.clone(),
            message_id.clone(),
            dispatch_prompt_to_adapter(
                &store,
                &adapter,
                run_id,
                worker_id,
                task_id,
                "orchestrator-main",
                &dispatch_id,
                &message_id,
                prompt,
            )?,
        ));
    }

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Verifying,
        Some("fanout_verify".to_string()),
    )?;
    let mut failures = Vec::new();
    let mut results = Vec::new();
    for (worker_id, task_id, dispatch_id, message_id, routed) in routed_results {
        if adapter.delivery_mode == "session" && routed.ok {
            let _ = store.complete_task(
                run_id,
                &task_id,
                "fanout dispatch delivered",
                json!({
                    "worker_id": worker_id,
                    "session_id": routed.session_id,
                    "dispatch_id": dispatch_id,
                    "message_id": message_id
                }),
            )?;
        } else if adapter.delivery_mode == "session" {
            let reason = "dispatch routing failed".to_string();
            let _ = store.fail_task(run_id, &task_id, &reason)?;
            failures.push(json!({
                "worker_id": worker_id,
                "task_id": task_id,
                "reason": reason
            }));
        } else if !routed.ok {
            failures.push(json!({
                "worker_id": worker_id,
                "task_id": task_id,
                "reason": "worker execution failed"
            }));
        }
        results.push(json!({
            "worker_id": worker_id,
            "task_id": task_id,
            "session_id": routed.session_id,
            "dispatch_id": dispatch_id,
            "message_id": message_id,
            "response": routed.response
        }));
    }

    let verifier_result = if let Some(_verifier_cfg) = cfg.workers.get("verify") {
        let verifier_adapter = worker_adapter_config(&cfg, "verify")?;
        let verifier_worker_id = format!("verify-{invocation_id}");
        let verifier_task_id = format!("task-{verifier_worker_id}");
        let verifier_prompt = serde_json::to_string_pretty(&json!({
            "run_id": run_id,
            "worker_type": worker_type,
            "prompt": prompt,
            "fanout_results": results,
            "fanout_failures": failures
        }))
        .map_err(|err| err.to_string())?;
        let task = store.create_task(
            run_id,
            &verifier_task_id,
            "verify fanout results",
            Some("Verifier pass over fan-out worker delivery results".to_string()),
        )?;
        let _ = acquire_claim(&store, run_id, &task.task_id, &verifier_worker_id, 10)?;
        if verifier_adapter.delivery_mode == "session" {
            ensure_adapter_session(
                &store,
                &verifier_adapter,
                run_id,
                &verifier_worker_id,
                Some(&task.task_id),
            )?;
        }
        let verifier_dispatch_id = format!("dispatch-{verifier_worker_id}-{invocation_id}");
        let verifier_message_id = format!("message-{verifier_worker_id}-{invocation_id}");
        let routed = dispatch_prompt_to_adapter(
            &store,
            &verifier_adapter,
            run_id,
            &verifier_worker_id,
            &verifier_task_id,
            "orchestrator-main",
            &verifier_dispatch_id,
            &verifier_message_id,
            &verifier_prompt,
        );
        match routed {
            Ok(routed) => {
                let verification_ok = failures.is_empty() && routed.ok;
                if verifier_adapter.delivery_mode == "session" && verification_ok {
                    let _ = store.complete_task(
                        run_id,
                        &verifier_task_id,
                        "verification dispatch delivered",
                        json!({
                            "worker_id": verifier_worker_id,
                            "session_id": routed.session_id,
                            "dispatch_id": verifier_dispatch_id,
                            "message_id": verifier_message_id
                        }),
                    )?;
                    let _ = store.append_runtime_event(
                        run_id,
                        EventEnvelope {
                            schema_version: SCHEMA_VERSION,
                            event: EventKind::VerificationPassed,
                            timestamp: Utc::now(),
                            run_id: Some(run_id.to_string()),
                            session_id: routed.session_id.clone(),
                            source: "orchestrator".to_string(),
                            worker: Some(verifier_worker_id.clone()),
                            task_id: Some(verifier_task_id.clone()),
                            message_id: Some(verifier_message_id.clone()),
                            reason: Some("verifier_dispatch_delivered".to_string()),
                            context: serde_json::Map::new(),
                        },
                    )?;
                } else if verifier_adapter.delivery_mode == "session" {
                    let _ =
                        store.fail_task(run_id, &verifier_task_id, "verification gate failed")?;
                    let _ = store.append_runtime_event(
                        run_id,
                        EventEnvelope {
                            schema_version: SCHEMA_VERSION,
                            event: EventKind::VerificationFailed,
                            timestamp: Utc::now(),
                            run_id: Some(run_id.to_string()),
                            session_id: routed.session_id.clone(),
                            source: "orchestrator".to_string(),
                            worker: Some(verifier_worker_id.clone()),
                            task_id: Some(verifier_task_id.clone()),
                            message_id: Some(verifier_message_id.clone()),
                            reason: Some("verification_gate_failed".to_string()),
                            context: serde_json::Map::new(),
                        },
                    )?;
                } else {
                    let event = if verification_ok {
                        EventKind::VerificationPassed
                    } else {
                        EventKind::VerificationFailed
                    };
                    let _ = store.append_runtime_event(
                        run_id,
                        EventEnvelope {
                            schema_version: SCHEMA_VERSION,
                            event,
                            timestamp: Utc::now(),
                            run_id: Some(run_id.to_string()),
                            session_id: routed.session_id.clone(),
                            source: "orchestrator".to_string(),
                            worker: Some(verifier_worker_id.clone()),
                            task_id: Some(verifier_task_id.clone()),
                            message_id: Some(verifier_message_id.clone()),
                            reason: Some(if verification_ok {
                                "verifier_execution_succeeded".to_string()
                            } else {
                                "verifier_execution_failed".to_string()
                            }),
                            context: serde_json::Map::new(),
                        },
                    )?;
                }
                Some(json!({
                    "configured": true,
                    "worker_id": verifier_worker_id,
                    "task_id": verifier_task_id,
                    "dispatch_id": verifier_dispatch_id,
                    "message_id": verifier_message_id,
                    "response": routed.response,
                    "ok": verification_ok
                }))
            }
            Err(err) => {
                let _ = store.fail_task(run_id, &verifier_task_id, &err)?;
                let _ = store.append_runtime_event(
                    run_id,
                    EventEnvelope {
                        schema_version: SCHEMA_VERSION,
                        event: EventKind::VerificationFailed,
                        timestamp: Utc::now(),
                        run_id: Some(run_id.to_string()),
                        session_id: None,
                        source: "orchestrator".to_string(),
                        worker: Some(verifier_worker_id.clone()),
                        task_id: Some(verifier_task_id.clone()),
                        message_id: None,
                        reason: Some(err.clone()),
                        context: serde_json::Map::new(),
                    },
                )?;
                failures.push(json!({
                    "worker_id": verifier_worker_id,
                    "task_id": verifier_task_id,
                    "reason": err
                }));
                Some(json!({
                    "configured": true,
                    "worker_id": verifier_worker_id,
                    "task_id": verifier_task_id,
                    "ok": false
                }))
            }
        }
    } else {
        None
    };

    let verifier_ok = verifier_result
        .as_ref()
        .and_then(|value| value.get("ok"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let final_phase = if failures.is_empty() && verifier_ok {
        RunPhase::Complete
    } else {
        RunPhase::Failed
    };
    let _ = transition_phase(&store, run_id, final_phase, Some("fanout_end".to_string()))?;
    let snapshot = store.read_snapshot(run_id)?;
    print_json(&json!({
        "ok": failures.is_empty(),
        "run_id": run_id,
        "worker_type": worker_type,
        "worker_count": worker_ids.len(),
        "results": results,
        "failures": failures,
        "verifier": verifier_result,
        "snapshot": snapshot
    }))
}

fn run_authority_renew(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "authority-renew requires <run_id> <owner> [lease_minutes]",
    )?;
    let owner = required_arg(
        args,
        1,
        "authority-renew requires <run_id> <owner> [lease_minutes]",
    )?;
    let lease_minutes = args
        .get(2)
        .map(|value| value.parse::<i64>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(5);
    let store = StateStore::new(resolve_state_root()?);
    let run = renew_authority(&store, run_id, owner, lease_minutes)?;
    print_json(&run)
}

fn run_phase_set(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "phase-set requires <run_id> <phase> [reason]")?;
    let phase = parse_run_phase(required_arg(
        args,
        1,
        "phase-set requires <run_id> <phase> [reason]",
    )?)?;
    let reason = args.get(2).cloned();
    let store = StateStore::new(resolve_state_root()?);
    let run = transition_phase(&store, run_id, phase, reason)?;
    print_json(&run)
}

fn run_task_claim(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "task-claim requires <run_id> <task_id> <owner> [lease_minutes]",
    )?;
    let task_id = required_arg(
        args,
        1,
        "task-claim requires <run_id> <task_id> <owner> [lease_minutes]",
    )?;
    let owner = required_arg(
        args,
        2,
        "task-claim requires <run_id> <task_id> <owner> [lease_minutes]",
    )?;
    let lease_minutes = args
        .get(3)
        .map(|value| value.parse::<i64>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(5);
    let store = StateStore::new(resolve_state_root()?);
    let task = acquire_claim(&store, run_id, task_id, owner, lease_minutes)?;
    print_json(&task)
}

fn run_task_release(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "task-release requires <run_id> <task_id> <owner>")?;
    let task_id = required_arg(args, 1, "task-release requires <run_id> <task_id> <owner>")?;
    let owner = required_arg(args, 2, "task-release requires <run_id> <task_id> <owner>")?;
    let store = StateStore::new(resolve_state_root()?);
    let task = release_claim(&store, run_id, task_id, owner)?;
    print_json(&task)
}

fn run_task_reclaim_expired(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "task-reclaim-expired requires <run_id>")?;
    let store = StateStore::new(resolve_state_root()?);
    let tasks = reclaim_expired_claims(&store, run_id)?;
    print_json(&json!({
        "run_id": run_id,
        "reclaimed": tasks.len(),
        "tasks": tasks,
    }))
}

fn run_task_approval(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "task-approval requires <run_id> <task_id> <pending|approved|rejected|clear> [reviewer] [reason]",
    )?;
    let task_id = required_arg(
        args,
        1,
        "task-approval requires <run_id> <task_id> <pending|approved|rejected|clear> [reviewer] [reason]",
    )?;
    let action = required_arg(
        args,
        2,
        "task-approval requires <run_id> <task_id> <pending|approved|rejected|clear> [reviewer] [reason]",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let task = match action {
        "clear" => store.update_task_approval(run_id, task_id, None, None, None)?,
        "pending" => {
            let reason = args.get(3).cloned();
            store.update_task_approval(
                run_id,
                task_id,
                Some(ApprovalStatus::Pending),
                None,
                reason,
            )?
        }
        "approved" | "rejected" => {
            let reviewer = required_arg(
                args,
                3,
                "task-approval approved/rejected requires <reviewer> [reason]",
            )?
            .to_string();
            let reason = if args.len() > 4 {
                Some(args[4..].join(" "))
            } else {
                None
            };
            let status = if action == "approved" {
                ApprovalStatus::Approved
            } else {
                ApprovalStatus::Rejected
            };
            store.update_task_approval(run_id, task_id, Some(status), Some(reviewer), reason)?
        }
        _ => {
            return Err(
                "task-approval action must be pending, approved, rejected, or clear".to_string(),
            );
        }
    };
    maybe_resume_lane_after_approval(&store, run_id, task_id, action, &task);
    print_json(&task)
}

fn maybe_resume_lane_after_approval(
    store: &StateStore,
    run_id: &str,
    task_id: &str,
    action: &str,
    task: &TaskRecord,
) {
    let Some(owner) = task.owner.as_deref() else {
        return;
    };
    match action {
        "approved" => {
            let reason = task
                .approval_reason
                .as_deref()
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or("continue from the current blocker");
            let _ = ask_worker(
                store,
                run_id,
                owner,
                &format!(
                    "Approval granted for task {task_id}. Resume the lane from the blocker, keep the scope narrow, and report progress with `conductor report {owner} \"<short result>\"`. Context: {reason}"
                ),
            );
            let _ =
                push_text_to_main_pane(run_id, &format!("{owner}: approval cleared for {task_id}"));
        }
        "rejected" => {
            let reason = task
                .approval_reason
                .as_deref()
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or("operator rejected the approval request");
            let _ = ask_worker(
                store,
                run_id,
                owner,
                &format!(
                    "Approval rejected for task {task_id}. Re-scope the lane or report a narrower blocker with `conductor report {owner} \"blocked: <reason>\"`. Context: {reason}"
                ),
            );
            let _ = push_text_to_main_pane(
                run_id,
                &format!("{owner}: approval rejected for {task_id}"),
            );
        }
        _ => {}
    }
}

fn run_worker_upsert(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-upsert requires <run_id> <worker_id> <state>",
    )?;
    let worker_id = required_arg(
        args,
        1,
        "worker-upsert requires <run_id> <worker_id> <state>",
    )?;
    let state = parse_worker_state(required_arg(
        args,
        2,
        "worker-upsert requires <run_id> <worker_id> <state>",
    )?)?;
    let store = StateStore::new(resolve_state_root()?);
    let now = Utc::now();
    let worker = WorkerRecord {
        worker_id: worker_id.to_string(),
        run_id: run_id.to_string(),
        worker_kind: WorkerKind::Worker,
        session_ref: None,
        state,
        current_task_id: args.get(3).cloned(),
        current_summary: args.get(4).cloned(),
        terminal_label: Some(worker_id.to_string()),
        last_heartbeat_at: Some(now),
        last_stdout_at: None,
        last_event_at: Some(now),
        reason: None,
    };
    let worker = store.upsert_worker(worker)?;
    print_json(&worker)
}

fn run_worker_exec(args: &[String]) -> Result<(), String> {
    if args.len() < 4 {
        return Err(
            "worker-exec requires <run_id> <worker_id> <task_id|-> <program> [args...]".to_string(),
        );
    }
    let run_id = args[0].as_str();
    let worker_id = args[1].as_str();
    let task_id = if args[2] == "-" {
        None
    } else {
        Some(args[2].clone())
    };
    let program = args[3].clone();
    let program_args = args[4..].to_vec();
    let stdin_payload = env::var("CONDUCTOR_WORKER_STDIN").ok();
    let cwd = env::var("CONDUCTOR_WORKER_CWD").ok().map(PathBuf::from);
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;
    let result = execute_worker(
        WorkerLaunchSpec {
            run_id: run_id.to_string(),
            worker_id: worker_id.to_string(),
            task_id,
            worker_kind: WorkerKind::Worker,
            program,
            args: program_args,
            cwd,
            stdin_payload,
            env: BTreeMap::new(),
        },
        &store,
    )?;
    print_json(&result)
}

fn run_worker_spawn_session(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "worker-spawn-session requires <run_id> <worker_id> <program> [args...]".to_string(),
        );
    }
    let run_id = &args[0];
    let worker_id = &args[1];
    let program = &args[2];
    let program_args = args[3..].to_vec();
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let result = spawn_session(
        &store,
        run_id,
        worker_id,
        program,
        &program_args,
        None,
        &BTreeMap::new(),
        &conductor_bin,
    )?;
    print_json(&result.session)
}

fn run_worker_adapter_exec(args: &[String]) -> Result<(), String> {
    if args.len() < 4 {
        return Err(
            "worker-adapter-exec requires <worker_type> <run_id> <worker_id> <task_id|-> [prompt]"
                .to_string(),
        );
    }
    let worker_type = &args[0];
    let run_id = &args[1];
    let worker_id = &args[2];
    let task_id = if args[3] == "-" {
        None
    } else {
        Some(args[3].as_str())
    };
    let prompt = args.get(4).map(String::as_str);
    let (_, cfg) = load_resolved_config()?;
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let launch = resolve_worker_adapter(&adapter, run_id, worker_id, task_id, prompt)?;
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;
    let result = execute_worker(
        WorkerLaunchSpec {
            run_id: run_id.to_string(),
            worker_id: worker_id.to_string(),
            task_id: task_id.map(str::to_string),
            worker_kind: WorkerKind::Worker,
            program: launch.program,
            args: launch.args,
            cwd: launch.cwd,
            stdin_payload: launch.stdin_payload,
            env: launch.env,
        },
        &store,
    )?;
    print_json(&result)
}

fn run_worker_adapter_spawn_session(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "worker-adapter-spawn-session requires <worker_type> <run_id> <worker_id> [prompt]"
                .to_string(),
        );
    }
    let worker_type = &args[0];
    let run_id = &args[1];
    let worker_id = &args[2];
    let prompt = args.get(3).map(String::as_str);
    let (_, cfg) = load_resolved_config()?;
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let launch = resolve_worker_adapter(&adapter, run_id, worker_id, None, None)?;
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let result = spawn_session(
        &store,
        run_id,
        worker_id,
        &launch.program,
        &launch.args,
        launch.cwd.as_deref(),
        &launch.env,
        &conductor_bin,
    )?;
    if let Some(body) = prompt {
        let _ = send_session_command(
            Path::new(&result.session.socket_path),
            &SessionCommand::SendStdin {
                data: format!("{body}\n"),
            },
        )?;
    }
    print_json(&result.session)
}

fn run_worker_send(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "worker-send requires <run_id> <session_id> <data>")?;
    let session_id = required_arg(args, 1, "worker-send requires <run_id> <session_id> <data>")?;
    let data = required_arg(args, 2, "worker-send requires <run_id> <session_id> <data>")?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let response = send_session_command(
        Path::new(&session.socket_path),
        &SessionCommand::SendStdin {
            data: format!(
                "{data}
"
            ),
        },
    )?;
    print_json(&response)
}

fn run_worker_send_raw(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-send-raw requires <run_id> <session_id> <data>",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-send-raw requires <run_id> <session_id> <data>",
    )?;
    let data = required_arg(
        args,
        2,
        "worker-send-raw requires <run_id> <session_id> <data>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let response = send_session_command(
        Path::new(&session.socket_path),
        &SessionCommand::SendRaw {
            data: data.to_string(),
        },
    )?;
    print_json(&response)
}

fn run_worker_attach(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "worker-attach requires <run_id> <session_id>")?;
    let session_id = required_arg(args, 1, "worker-attach requires <run_id> <session_id>")?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let socket_path = PathBuf::from(&session.socket_path);
    let stdout_path = PathBuf::from(&session.stdout_path);

    let running = Arc::new(AtomicBool::new(true));
    let follow_running = running.clone();
    let follow_path = stdout_path.clone();
    let follow_handle = thread::spawn(move || follow_log_file(&follow_path, follow_running));

    let raw_mode = TerminalRawMode::enable()?;
    let _ = std::io::stdout().write_all(
        b"\r\n[attached] press Ctrl-] to detach. input is forwarded to the worker PTY.\r\n",
    );
    let _ = std::io::stdout().flush();

    let mut stdin = std::io::stdin();
    let mut buf = [0_u8; 1];
    while running.load(Ordering::SeqCst) {
        match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                if buf[0] == 0x1d {
                    break;
                }
                let data = String::from_utf8_lossy(&buf[..1]).to_string();
                let response =
                    send_session_command(&socket_path, &SessionCommand::SendRaw { data })?;
                if response.status == "exited" || response.status == "stopped" {
                    break;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err.to_string()),
        }
    }

    running.store(false, Ordering::SeqCst);
    let _ = follow_handle.join();
    drop(raw_mode);
    let _ = std::io::stdout().write_all(b"\r\n[detached]\r\n");
    let _ = std::io::stdout().flush();
    Ok(())
}

fn run_worker_open_terminal(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-open-terminal requires <run_id> <session_id> [terminal_app]",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-open-terminal requires <run_id> <session_id> [terminal_app]",
    )?;
    let terminal_app = args
        .get(2)
        .cloned()
        .or_else(|| env::var("CONDUCTOR_TERMINAL_APP").ok())
        .unwrap_or_else(|| "Terminal".to_string());
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let state_root = resolve_state_root()?;
    let config_path = resolve_config_path().ok();
    let attach_cmd = build_attach_shell_command(
        &cwd,
        &conductor_bin,
        &state_root,
        config_path.as_deref(),
        run_id,
        session_id,
    );

    open_terminal_script(&terminal_app, &attach_cmd)?;
    print_json(&json!({
        "ok": true,
        "terminal_app": terminal_app,
        "run_id": run_id,
        "session_id": session_id
    }))
}

fn run_worker_log(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-log requires <run_id> <session_id> [stdout|stderr|host_stdout|host_stderr] [lines]",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-log requires <run_id> <session_id> [stdout|stderr|host_stdout|host_stderr] [lines]",
    )?;
    let stream = args.get(2).map(String::as_str).unwrap_or("stdout");
    let lines = args
        .get(3)
        .map(|value| value.parse::<usize>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(40);
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let path = match stream {
        "stdout" => PathBuf::from(&session.stdout_path),
        "stderr" => PathBuf::from(&session.stderr_path),
        "host_stdout" => store
            .session_dir(run_id, session_id)
            .join("host.stdout.log"),
        "host_stderr" => store
            .session_dir(run_id, session_id)
            .join("host.stderr.log"),
        _ => {
            return Err(
                "worker-log stream must be stdout, stderr, host_stdout, or host_stderr".to_string(),
            );
        }
    };
    let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let collected = raw.lines().collect::<Vec<_>>();
    let start = collected.len().saturating_sub(lines);
    println!(
        "{}",
        collected[start..].join(
            "
"
        )
    );
    Ok(())
}

fn run_hud_open(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "hud-open requires <run_id> [terminal_app]")?;
    let terminal_app = args
        .get(1)
        .cloned()
        .or_else(|| env::var("CONDUCTOR_TERMINAL_APP").ok())
        .unwrap_or_else(|| "Terminal".to_string());
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let state_root = resolve_state_root()?;
    let config_path = resolve_config_path().ok();
    let hud_cmd = build_hud_shell_command(
        &cwd,
        &conductor_bin,
        &state_root,
        config_path.as_deref(),
        run_id,
    );
    open_terminal_script(&terminal_app, &hud_cmd)?;
    print_json(&json!({
        "ok": true,
        "terminal_app": terminal_app,
        "run_id": run_id,
        "mode": "hud"
    }))
}

fn run_ops_open(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "ops-open requires <run_id> [tmux_session_name]")?;
    let tmux_session_name = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| default_tmux_session_name(run_id));
    run_ops_open_with_filter(run_id, &tmux_session_name, None)
}

fn run_ops_open_with_filter(
    run_id: &str,
    tmux_session_name: &str,
    only_worker_id: Option<&str>,
) -> Result<(), String> {
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let state_root = resolve_state_root()?;
    let config_path = resolve_config_path().ok();
    let store = StateStore::new(state_root.clone());
    let snapshot = store.read_snapshot(run_id)?;
    let hud_cmd = build_hud_shell_command(
        &cwd,
        &conductor_bin,
        &state_root,
        config_path.as_deref(),
        run_id,
    );

    let mut pane_specs = Vec::new();
    for worker in snapshot.workers {
        if let Some(target) = only_worker_id {
            if worker.worker_id != target {
                continue;
            }
        }
        let worker_record = store.read_worker(run_id, &worker.worker_id)?;
        if let Some(session_id) = worker_record.session_ref {
            let attach_cmd = build_attach_shell_command(
                &cwd,
                &conductor_bin,
                &state_root,
                config_path.as_deref(),
                run_id,
                &session_id,
            );
            pane_specs.push(OpsPaneSpec {
                title: worker.worker_id,
                command: attach_cmd,
                starter_prompt: None,
            });
        }
    }
    pane_specs.sort_by_key(|spec| pane_sort_key(&spec.title));

    if command_available("tmux") {
        let created = ensure_tmux_ops_session(&tmux_session_name, &hud_cmd, &pane_specs)?;
        let attached = if env::var("CONDUCTOR_OPS_NO_ATTACH").ok().as_deref() == Some("1")
            || current_tmux_session_hint().as_deref() == Some(tmux_session_name)
        {
            false
        } else {
            attach_tmux_ops_session(&tmux_session_name)?;
            true
        };
        print_json(&json!({
            "ok": true,
            "run_id": run_id,
            "tmux_session": tmux_session_name,
            "created": created,
            "attached": attached,
            "hud": true,
            "sessions": pane_specs.iter().map(|spec| json!({
                "worker_id": spec.title,
                "session_id": serde_json::Value::Null
            })).collect::<Vec<_>>()
        }))
    } else {
        let terminal_app = env::var("CONDUCTOR_TERMINAL_APP")
            .ok()
            .unwrap_or_else(|| "Terminal".to_string());
        open_terminal_script(&terminal_app, &hud_cmd)?;
        for spec in &pane_specs {
            open_terminal_script(&terminal_app, &spec.command)?;
        }
        print_json(&json!({
            "ok": true,
            "run_id": run_id,
            "terminal_app": terminal_app,
            "fallback": "terminal_windows",
            "hud": true,
            "sessions": pane_specs.iter().map(|spec| json!({
                "worker_id": spec.title,
                "session_id": serde_json::Value::Null
            })).collect::<Vec<_>>()
        }))
    }
}

fn run_worker_session_status(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-session-status requires <run_id> <session_id>",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-session-status requires <run_id> <session_id>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let response = send_session_command(Path::new(&session.socket_path), &SessionCommand::Status)?;
    let mut next = session;
    next.updated_at = Utc::now();
    next.status = match response.status.as_str() {
        "running" => SessionStatus::Running,
        "stopped" => SessionStatus::Stopped,
        "exited" => SessionStatus::Exited,
        _ => SessionStatus::Failed,
    };
    if let Some(message) = &response.message {
        if let Some(code) = message.strip_prefix("exit_code=") {
            next.exit_code = code.parse::<i32>().ok();
        }
    }
    if matches!(
        next.status,
        SessionStatus::Exited | SessionStatus::Stopped | SessionStatus::Failed
    ) {
        next.exited_at = Some(Utc::now());
    }
    store.write_session(&next)?;
    print_json(&response)
}

fn run_worker_stop_session(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-stop-session requires <run_id> <session_id>",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-stop-session requires <run_id> <session_id>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let response = send_session_command(Path::new(&session.socket_path), &SessionCommand::Stop)?;
    let mut next = session;
    next.updated_at = Utc::now();
    next.status = SessionStatus::Stopped;
    next.exited_at = Some(Utc::now());
    next.exit_code = Some(-1);
    store.write_session(&next)?;
    let mut worker = store.read_worker(run_id, &next.worker_id)?;
    worker.state = WorkerState::Stopped;
    worker.last_event_at = Some(Utc::now());
    worker.reason = Some("session_stopped".to_string());
    store.upsert_worker(worker)?;
    print_json(&response)
}

fn run_worker_host_command(args: &[String]) -> Result<(), String> {
    if args.len() < 7 {
        return Err("worker-host requires <run_id> <worker_id> <session_id> <socket_path> <stdout_path> <stderr_path> <program> [args...]".to_string());
    }
    let run_id = &args[0];
    let worker_id = &args[1];
    let session_id = &args[2];
    let socket_path = PathBuf::from(&args[3]);
    let stdout_path = PathBuf::from(&args[4]);
    let stderr_path = PathBuf::from(&args[5]);
    let program = &args[6];
    let program_args = args[7..].to_vec();
    run_worker_host(
        run_id,
        worker_id,
        session_id,
        &socket_path,
        &stdout_path,
        &stderr_path,
        program,
        &program_args,
    )
}

fn run_dispatch_route(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "dispatch-route requires <run_id> <request_id> <message_id> <body>",
    )?;
    let request_id = required_arg(
        args,
        1,
        "dispatch-route requires <run_id> <request_id> <message_id> <body>",
    )?;
    let message_id = required_arg(
        args,
        2,
        "dispatch-route requires <run_id> <request_id> <message_id> <body>",
    )?;
    let body = required_arg(
        args,
        3,
        "dispatch-route requires <run_id> <request_id> <message_id> <body>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let dispatch = store.read_dispatch(run_id, request_id)?;
    let session = store.read_session(run_id, &format!("session-{}", dispatch.target))?;
    let mailbox =
        store.create_mailbox_message(run_id, message_id, "orchestrator", &dispatch.target, body)?;
    store.update_dispatch_status(run_id, request_id, DispatchStatus::Notified, None)?;
    store.update_mailbox_status(run_id, &dispatch.target, message_id, false)?;
    let response = send_session_command(
        Path::new(&session.socket_path),
        &SessionCommand::SendStdin {
            data: format!(
                "{body}
"
            ),
        },
    )?;
    if response.ok {
        store.update_mailbox_status(run_id, &dispatch.target, message_id, true)?;
        store.update_dispatch_status(run_id, request_id, DispatchStatus::Delivered, None)?;
    } else {
        store.update_dispatch_status(
            run_id,
            request_id,
            DispatchStatus::Failed,
            response.message.clone(),
        )?;
    }
    print_json(&json!({
        "dispatch": dispatch.request_id,
        "target": dispatch.target,
        "mailbox": mailbox.message_id,
        "response": response
    }))
}

fn run_hud_view(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "hud-view requires <run_id>")?;
    let store = StateStore::new(resolve_state_root()?);
    let snapshot = store.read_snapshot(run_id)?;
    let run = &snapshot.run;
    let authority = snapshot
        .authority
        .as_ref()
        .map(|lease| lease.owner.clone())
        .unwrap_or_else(|| "none".to_string());
    println!("CONDUCTOR OPS");
    println!("=============");
    println!("run        {}", run.run_id);
    println!("phase      {:?}", run.phase);
    println!("active     {}", run.active);
    println!("authority  {}", authority);
    println!();
    println!(
        "tasks      pending={} blocked={} active={} done={} failed={}",
        snapshot.tasks.pending,
        snapshot.tasks.blocked,
        snapshot.tasks.in_progress,
        snapshot.tasks.completed,
        snapshot.tasks.failed
    );
    println!(
        "dispatch   pending={} notified={} delivered={} failed={}",
        snapshot.dispatch.pending,
        snapshot.dispatch.notified,
        snapshot.dispatch.delivered,
        snapshot.dispatch.failed
    );
    println!("mailbox    unread={}", snapshot.mailbox.unread);
    println!(
        "readiness  ready={} approvals={} stale_operator={} silent={}",
        snapshot.readiness.ready,
        snapshot.readiness.pending_approvals,
        snapshot.readiness.stale_operator,
        snapshot.readiness.silent_workers.len()
    );
    println!(
        "monitor    all_idle={} bootstrapping={} verification_gaps={} reclaimed={} non_reporting={} handoffs={} active_handoff={} pending_leader_notifications={} leader_nudge={}",
        snapshot.monitor.all_workers_idle,
        snapshot.monitor.bootstrapping_workers.len(),
        snapshot.monitor.verification_gaps,
        snapshot.monitor.reclaimed_claims,
        snapshot.monitor.non_reporting_workers.len(),
        snapshot.monitor.pending_handoffs,
        snapshot.monitor.active_handoff.as_deref().unwrap_or("-"),
        snapshot.monitor.pending_leader_notifications,
        snapshot
            .monitor
            .leader_nudge_reason
            .as_deref()
            .unwrap_or("-")
    );
    println!("next       {}", next_operator_action(&snapshot));
    println!(
        "focus      {}",
        snapshot.decision.focus_worker.as_deref().unwrap_or("-")
    );
    println!("why        {}", snapshot.decision.reason);
    println!();
    println!("workers");
    println!("-------");
    for worker in snapshot.workers {
        let task_id = worker.current_task_id.as_deref().unwrap_or("-");
        let summary = worker.current_summary.as_deref().unwrap_or("-");
        println!(
            "{} | kind={:?} | state={:?} | task={} | summary={} | lane={}",
            worker.worker_id,
            worker.worker_kind,
            worker.state,
            task_id,
            summary,
            worker_lane_status(&worker)
        );
    }
    Ok(())
}

fn run_next(args: &[String]) -> Result<(), String> {
    let run_id = args.first().cloned().unwrap_or_else(default_run_id);
    let store = StateStore::new(resolve_state_root()?);
    let snapshot = store.read_snapshot(&run_id)?;
    let focus_reason = snapshot
        .decision
        .focus_worker
        .as_deref()
        .and_then(|focus_worker| {
            snapshot
                .workers
                .iter()
                .find(|worker| worker.worker_id == focus_worker)
                .and_then(|worker| worker.reason.as_deref())
        });
    if let Some(command) = suggested_operator_command(&run_id, &snapshot, focus_reason) {
        println!("{command}");
    } else {
        println!("# no direct operator command suggested");
    }
    Ok(())
}

fn run_inbox(args: &[String]) -> Result<(), String> {
    let run_id = args.first().cloned().unwrap_or_else(default_run_id);
    let limit = args
        .get(1)
        .map(|value| value.parse::<usize>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(8);
    let store = StateStore::new(resolve_state_root()?);
    let mailbox = store.read_mailbox(&run_id, "main")?;
    let mut records = mailbox.records;
    records.sort_by_key(|record| record.created_at);
    let start = records.len().saturating_sub(limit);
    let payload = records[start..]
        .iter()
        .map(|record| {
            json!({
                "message_id": record.message_id,
                "from": record.from_worker,
                "to": record.to_worker,
                "body": record.body,
                "created_at": record.created_at,
                "notified_at": record.notified_at,
                "delivered_at": record.delivered_at
            })
        })
        .collect::<Vec<_>>();
    print_json(&payload)
}

fn run_hud_watch(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "hud-watch requires <run_id> [interval_ms] [iterations]",
    )?;
    let interval_ms = args
        .get(1)
        .map(|value| value.parse::<u64>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(1000);
    let iterations = args
        .get(2)
        .map(|value| value.parse::<usize>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(0);
    let store = StateStore::new(resolve_state_root()?);
    let mut count = 0usize;
    loop {
        let snapshot = store.read_snapshot(run_id)?;
        print!("\x1B[2J\x1B[H");
        println!("CONDUCTOR OPS");
        println!("=============");
        println!("run        {}", snapshot.run.run_id);
        println!("phase      {:?}", snapshot.run.phase);
        println!("active     {}", snapshot.run.active);
        println!(
            "authority  {}",
            snapshot
                .authority
                .as_ref()
                .map(|lease| lease.owner.clone())
                .unwrap_or_else(|| "none".to_string())
        );
        println!();
        println!(
            "tasks      pending={} blocked={} active={} done={} failed={}",
            snapshot.tasks.pending,
            snapshot.tasks.blocked,
            snapshot.tasks.in_progress,
            snapshot.tasks.completed,
            snapshot.tasks.failed
        );
        println!(
            "dispatch   pending={} notified={} delivered={} failed={}",
            snapshot.dispatch.pending,
            snapshot.dispatch.notified,
            snapshot.dispatch.delivered,
            snapshot.dispatch.failed
        );
        println!("mailbox    unread={}", snapshot.mailbox.unread);
        println!(
            "readiness  ready={} approvals={} stale_operator={} silent={}",
            snapshot.readiness.ready,
            snapshot.readiness.pending_approvals,
            snapshot.readiness.stale_operator,
            snapshot.readiness.silent_workers.len()
        );
        println!(
            "monitor    all_idle={} bootstrapping={} verification_gaps={} reclaimed={} non_reporting={} handoffs={} active_handoff={} pending_leader_notifications={} leader_nudge={}",
            snapshot.monitor.all_workers_idle,
            snapshot.monitor.bootstrapping_workers.len(),
            snapshot.monitor.verification_gaps,
            snapshot.monitor.reclaimed_claims,
            snapshot.monitor.non_reporting_workers.len(),
            snapshot.monitor.pending_handoffs,
            snapshot.monitor.active_handoff.as_deref().unwrap_or("-"),
            snapshot.monitor.pending_leader_notifications,
            snapshot
                .monitor
                .leader_nudge_reason
                .as_deref()
                .unwrap_or("-")
        );
        println!("next       {}", next_operator_action(&snapshot));
        println!(
            "focus      {}",
            snapshot.decision.focus_worker.as_deref().unwrap_or("-")
        );
        println!("why        {}", snapshot.decision.reason);
        println!();
        println!("workers");
        println!("-------");
        for worker in snapshot.workers {
            let task_id = worker.current_task_id.as_deref().unwrap_or("-");
            let summary = worker.current_summary.as_deref().unwrap_or("-");
            println!(
                "{} | kind={:?} | state={:?} | task={} | summary={} | lane={}",
                worker.worker_id,
                worker.worker_kind,
                worker.state,
                task_id,
                summary,
                worker_lane_status(&worker)
            );
        }
        count += 1;
        if iterations > 0 && count >= iterations {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(interval_ms));
    }
    Ok(())
}

fn run_hud_strip_watch(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "hud-strip-watch requires <run_id> [interval_ms] [iterations]",
    )?;
    let interval_ms = args
        .get(1)
        .map(|value| value.parse::<u64>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(1000);
    let iterations = args
        .get(2)
        .map(|value| value.parse::<usize>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(0);
    let store = StateStore::new(resolve_state_root()?);
    let mut count = 0usize;
    loop {
        let snapshot = store.read_snapshot(run_id)?;
        print!("\r\x1b[2K{}", render_hud_strip(&snapshot, true));
        stdout().flush().map_err(|err| err.to_string())?;
        count += 1;
        if iterations > 0 && count >= iterations {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(interval_ms));
    }
    println!();
    Ok(())
}

fn run_hud_strip_once(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "hud-strip-once requires <run_id>")?;
    let store = StateStore::new(resolve_state_root()?);
    let snapshot = store.read_snapshot(run_id)?;
    println!("{}", render_hud_strip(&snapshot, false));
    Ok(())
}

fn render_hud_strip(snapshot: &crate::runtime::types::RuntimeSnapshot, ansi: bool) -> String {
    let authority = snapshot
        .authority
        .as_ref()
        .map(|lease| lease.owner.as_str())
        .unwrap_or("none");
    let active_workers = snapshot
        .workers
        .iter()
        .filter(|worker| {
            matches!(
                worker.state,
                crate::runtime::types::WorkerState::Working
                    | crate::runtime::types::WorkerState::AwaitingReport
                    | crate::runtime::types::WorkerState::Blocked
            )
        })
        .count();
    let awaiting_reports = snapshot
        .workers
        .iter()
        .filter(|worker| worker_lane_status(worker) == "awaiting-report")
        .count();
    let bootstrapping_workers = snapshot
        .workers
        .iter()
        .filter(|worker| worker_lane_status(worker) == "bootstrapping")
        .count();
    let stalled_workers = snapshot
        .workers
        .iter()
        .filter(|worker| worker_lane_status(worker) == "stalled")
        .count();
    let blocked_workers = snapshot
        .workers
        .iter()
        .filter(|worker| worker_lane_status(worker) == "blocked")
        .count();
    let next = next_operator_action(snapshot);
    let focus = snapshot.decision.focus_worker.as_deref().unwrap_or("-");
    if ansi {
        format!(
            "\x1b[38;5;111m{}\x1b[0m  \x1b[38;5;150m{:?}\x1b[0m  auth:{}  tasks {}/{}/{}/{}/{}  workers {}/{}  boot:{}  await:{}  stalled:{}  blocked:{}  approvals:{}  silent:{}  verify:{}  reclaimed:{}  handoffs:{}  mail:{}  leader:{}  next:{}  focus:{}",
            snapshot.run.run_id,
            snapshot.run.phase,
            authority,
            snapshot.tasks.pending,
            snapshot.tasks.in_progress,
            snapshot.tasks.blocked,
            snapshot.tasks.completed,
            snapshot.tasks.failed,
            active_workers,
            snapshot.workers.len(),
            bootstrapping_workers,
            awaiting_reports,
            stalled_workers,
            blocked_workers,
            snapshot.readiness.pending_approvals,
            snapshot.readiness.silent_workers.len(),
            snapshot.monitor.verification_gaps,
            snapshot.monitor.reclaimed_claims,
            snapshot.monitor.pending_handoffs,
            snapshot.mailbox.unread,
            snapshot
                .monitor
                .leader_nudge_reason
                .as_deref()
                .unwrap_or("-"),
            next,
            focus,
        )
    } else {
        format!(
            "{}  {:?}  auth:{}  tasks {}/{}/{}/{}/{}  workers {}/{}  boot:{}  await:{}  stalled:{}  blocked:{}  approvals:{}  silent:{}  verify:{}  reclaimed:{}  handoffs:{}  mail:{}  leader:{}  next:{}  focus:{}",
            snapshot.run.run_id,
            snapshot.run.phase,
            authority,
            snapshot.tasks.pending,
            snapshot.tasks.in_progress,
            snapshot.tasks.blocked,
            snapshot.tasks.completed,
            snapshot.tasks.failed,
            active_workers,
            snapshot.workers.len(),
            bootstrapping_workers,
            awaiting_reports,
            stalled_workers,
            blocked_workers,
            snapshot.readiness.pending_approvals,
            snapshot.readiness.silent_workers.len(),
            snapshot.monitor.verification_gaps,
            snapshot.monitor.reclaimed_claims,
            snapshot.monitor.pending_handoffs,
            snapshot.mailbox.unread,
            snapshot
                .monitor
                .leader_nudge_reason
                .as_deref()
                .unwrap_or("-"),
            next,
            focus,
        )
    }
}

fn worker_lane_status(worker: &crate::runtime::types::WorkerProjection) -> &'static str {
    match worker.state {
        WorkerState::AwaitingReport => {
            if worker
                .reason
                .as_deref()
                .map(|reason| {
                    reason == "direct_team_bootstrapping" || reason == "direct_team_pane_respawned"
                })
                .unwrap_or(false)
            {
                "bootstrapping"
            } else if worker.reason.as_deref() == Some("stalled_non_reporting") {
                "stalled"
            } else {
                "awaiting-report"
            }
        }
        WorkerState::Blocked => "blocked",
        WorkerState::Working => {
            if worker.reason.as_deref() == Some("stalled_non_reporting") {
                "stalled"
            } else if worker
                .reason
                .as_deref()
                .map(|reason| reason.starts_with("awaiting_report_nudged"))
                .unwrap_or(false)
            {
                "awaiting-report"
            } else {
                let summary = worker.current_summary.as_deref().unwrap_or("");
                if summary.starts_with("direct ") {
                    "awaiting-report"
                } else {
                    "active"
                }
            }
        }
        WorkerState::Idle => {
            if worker
                .current_summary
                .as_deref()
                .map(|summary| !summary.trim().is_empty())
                .unwrap_or(false)
            {
                "reported"
            } else {
                "idle"
            }
        }
        WorkerState::Done => "done",
        WorkerState::DonePendingVerification => "done-awaiting-verify",
        WorkerState::VerifiedComplete => "verified",
        WorkerState::Failed => "failed",
        WorkerState::Stopped => "stopped",
        WorkerState::Unknown => "unknown",
    }
}

fn next_operator_action(snapshot: &crate::runtime::types::RuntimeSnapshot) -> &str {
    snapshot.decision.next_action.as_str()
}

fn run_events_list(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "events-list requires <run_id> [event_name]")?;
    let wakeable_only = args.iter().skip(1).any(|arg| arg == "--wakeable");
    let event_name = args
        .iter()
        .skip(1)
        .find(|arg| arg.as_str() != "--wakeable")
        .map(String::as_str);
    let store = StateStore::new(resolve_state_root()?);
    let events = filter_events(store.read_events(run_id)?, event_name, wakeable_only);
    let payload = events
        .into_iter()
        .map(|event| {
            json!({
                "event": event_name_of(&event),
                "timestamp": event.timestamp,
                "source": event.source,
                "run_id": event.run_id,
                "worker": event.worker,
                "task_id": event.task_id,
                "message_id": event.message_id,
                "reason": event.reason,
                "context": event.context
            })
        })
        .collect::<Vec<_>>();
    print_json(&payload)
}

fn run_hook_run(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err("hook-run requires <run_id> <event_name|*> <program> [args...]".to_string());
    }
    let run_id = &args[0];
    let event_name = &args[1];
    let program = &args[2];
    let program_args = args[3..].to_vec();
    let timeout_secs = env::var("CONDUCTOR_HOOK_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(2);
    let cwd = env::var("CONDUCTOR_HOOK_CWD").ok().map(PathBuf::from);
    let store = StateStore::new(resolve_state_root()?);
    let handled = watch_and_run_hooks(
        &store,
        run_id,
        Some(event_name),
        program,
        &program_args,
        timeout_secs,
        cwd,
    )?;
    print_json(&json!({
        "ok": true,
        "handled": handled,
        "event": event_name
    }))
}

fn run_task_create(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "task-create requires <run_id> <task_id> <title>")?;
    let task_id = required_arg(args, 1, "task-create requires <run_id> <task_id> <title>")?;
    let title = required_arg(args, 2, "task-create requires <run_id> <task_id> <title>")?;
    let description = args.get(3).cloned();
    let store = StateStore::new(resolve_state_root()?);
    let task = store.create_task(run_id, task_id, title, description)?;
    print_json(&task)
}

fn run_dispatch_queue(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "dispatch-queue requires <run_id> <request_id> <target>",
    )?;
    let request_id = required_arg(
        args,
        1,
        "dispatch-queue requires <run_id> <request_id> <target>",
    )?;
    let target = required_arg(
        args,
        2,
        "dispatch-queue requires <run_id> <request_id> <target>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let record = store.queue_dispatch(run_id, request_id, target, serde_json::Map::new())?;
    print_json(&record)
}

fn run_dispatch_update(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "dispatch-update requires <run_id> <request_id> <status>",
    )?;
    let request_id = required_arg(
        args,
        1,
        "dispatch-update requires <run_id> <request_id> <status>",
    )?;
    let status = parse_dispatch_status(required_arg(
        args,
        2,
        "dispatch-update requires <run_id> <request_id> <status>",
    )?)?;
    let reason = args.get(3).cloned();
    let store = StateStore::new(resolve_state_root()?);
    let record = store.update_dispatch_status(run_id, request_id, status, reason)?;
    print_json(&record)
}

fn run_mailbox_send(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let message_id = required_arg(
        args,
        1,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let from_worker = required_arg(
        args,
        2,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let to_worker = required_arg(
        args,
        3,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let body = required_arg(
        args,
        4,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let message = store.create_mailbox_message(run_id, message_id, from_worker, to_worker, body)?;
    print_json(&message)
}

fn run_mailbox_update(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "mailbox-update requires <run_id> <worker_id> <message_id> <notified|delivered>",
    )?;
    let worker_id = required_arg(
        args,
        1,
        "mailbox-update requires <run_id> <worker_id> <message_id> <notified|delivered>",
    )?;
    let message_id = required_arg(
        args,
        2,
        "mailbox-update requires <run_id> <worker_id> <message_id> <notified|delivered>",
    )?;
    let mode = required_arg(
        args,
        3,
        "mailbox-update requires <run_id> <worker_id> <message_id> <notified|delivered>",
    )?;
    let delivered = match mode {
        "notified" => false,
        "delivered" => true,
        _ => return Err("mailbox-update status must be notified or delivered".to_string()),
    };
    let store = StateStore::new(resolve_state_root()?);
    let message = store.update_mailbox_status(run_id, worker_id, message_id, delivered)?;
    print_json(&message)
}

#[derive(Debug, Serialize)]
struct RoutedDispatch {
    session_id: Option<String>,
    ok: bool,
    response: serde_json::Value,
}

fn ensure_run_exists(store: &StateStore, run_id: &str) -> Result<(), String> {
    if !store
        .root()
        .join("runs")
        .join(run_id)
        .join("run.json")
        .exists()
    {
        let _ = store.init_run(run_id, "orchestrator-main")?;
    }
    Ok(())
}

fn default_run_id() -> String {
    let fallback = "conductor".to_string();
    let cwd = match env::current_dir() {
        Ok(value) => value,
        Err(_) => return fallback,
    };
    let name = match cwd.file_name().and_then(|value| value.to_str()) {
        Some(value) if !value.trim().is_empty() => value,
        _ => return fallback,
    };
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        fallback
    } else {
        sanitized
    }
}

fn ensure_adapter_session(
    store: &StateStore,
    adapter: &WorkerAdapterConfig,
    run_id: &str,
    worker_id: &str,
    task_id: Option<&str>,
) -> Result<String, String> {
    let session_id = format!("session-{worker_id}");
    let desired_kind = worker_kind_for_type(adapter.worker_type.as_str(), worker_id);
    if store.session_file(run_id, &session_id).exists() {
        let mut worker = store.read_worker(run_id, worker_id)?;
        if worker.worker_kind != desired_kind {
            worker.worker_kind = desired_kind;
            let _ = store.upsert_worker(worker)?;
        }
        return Ok(session_id);
    }
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let launch = resolve_worker_adapter(adapter, run_id, worker_id, task_id, None)?;
    let result = spawn_session(
        store,
        run_id,
        worker_id,
        &launch.program,
        &launch.args,
        launch.cwd.as_deref(),
        &launch.env,
        &conductor_bin,
    )?;
    let mut worker = store.read_worker(run_id, worker_id)?;
    if worker.worker_kind != desired_kind {
        worker.worker_kind = desired_kind;
        let _ = store.upsert_worker(worker)?;
    }
    Ok(result.session.session_id)
}

fn worker_kind_for_type(worker_type: &str, worker_id: &str) -> WorkerKind {
    if worker_type == "surface" {
        WorkerKind::Orchestrator
    } else if worker_type == "verify"
        || worker_id.starts_with("verify-")
        || worker_id.starts_with("verifier-")
    {
        WorkerKind::Verifier
    } else {
        WorkerKind::Worker
    }
}

fn dispatch_prompt_to_adapter(
    store: &StateStore,
    adapter: &WorkerAdapterConfig,
    run_id: &str,
    worker_id: &str,
    task_id: &str,
    source_worker: &str,
    dispatch_id: &str,
    message_id: &str,
    body: &str,
) -> Result<RoutedDispatch, String> {
    if adapter.delivery_mode != "session" {
        return Err(format!(
            "worker type {} is not allowed to use non-session delivery",
            adapter.worker_type
        ));
    }
    let _ = store.queue_dispatch(run_id, dispatch_id, worker_id, serde_json::Map::new())?;
    let dispatch = store.read_dispatch(run_id, dispatch_id)?;
    let _ =
        store.create_mailbox_message(run_id, message_id, source_worker, &dispatch.target, body)?;
    let _ = store.update_dispatch_status(run_id, dispatch_id, DispatchStatus::Notified, None)?;
    let _ = store.update_mailbox_status(run_id, &dispatch.target, message_id, false)?;
    let session_id = ensure_adapter_session(store, adapter, run_id, worker_id, Some(task_id))?;
    let session = store.read_session(run_id, &session_id)?;
    let response = send_session_command(
        Path::new(&session.socket_path),
        &SessionCommand::SendStdin {
            data: format!("{body}\n"),
        },
    )?;
    if response.ok {
        let _ = store.update_mailbox_status(run_id, &dispatch.target, message_id, true)?;
        let _ =
            store.update_dispatch_status(run_id, dispatch_id, DispatchStatus::Delivered, None)?;
    } else {
        let _ = store.update_dispatch_status(
            run_id,
            dispatch_id,
            DispatchStatus::Failed,
            response.message.clone(),
        )?;
    }
    Ok(RoutedDispatch {
        session_id: Some(session_id),
        ok: response.ok,
        response: serde_json::to_value(response).map_err(|err| err.to_string())?,
    })
}

fn follow_log_file(path: &Path, running: Arc<AtomicBool>) {
    let mut offset = 0_u64;
    while running.load(Ordering::SeqCst) {
        if let Ok(mut file) = File::open(path) {
            if let Ok(metadata) = file.metadata() {
                let len = metadata.len();
                if len < offset {
                    offset = 0;
                }
                if len > offset && file.seek(SeekFrom::Start(offset)).is_ok() {
                    let mut buffer = Vec::new();
                    if file.read_to_end(&mut buffer).is_ok() && !buffer.is_empty() {
                        let _ = std::io::stdout().write_all(&buffer);
                        let _ = std::io::stdout().flush();
                        offset = len;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn build_attach_shell_command(
    cwd: &Path,
    conductor_bin: &Path,
    state_root: &Path,
    config_path: Option<&Path>,
    run_id: &str,
    session_id: &str,
) -> String {
    let mut env_parts = vec![format!("CONDUCTOR_STATE_DIR={}", shell_quote(state_root))];
    if let Some(path) = config_path {
        env_parts.push(format!("CONDUCTOR_CONFIG={}", shell_quote(path)));
    }
    let command_parts = [
        env_parts.join(" "),
        shell_quote(conductor_bin),
        "worker-attach".to_string(),
        shell_quote_str(run_id),
        shell_quote_str(session_id),
    ];
    format!("cd {} && {}", shell_quote(cwd), command_parts.join(" "))
}

fn build_hud_shell_command(
    cwd: &Path,
    conductor_bin: &Path,
    state_root: &Path,
    config_path: Option<&Path>,
    run_id: &str,
) -> String {
    let mut env_parts = vec![format!("CONDUCTOR_STATE_DIR={}", shell_quote(state_root))];
    if let Some(path) = config_path {
        env_parts.push(format!("CONDUCTOR_CONFIG={}", shell_quote(path)));
    }
    env_parts.extend(terminal_passthrough_env(Vec::new()));
    let env_prefix = if env_parts.is_empty() {
        String::new()
    } else {
        format!("{} ", env_parts.join(" "))
    };
    let command_parts = [
        shell_quote(conductor_bin),
        "hud-strip-watch".to_string(),
        shell_quote_str(run_id),
        "1000".to_string(),
    ];
    build_tmux_pane_shell_command(format!(
        "cd {} && {}exec {}",
        shell_quote(cwd),
        env_prefix,
        command_parts.join(" ")
    ))
}

fn build_hud_status_command(
    cwd: &Path,
    conductor_bin: &Path,
    state_root: &Path,
    config_path: Option<&Path>,
    run_id: &str,
) -> String {
    let mut env_parts = vec![format!("CONDUCTOR_STATE_DIR={}", shell_quote(state_root))];
    if let Some(path) = config_path {
        env_parts.push(format!("CONDUCTOR_CONFIG={}", shell_quote(path)));
    }
    env_parts.extend(terminal_passthrough_env(Vec::new()));
    let env_prefix = if env_parts.is_empty() {
        String::new()
    } else {
        format!("{} ", env_parts.join(" "))
    };
    format!(
        "cd {} && {}{} hud-strip-once {}",
        shell_quote(cwd),
        env_prefix,
        shell_quote(conductor_bin),
        shell_quote_str(run_id),
    )
}

fn build_launch_shell_command(
    cwd: &Path,
    launch: &crate::runtime::adapters::WorkerAdapterLaunch,
) -> String {
    let base_env = launch
        .env
        .iter()
        .map(|(key, value)| format!("{key}={}", shell_quote_str(value)))
        .collect::<Vec<_>>();
    let mut env_parts = terminal_passthrough_env(base_env);
    let mut command_parts = vec![shell_quote_str(&launch.program)];
    command_parts.extend(launch.args.iter().map(|arg| shell_quote_str(arg)));
    if let Some(payload) = &launch.stdin_payload {
        env_parts.push(format!(
            "CONDUCTOR_WORKER_STDIN={}",
            shell_quote_str(payload)
        ));
    }
    let env_prefix = if env_parts.is_empty() {
        String::new()
    } else {
        format!("{} ", env_parts.join(" "))
    };
    build_tmux_pane_shell_command(format!(
        "cd {} && {}exec {}",
        shell_quote(cwd),
        env_prefix,
        command_parts.join(" ")
    ))
}

fn build_direct_launch_shell_command(
    cwd: &Path,
    launch: &crate::runtime::adapters::WorkerAdapterLaunch,
    initial_prompt: Option<&str>,
) -> String {
    let base_env = launch
        .env
        .iter()
        .map(|(key, value)| format!("{key}={}", shell_quote_str(value)))
        .collect::<Vec<_>>();
    let env_parts = terminal_passthrough_env(base_env);
    let env_prefix = if env_parts.is_empty() {
        String::new()
    } else {
        format!("{} ", env_parts.join(" "))
    };
    let mut command_parts = vec![shell_quote_str(&launch.program)];
    command_parts.extend(
        team_launch_args(launch)
            .into_iter()
            .map(|arg| shell_quote_str(&arg)),
    );
    if let Some(prompt) = initial_prompt.filter(|value| !value.trim().is_empty()) {
        command_parts.push(shell_quote_str(prompt));
    }
    build_tmux_pane_shell_command(format!(
        "cd {} && {}exec {}",
        shell_quote(cwd),
        env_prefix,
        command_parts.join(" ")
    ))
}

fn launch_uses_inline_prompt(launch: &crate::runtime::adapters::WorkerAdapterLaunch) -> bool {
    Path::new(&launch.program)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("codex"))
        .unwrap_or(false)
}

fn terminal_passthrough_env(mut env_parts: Vec<String>) -> Vec<String> {
    for key in [
        "COLORTERM",
        "TERM_PROGRAM",
        "TERM_PROGRAM_VERSION",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
    ] {
        if let Ok(value) = env::var(key) {
            if !value.trim().is_empty() {
                env_parts.push(format!("{key}={}", shell_quote_str(&value)));
            }
        }
    }
    env_parts
}

fn team_launch_args(launch: &crate::runtime::adapters::WorkerAdapterLaunch) -> Vec<String> {
    let mut args = launch.args.clone();
    let is_codex = Path::new(&launch.program)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("codex"))
        .unwrap_or(false);
    if is_codex && !args.iter().any(|arg| arg == "--no-alt-screen") {
        args.insert(0, "--no-alt-screen".to_string());
    }
    args
}

fn build_tmux_pane_shell_command(inner: String) -> String {
    let shell_path = env::var("SHELL").ok();
    let raw_shell = shell_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/bin/sh");
    let shell_bin = match raw_shell {
        "/bin/zsh" | "/bin/bash" | "/bin/sh" => raw_shell,
        _ => "/bin/sh",
    };
    let rc_source = if shell_bin.ends_with("/zsh") {
        "if [ -f ~/.zshrc ]; then source ~/.zshrc; fi; "
    } else if shell_bin.ends_with("/bash") {
        "if [ -f ~/.bashrc ]; then source ~/.bashrc; fi; "
    } else {
        ""
    };
    let wrapped = format!("{rc_source}unset NO_COLOR; {inner}");
    format!(
        "{} -lc {}",
        shell_quote_str(shell_bin),
        shell_quote_str(&wrapped)
    )
}

fn open_terminal_script(terminal_app: &str, command: &str) -> Result<(), String> {
    let script = format!(
        "tell application {} to activate\n\
         tell application {} to do script {}",
        apple_script_string(terminal_app),
        apple_script_string(terminal_app),
        apple_script_string(command)
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn ensure_tmux_ops_session(
    session_name: &str,
    hud_cmd: &str,
    pane_specs: &[OpsPaneSpec],
) -> Result<bool, String> {
    if tmux_session_exists(session_name)? {
        run_tmux(["set-option", "-t", session_name, "mouse", "on"])?;
        run_tmux(["set-option", "-t", session_name, "set-clipboard", "on"])?;
        sync_existing_tmux_ops_session(session_name, pane_specs)?;
        rebalance_tmux_team_layout(session_name)?;
        return Ok(false);
    }

    let (main_title, main_cmd) = if let Some(spec) = pane_specs.first() {
        (spec.title.as_str(), spec.command.as_str())
    } else {
        ("HUD", hud_cmd)
    };
    run_tmux([
        "new-session",
        "-d",
        "-s",
        session_name,
        "-n",
        "ops",
        main_cmd,
    ])?;
    run_tmux(["set-option", "-t", session_name, "mouse", "on"])?;
    run_tmux(["set-option", "-t", session_name, "set-clipboard", "on"])?;
    let main_pane_id = run_tmux_capture([
        "display-message",
        "-p",
        "-t",
        &format!("{session_name}:0.0"),
        "#{pane_id}",
    ])?;
    run_tmux([
        "set-option",
        "-w",
        "-t",
        &format!("{session_name}:0"),
        "@conductor_main_pane",
        main_pane_id.trim(),
    ])?;
    let main_exit_hook = format!(
        "if -F '#{{==:#{{hook_pane}},{}}}' 'kill-session -t {}' ''",
        main_pane_id.trim(),
        session_name
    );
    run_tmux([
        "set-hook",
        "-t",
        session_name,
        "pane-exited",
        &main_exit_hook,
    ])?;

    let include_hud = !hud_cmd.trim().is_empty();

    if include_hud {
        run_tmux([
            "split-window",
            "-v",
            "-l",
            "1",
            "-t",
            &format!("{session_name}:0"),
            hud_cmd,
        ])?;
        for spec in pane_specs.iter().skip(1) {
            let new_pane_id = run_tmux_capture([
                "split-window",
                "-h",
                "-d",
                "-P",
                "-F",
                "#{pane_id}",
                "-t",
                &format!("{session_name}:0.0"),
                &spec.command,
            ])?;
            let new_pane_id = new_pane_id.trim().to_string();
            if let Some(prompt) = &spec.starter_prompt {
                send_prompt_to_tmux_pane(&new_pane_id, prompt)?;
            }
        }
    }

    if include_hud && pane_specs.len() > 1 {
        run_tmux(["select-layout", "-t", &format!("{session_name}:0"), "tiled"])?;
        run_tmux([
            "resize-pane",
            "-t",
            &format!("{session_name}:0.{}", pane_specs.len()),
            "-y",
            "1",
        ])?;
    }

    run_tmux([
        "select-pane",
        "-t",
        &format!("{session_name}:0.0"),
        "-T",
        main_title,
    ])?;

    if include_hud {
        run_tmux([
            "select-pane",
            "-t",
            &format!("{session_name}:0.{}", pane_specs.len()),
            "-T",
            "HUD",
        ])?;
    }

    let pane_title_offset = 1;
    for (index, spec) in pane_specs.iter().skip(1).enumerate() {
        run_tmux([
            "select-pane",
            "-t",
            &format!("{session_name}:0.{}", index + pane_title_offset),
            "-T",
            &spec.title,
        ])?;
    }

    if let Some(spec) = pane_specs.first() {
        if let Some(prompt) = &spec.starter_prompt {
            send_prompt_to_tmux_pane(main_pane_id.trim(), prompt)?;
        }
    }

    rebalance_tmux_team_layout(session_name)?;

    Ok(true)
}

fn sync_existing_tmux_ops_session(
    session_name: &str,
    pane_specs: &[OpsPaneSpec],
) -> Result<(), String> {
    let panes = run_tmux_capture([
        "list-panes",
        "-t",
        &format!("{session_name}:0"),
        "-F",
        "#{pane_id}\t#{pane_index}\t#{pane_title}\t#{pane_current_command}",
    ])?;
    let mut title_to_pane = BTreeMap::new();
    let main_pane_id = find_main_pane_id(session_name, &panes)?;
    for line in panes.lines() {
        let mut parts = line.splitn(4, '\t');
        let pane_id = parts.next().unwrap_or_default().trim().to_string();
        let _pane_index = parts.next().unwrap_or_default();
        let pane_title = parts.next().unwrap_or_default().trim().to_string();
        if !pane_title.is_empty() {
            title_to_pane.insert(pane_title, pane_id);
        }
    }
    let mut stack_target = title_to_pane
        .iter()
        .filter(|(title, _)| {
            title.starts_with("explore-")
                || title.starts_with("build-")
                || title.starts_with("review-")
                || title.starts_with("verify-")
        })
        .map(|(_, pane_id)| pane_id.clone())
        .last()
        .unwrap_or_else(|| main_pane_id.clone());

    for spec in pane_specs {
        if title_to_pane.contains_key(&spec.title) {
            continue;
        }
        let split_direction = if stack_target == main_pane_id {
            "-h"
        } else {
            "-v"
        };
        let new_pane_id = run_tmux_capture([
            "split-window",
            split_direction,
            "-d",
            "-P",
            "-F",
            "#{pane_id}",
            "-t",
            &stack_target,
            &spec.command,
        ])?;
        let new_pane_id = new_pane_id.trim().to_string();
        run_tmux(["select-pane", "-t", &new_pane_id, "-T", &spec.title])?;
        if let Some(prompt) = &spec.starter_prompt {
            send_prompt_to_tmux_pane(&new_pane_id, prompt)?;
        }
        stack_target = new_pane_id;
    }
    Ok(())
}

fn send_prompt_to_tmux_pane(pane_id: &str, prompt: &str) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Ok(());
    }
    wait_for_tmux_pane_prompt_ready(pane_id)?;
    let prompt = prompt.replace('\n', " ");
    run_tmux(["send-keys", "-t", pane_id, "-l", "--", &prompt])?;
    let script = format!(
        "sleep 0.1; tmux send-keys -t {} C-m; sleep 0.1; tmux send-keys -t {} C-m",
        shell_quote_str(pane_id),
        shell_quote_str(pane_id),
    );
    run_tmux(["run-shell", "-b", &script])
}

fn wait_for_tmux_pane_prompt_ready(pane_id: &str) -> Result<(), String> {
    for _ in 0..80 {
        let output = run_tmux_capture(["capture-pane", "-p", "-t", pane_id, "-S", "-80"])?;
        if pane_output_ready_for_codex_prompt(&output) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn pane_output_ready_for_codex_prompt(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    (lower.contains("openai codex") || lower.contains("/model to change"))
        && !lower.contains("zsh: command not found")
        && !lower.contains("no such file or directory")
}

fn rebalance_tmux_team_layout(session_name: &str) -> Result<(), String> {
    let panes = run_tmux_capture([
        "list-panes",
        "-t",
        &format!("{session_name}:0"),
        "-F",
        "#{pane_id}\t#{pane_index}\t#{pane_title}\t#{pane_current_command}",
    ])?;

    let main_pane_id = find_main_pane_id(session_name, &panes)?;
    let mut worker_count = 0usize;
    for line in panes.lines() {
        let mut parts = line.splitn(4, '\t');
        let pane_id = parts.next().unwrap_or_default().trim().to_string();
        let _pane_index = parts.next().unwrap_or_default();
        let pane_title = parts.next().unwrap_or_default().trim().to_string();
        if pane_id == main_pane_id {
            continue;
        }
        if pane_title.is_empty() || pane_title == "HUD" {
            continue;
        }
        worker_count += 1;
    }

    if worker_count == 0 {
        return Ok(());
    }

    let window_width = run_tmux_capture([
        "display-message",
        "-p",
        "-t",
        &format!("{session_name}:0"),
        "#{window_width}",
    ])?
    .trim()
    .parse::<u64>()
    .unwrap_or(0);

    if window_width > 0 {
        let desired_width = std::cmp::max(40_u64, (window_width * 62) / 100);
        run_tmux([
            "set-option",
            "-t",
            &format!("{session_name}:0"),
            "main-pane-width",
            &desired_width.to_string(),
        ])?;
    }

    run_tmux([
        "select-layout",
        "-t",
        &format!("{session_name}:0"),
        "main-vertical",
    ])?;
    run_tmux(["select-pane", "-t", &main_pane_id])?;
    Ok(())
}

fn collapse_tmux_surface_to_main(session_name: &str) -> Result<(), String> {
    let panes = run_tmux_capture([
        "list-panes",
        "-t",
        &format!("{session_name}:0"),
        "-F",
        "#{pane_id}\t#{pane_index}\t#{pane_title}\t#{pane_current_command}",
    ])?;
    let main_pane_id = find_main_pane_id(session_name, &panes)?;
    for line in panes.lines() {
        let pane_id = line
            .split('\t')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        if pane_id.is_empty() || pane_id == main_pane_id {
            continue;
        }
        let _ = run_tmux(["kill-pane", "-t", &pane_id]);
    }
    run_tmux(["select-pane", "-t", &main_pane_id])?;
    Ok(())
}

fn configure_tmux_main_exit_hook(session_name: &str, close_session: bool) -> Result<(), String> {
    if !close_session {
        let _ = run_tmux(["set-hook", "-u", "-t", session_name, "pane-exited"]);
        return Ok(());
    }
    let panes = run_tmux_capture([
        "list-panes",
        "-t",
        &format!("{session_name}:0"),
        "-F",
        "#{pane_id}\t#{pane_index}\t#{pane_title}\t#{pane_current_command}",
    ])?;
    let main_pane_id = find_main_pane_id(session_name, &panes)?;
    let main_exit_hook = format!(
        "if -F '#{{==:#{{hook_pane}},{}}}' 'kill-session -t {}' ''",
        main_pane_id.trim(),
        session_name
    );
    run_tmux([
        "set-hook",
        "-t",
        session_name,
        "pane-exited",
        &main_exit_hook,
    ])
}

fn find_main_pane_id(session_name: &str, panes: &str) -> Result<String, String> {
    let target = format!("{session_name}:0");
    if let Ok(configured) = run_tmux_capture([
        "show-options",
        "-w",
        "-v",
        "-t",
        &target,
        "@conductor_main_pane",
    ]) {
        let configured = configured.trim().to_string();
        if !configured.is_empty()
            && panes
                .lines()
                .any(|line| line.split('\t').next().unwrap_or_default().trim() == configured)
        {
            return Ok(configured);
        }
    }

    for line in panes.lines() {
        let mut parts = line.splitn(4, '\t');
        let pane_id = parts.next().unwrap_or_default().trim().to_string();
        let pane_index = parts.next().unwrap_or_default().trim();
        let pane_title = parts.next().unwrap_or_default().trim();
        let pane_command = parts.next().unwrap_or_default().trim();
        if pane_title == "main"
            || pane_title == "conductor-kit"
            || pane_index == "0"
            || pane_command.contains("codex")
        {
            return Ok(pane_id);
        }
    }

    Err(format!(
        "could not find the main pane in tmux session {session_name}"
    ))
}

fn attach_tmux_ops_session(session_name: &str) -> Result<(), String> {
    let has_tty = stdin().is_terminal() && stdout().is_terminal();
    if has_live_tmux_client() {
        if has_tty {
            run_tmux_interactive(["switch-client", "-t", session_name])
        } else {
            run_tmux(["switch-client", "-t", session_name])
        }
    } else if has_tty {
        run_tmux_interactive(["attach-session", "-t", session_name])
    } else {
        Err(format!(
            "created tmux session '{session_name}', but there is no interactive terminal to attach it"
        ))
    }
}

fn has_live_tmux_client() -> bool {
    if env::var_os("TMUX").is_none() {
        return false;
    }
    run_tmux_capture(["display-message", "-p", "#S"]).is_ok()
}

fn current_tmux_session_hint() -> Option<String> {
    env::var("CONDUCTOR_TMUX_SESSION")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if !has_live_tmux_client() {
                return None;
            }
            run_tmux_capture(["display-message", "-p", "#S"])
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn tmux_session_exists(session_name: &str) -> Result<bool, String> {
    let output = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .output()
        .map_err(|err| err.to_string())?;
    Ok(output.status.success())
}

fn run_tmux<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args_vec = args
        .into_iter()
        .map(|value| value.as_ref().to_string())
        .collect::<Vec<_>>();
    let output = Command::new("tmux")
        .args(&args_vec)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(format!("tmux command failed: {}", args_vec.join(" ")))
        } else {
            Err(stderr)
        }
    }
}

fn run_tmux_capture<I, S>(args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args_vec = args
        .into_iter()
        .map(|value| value.as_ref().to_string())
        .collect::<Vec<_>>();
    let output = Command::new("tmux")
        .args(&args_vec)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(format!("tmux command failed: {}", args_vec.join(" ")))
        } else {
            Err(stderr)
        }
    }
}

fn run_tmux_interactive<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args_vec = args
        .into_iter()
        .map(|value| value.as_ref().to_string())
        .collect::<Vec<_>>();
    let mut command = Command::new("tmux");
    command.args(args_vec.iter().map(|value| value.as_str()));
    apply_tmux_terminal_env(&mut command);
    let status = command.status().map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux command failed: {}", args_vec.join(" ")))
    }
}

fn apply_tmux_terminal_env(command: &mut Command) {
    if let Some(term) = preferred_tmux_term() {
        command.env("TERM", &term);
    }
    if env::var("COLORTERM").ok().as_deref() != Some("truecolor") {
        if env::var("TERM_PROGRAM").ok().as_deref() == Some("ghostty") {
            command.env("COLORTERM", "truecolor");
        }
    }
}

fn preferred_tmux_term() -> Option<String> {
    match env::var("TERM") {
        Ok(term) if !term.trim().is_empty() && term != "dumb" => None,
        _ => resolve_tmux_term_fallback_with(env::var("TERM_PROGRAM").ok().as_deref()),
    }
}

fn resolve_tmux_term_fallback() -> Option<String> {
    resolve_tmux_term_fallback_with(env::var("TERM_PROGRAM").ok().as_deref())
}

fn resolve_tmux_term_fallback_with(term_program: Option<&str>) -> Option<String> {
    if let Ok(value) = run_tmux_capture(["show-environment", "-g", "TERM"]) {
        if let Some(term) = value.strip_prefix("TERM=") {
            let trimmed = term.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    match term_program {
        Some("ghostty") => Some("xterm-ghostty".to_string()),
        _ => Some("xterm-256color".to_string()),
    }
}

fn default_tmux_session_name(run_id: &str) -> String {
    format!("conductor-{}", sanitize_tmux_name(run_id))
}

fn surface_tmux_session_name(run_id: &str) -> String {
    format!("conductor-{}-surface", sanitize_tmux_name(run_id))
}

fn sanitize_tmux_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
}

fn pane_sort_key(worker_id: &str) -> (u8, String) {
    if worker_id == "main" {
        (0, worker_id.to_string())
    } else {
        (1, worker_id.to_string())
    }
}

fn shell_quote(value: &Path) -> String {
    shell_quote_str(&value.display().to_string())
}

fn shell_quote_str(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn apple_script_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

struct TerminalRawMode {
    fd: i32,
    original: libc::termios,
}

impl TerminalRawMode {
    fn enable() -> Result<Self, String> {
        let fd = std::io::stdin().as_raw_fd();
        let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
        let rc = unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let original = unsafe { termios.assume_init() };
        let mut raw = original;
        unsafe {
            libc::cfmakeraw(&mut raw);
        }
        let rc = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Self { fd, original })
    }
}

impl Drop for TerminalRawMode {
    fn drop(&mut self) {
        let _ = unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.original) };
    }
}

fn validate_config(cfg: &Config) -> Vec<String> {
    let mut issues = Vec::new();

    if cfg.defaults.idle_timeout_ms < 0 {
        issues.push("defaults.idle_timeout_ms must be >= 0".to_string());
    }
    if cfg.defaults.max_parallel < 1 {
        issues.push("defaults.max_parallel must be >= 1".to_string());
    }
    if cfg.runtime.transport.mode != "direct" {
        issues.push("runtime.transport.mode must be direct".to_string());
    }
    if cfg.runtime.transport.preferred.is_empty() {
        issues.push("runtime.transport.preferred must not be empty".to_string());
    }
    if cfg.runtime.transport.allow_tmux_fallback {
        issues.push(
            "runtime.transport.allow_tmux_fallback must remain false in the new baseline"
                .to_string(),
        );
    }
    if !cfg.runtime.loop_config.persist_runs {
        issues.push("runtime.loop.persist_runs must be true".to_string());
    }
    if cfg.runtime.loop_config.resume_strategy != "ledger" {
        issues.push("runtime.loop.resume_strategy must be ledger".to_string());
    }
    if !cfg.runtime.memory.enabled {
        issues.push("runtime.memory.enabled must be true".to_string());
    }
    if cfg.runtime.memory.ttl_hours < 1 {
        issues.push("runtime.memory.ttl_hours must be >= 1".to_string());
    }
    if !cfg.runtime.memory.invalidate_on_git_head_change {
        issues.push("runtime.memory.invalidate_on_git_head_change must be true".to_string());
    }
    if cfg.runtime.workers.max_workers < 1 {
        issues.push("runtime.workers.max_workers must be >= 1".to_string());
    }
    if !matches!(
        cfg.runtime.workers.spawn_policy.as_str(),
        "ephemeral" | "persistent"
    ) {
        issues.push("runtime.workers.spawn_policy must be ephemeral or persistent".to_string());
    }
    if !matches!(
        cfg.runtime.workers.continue_policy.as_str(),
        "resume_when_possible" | "always_new"
    ) {
        issues.push(
            "runtime.workers.continue_policy must be resume_when_possible or always_new"
                .to_string(),
        );
    }
    if cfg.workers.is_empty() {
        issues.push("workers must not be empty".to_string());
    }

    for (name, worker) in &cfg.workers {
        if worker.cli.trim().is_empty() {
            issues.push(format!("workers.{name}.cli is required"));
        }
        if worker.model.trim().is_empty() {
            issues.push(format!("workers.{name}.model is required"));
        }
        if worker.description.trim().is_empty() {
            issues.push(format!("workers.{name}.description is required"));
        }
        if !command_available(&worker.cli) {
            issues.push(format!(
                "workers.{name}.cli binary not found in PATH: {}",
                worker.cli
            ));
        }
        if let Some(delivery_mode) = &worker.delivery_mode {
            if delivery_mode != "session" {
                issues.push(format!(
                    "workers.{name}.delivery_mode must remain session in the PTY baseline"
                ));
            }
        }
        if let Some(launch_mode) = &worker.launch_mode {
            if !matches!(
                launch_mode.as_str(),
                "stdin_json" | "stdin_text" | "argv_prompt" | "argv_json"
            ) {
                issues.push(format!(
                    "workers.{name}.launch_mode must be stdin_json, stdin_text, argv_prompt, or argv_json"
                ));
            }
        }
        if let Some(reasoning) = &worker.reasoning {
            if reasoning.trim().is_empty() {
                issues.push(format!("workers.{name}.reasoning must not be empty"));
            }
        }
    }

    issues
}

fn worker_adapter_config(cfg: &Config, worker_type: &str) -> Result<WorkerAdapterConfig, String> {
    let worker = cfg
        .workers
        .get(worker_type)
        .ok_or_else(|| format!("unknown worker type: {worker_type}"))?;
    Ok(WorkerAdapterConfig {
        worker_type: worker_type.to_string(),
        cli: worker.cli.clone(),
        model: worker.model.clone(),
        reasoning: worker.reasoning.clone(),
        description: worker.description.clone(),
        delivery_mode: worker
            .delivery_mode
            .clone()
            .unwrap_or_else(|| "session".to_string()),
        launch_mode: worker
            .launch_mode
            .clone()
            .unwrap_or_else(|| "stdin_json".to_string()),
        base_args: worker.base_args.clone().unwrap_or_default(),
        env: worker.env.clone().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::{
        Defaults, LoopConfig, MemoryConfig, RuntimeConfig, SurfaceConfig, TransportConfig,
        WorkerConfig, WorkerRuntimeConfig,
    };
    use crate::cli::host_catalog::{HostCatalog, VendorCatalog};
    use crate::runtime::types::{
        AuthorityLease, DispatchCounts, MailboxCounts, ReadinessState, ReplayState, RunPhase,
        RunSnapshot, RuntimeSnapshot, SessionRecord, SessionStatus, TaskCounts, TaskStatus,
        WorkerKind, WorkerProjection, WorkerRecord, WorkerState,
    };
    use chrono::Utc;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_config_with_workers(workers: BTreeMap<String, WorkerConfig>) -> Config {
        Config {
            defaults: Defaults {
                idle_timeout_ms: 1000,
                summary_only: true,
                max_parallel: 4,
            },
            surface: SurfaceConfig {
                cli: "codex".to_string(),
                description: "surface".to_string(),
                base_args: Some(Vec::new()),
                env: None,
            },
            runtime: RuntimeConfig {
                transport: TransportConfig {
                    mode: "direct".to_string(),
                    preferred: vec!["stdio".to_string()],
                    allow_tmux_fallback: false,
                },
                loop_config: LoopConfig {
                    persist_runs: true,
                    resume_strategy: "ledger".to_string(),
                },
                memory: MemoryConfig {
                    enabled: true,
                    ttl_hours: 24,
                    invalidate_on_git_head_change: true,
                },
                workers: WorkerRuntimeConfig {
                    max_workers: 6,
                    spawn_policy: "persistent".to_string(),
                    continue_policy: "resume_when_possible".to_string(),
                },
            },
            workers,
        }
    }

    fn sample_settings_app(profile: WorkerConfig, host_catalog: HostCatalog) -> SettingsApp {
        let mut workers = BTreeMap::new();
        workers.insert("explore".to_string(), profile);
        SettingsApp {
            config_path: PathBuf::from("/tmp/conductor.json"),
            cfg: sample_config_with_workers(workers),
            entries: vec![SettingsEntry::Profile("explore".to_string())],
            selected_entry: 0,
            selected_field: 0,
            depth: SettingsDepth::Entries,
            choices: Vec::new(),
            selected_choice: 0,
            pending_choice: None,
            input: String::new(),
            status: String::new(),
            host_catalog,
        }
    }

    fn sample_snapshot() -> RuntimeSnapshot {
        RuntimeSnapshot {
            schema_version: SCHEMA_VERSION,
            run: RunSnapshot {
                run_id: "demo-run".to_string(),
                phase: RunPhase::Executing,
                active: true,
                started_at: Utc::now(),
                updated_at: Utc::now(),
            },
            authority: Some(AuthorityLease {
                owner: "orchestrator-main".to_string(),
                lease_id: "lease-1".to_string(),
                leased_until: Utc::now(),
                stale: false,
            }),
            workers: Vec::new(),
            tasks: TaskCounts {
                pending: 1,
                blocked: 0,
                in_progress: 2,
                completed: 3,
                failed: 0,
            },
            dispatch: DispatchCounts {
                pending: 0,
                notified: 0,
                delivered: 0,
                failed: 0,
            },
            mailbox: MailboxCounts { unread: 4 },
            replay: ReplayState {
                cursor: None,
                pending_events: 0,
            },
            readiness: ReadinessState {
                ready: true,
                reasons: Vec::new(),
                pending_approvals: 0,
                stale_operator: false,
                silent_workers: Vec::new(),
            },
            monitor: crate::runtime::types::MonitorState {
                leader_stale: false,
                all_workers_idle: false,
                bootstrapping_workers: Vec::new(),
                verification_gaps: 0,
                non_reporting_workers: Vec::new(),
                reclaimed_claims: 0,
                pending_handoffs: 1,
                active_handoff: Some("review-1 -> build-1: narrow the blocker".to_string()),
                pending_leader_notifications: 4,
                leader_nudge_reason: Some("read_inbox".to_string()),
            },
            decision: crate::runtime::types::OperatorDecision {
                next_action: "read-inbox".to_string(),
                focus_worker: Some("explore-1".to_string()),
                reason: "new worker reports are waiting in the mailbox".to_string(),
            },
        }
    }

    #[test]
    fn resolve_tmux_term_fallback_prefers_ghostty_when_term_is_dumb() {
        let fallback = resolve_tmux_term_fallback_with(Some("ghostty"));
        assert_eq!(fallback.as_deref(), Some("xterm-ghostty"));
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock drift")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }

    #[test]
    fn sync_profile_for_cli_replaces_mismatched_model_and_reasoning() {
        let mut host_catalog = HostCatalog::default();
        host_catalog.claude = VendorCatalog {
            default_model: Some("claude-sonnet-4-6".to_string()),
            models: vec![
                "claude-sonnet-4-6".to_string(),
                "claude-opus-4-1".to_string(),
            ],
            reasoning_levels: BTreeMap::from([(
                "claude-sonnet-4-6".to_string(),
                vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            )]),
        };

        let mut app = sample_settings_app(
            WorkerConfig {
                cli: "claude".to_string(),
                model: "gpt-5.4".to_string(),
                reasoning: Some("xhigh".to_string()),
                description: "explore".to_string(),
                delivery_mode: Some("session".to_string()),
                launch_mode: Some("stdin_text".to_string()),
                base_args: Some(Vec::new()),
                env: None,
            },
            host_catalog,
        );

        sync_profile_for_cli(&mut app, "explore");

        let profile = app.cfg.workers.get("explore").expect("missing profile");
        assert_eq!(profile.model, "claude-sonnet-4-6");
        assert_eq!(profile.reasoning.as_deref(), Some("low"));
    }

    #[test]
    fn render_hud_strip_plain_is_single_line_without_escape_codes() {
        let line = render_hud_strip(&sample_snapshot(), false);
        assert!(line.contains("demo-run"));
        assert!(line.contains("Executing"));
        assert!(line.contains("auth:orchestrator-main"));
        assert!(line.contains("mail:4"));
        assert!(line.contains("reclaimed:0"));
        assert!(!line.contains('\u{1b}'));
    }

    #[test]
    fn resolve_team_agent_requires_configured_profile_names() {
        let cfg = sample_config_with_workers(BTreeMap::from([(
            "review".to_string(),
            WorkerConfig {
                cli: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                reasoning: Some("medium".to_string()),
                description: "review".to_string(),
                delivery_mode: Some("session".to_string()),
                launch_mode: Some("stdin_text".to_string()),
                base_args: Some(Vec::new()),
                env: None,
            },
        )]));

        let resolved = resolve_team_agent(&cfg, "review").expect("review should resolve");
        assert_eq!(resolved.0, "review");
        assert_eq!(resolved.1, "review");

        let err = resolve_team_agent(&cfg, "codex").expect_err("vendor alias should fail");
        assert!(err.contains("unknown team agent profile 'codex'"));
    }

    #[test]
    fn parse_team_invocation_infers_shape_when_no_explicit_shape_is_given() {
        let available = vec![
            "explore".to_string(),
            "build".to_string(),
            "review".to_string(),
            "verify".to_string(),
        ];
        let (run_id, count, profiles, prompt) =
            parse_team_invocation(&[], &available).expect("inference should parse");
        assert_eq!(run_id, default_run_id());
        assert_eq!(count, 2);
        assert_eq!(profiles, vec!["explore", "build"]);
        assert!(prompt.is_none());
    }

    #[test]
    fn parse_team_invocation_infers_prompt_only_shape() {
        let available = vec![
            "explore".to_string(),
            "build".to_string(),
            "review".to_string(),
            "verify".to_string(),
        ];
        let (run_id, count, profiles, prompt) = parse_team_invocation(
            &["--prompt".to_string(), "map the repo".to_string()],
            &available,
        )
        .expect("prompt-only invocation should parse");
        assert_eq!(run_id, default_run_id());
        assert_eq!(count, 2);
        assert_eq!(profiles, vec!["explore", "review"]);
        assert_eq!(prompt.as_deref(), Some("map the repo"));
    }

    #[test]
    fn render_team_starter_prompt_includes_profile_and_task() {
        let prompt = render_team_starter_prompt(
            "demo-run",
            "explore-1",
            "explore",
            Some("inspect the repository and find the likely bug"),
            Some(("codex", "gpt-5.3-codex-spark")),
        );
        assert!(prompt.contains("explore-1"));
        assert!(prompt.contains("Profile: explore"));
        assert!(prompt.contains("inspect the repository and find the likely bug"));
        assert!(prompt.contains("First reply: one short ack"));
        assert!(prompt.contains("conductor report explore-1"));
        assert!(prompt.contains("continue assigned work or the next feasible task"));
        assert!(prompt.contains("blocked: <reason>"));
        assert!(prompt.contains("done: <result>"));
    }

    #[test]
    fn default_entry_attaches_existing_surface_sessions() {
        assert!(matches!(
            decide_default_entry_action(true, true),
            DefaultEntryAction::AttachSurface
        ));
        assert!(matches!(
            decide_default_entry_action(false, true),
            DefaultEntryAction::AttachSurface
        ));
    }

    #[test]
    fn default_entry_starts_when_no_surface_exists() {
        assert!(matches!(
            decide_default_entry_action(true, false),
            DefaultEntryAction::Start
        ));
        assert!(matches!(
            decide_default_entry_action(false, false),
            DefaultEntryAction::Start
        ));
    }

    #[test]
    fn pane_output_ready_for_codex_prompt_requires_the_codex_surface() {
        assert!(!pane_output_ready_for_codex_prompt("zsh prompt only"));
        assert!(pane_output_ready_for_codex_prompt(
            "OpenAI Codex (v0.118.0)\n/model to change"
        ));
    }

    #[test]
    fn report_updates_worker_summary_and_main_mailbox() {
        let root = unique_temp_dir("conductor-report");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "main".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Orchestrator,
                session_ref: None,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("surface".to_string()),
                terminal_label: Some("main".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: None,
            })
            .expect("failed to upsert main worker");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "explore-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Working,
                current_task_id: None,
                current_summary: Some("initial".to_string()),
                terminal_label: Some("explore-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: None,
            })
            .expect("failed to upsert worker");

        report_to_main(
            &store,
            "demo-run",
            "explore-1",
            "mapped the key docs and likely change files",
            None,
        )
        .expect("report should succeed");

        let worker = store
            .read_worker("demo-run", "explore-1")
            .expect("worker should be readable");
        assert_eq!(
            worker.current_summary.as_deref(),
            Some("mapped the key docs and likely change files")
        );
        assert_eq!(worker.state, WorkerState::Working);
        assert_eq!(
            worker.reason.as_deref(),
            Some("reported_progress_continuing")
        );

        let snapshot = store
            .read_snapshot("demo-run")
            .expect("snapshot should be readable");
        assert!(snapshot.mailbox.unread <= 2);

        let events = store
            .read_events("demo-run")
            .expect("events should be readable");
        assert!(events.iter().any(|event| {
            event.event == EventKind::MailboxMessageCreated
                && event.worker.as_deref() == Some("explore-1")
                && event.reason.as_deref() == Some("worker_reported_to_main")
        }));
        assert!(events.iter().any(|event| {
            event.event == EventKind::LeaderNotificationDeferred
                && event.reason.as_deref() == Some("main_pane_unavailable")
        }));
        assert!(events.iter().any(|event| {
            event
                .message_id
                .as_deref()
                .map(|id| id.starts_with("report-explore-1-"))
                .unwrap_or(false)
                && matches!(
                    event.event,
                    EventKind::MailboxMessageNotified | EventKind::MailboxMessageDelivered
                )
        }));
    }

    #[test]
    fn build_operator_report_prompt_mentions_the_worker_and_summary() {
        let prompt = build_operator_report_prompt("explore-1", "mapped the key docs");
        assert!(prompt.contains("explore-1: mapped the key docs"));
        assert!(prompt.contains("mapped the key docs"));
        assert_eq!(prompt, "explore-1: mapped the key docs");
    }

    #[test]
    fn build_team_report_nudge_prompt_requests_an_upward_report() {
        let prompt = build_team_report_nudge_prompt("explore-1");
        assert!(prompt.contains("Act now."));
        assert!(prompt.contains("conductor report explore-1"));
        assert!(prompt.contains("blocked: <reason>"));
        assert!(prompt.contains("done: <result>"));
    }

    #[test]
    fn advance_team_nudge_reason_escalates_to_stalled() {
        assert_eq!(
            advance_team_nudge_reason(Some("direct_team_pane")),
            ("awaiting_report_nudged", false)
        );
        assert_eq!(
            advance_team_nudge_reason(Some("awaiting_report_nudged")),
            ("awaiting_report_nudged_twice", false)
        );
        assert_eq!(
            advance_team_nudge_reason(Some("awaiting_report_nudged_twice")),
            ("stalled_non_reporting", true)
        );
    }

    #[test]
    fn classify_worker_report_marks_blockers_as_blocked() {
        assert_eq!(
            classify_worker_report("blocked: waiting on a missing token"),
            (
                WorkerState::Blocked,
                "blocked_dependency_reported_to_operator".to_string(),
                WorkerReportKind::Blocked
            )
        );
        assert_eq!(
            classify_worker_report("mapped the key docs and likely change files"),
            (
                WorkerState::Working,
                "reported_progress_continuing".to_string(),
                WorkerReportKind::Progress
            )
        );
        assert_eq!(
            classify_worker_report("done: mapped the key docs and likely change files"),
            (
                WorkerState::DonePendingVerification,
                "completion_reported_to_operator".to_string(),
                WorkerReportKind::Done
            )
        );
    }

    #[test]
    fn infer_blocked_kind_distinguishes_common_operator_paths() {
        assert_eq!(
            infer_blocked_kind("blocked: waiting for operator approval"),
            BlockedKind::Approval
        );
        assert_eq!(
            infer_blocked_kind("blocked: need stronger test evidence before claiming done"),
            BlockedKind::Evidence
        );
        assert_eq!(
            infer_blocked_kind("blocked: waiting on an MCP dependency"),
            BlockedKind::Dependency
        );
    }

    #[test]
    fn build_operator_followup_prompt_only_expands_terminal_reports() {
        assert!(
            build_operator_followup_prompt(
                "explore-1",
                "mapped the key docs",
                WorkerReportKind::Progress
            )
            .is_none()
        );
        let blocked = build_operator_followup_prompt(
            "review-1",
            "blocked: waiting on approval",
            WorkerReportKind::Blocked,
        )
        .expect("blocked follow-up should exist");
        assert!(blocked.contains("review-1 is blocked"));
        let done = build_operator_followup_prompt(
            "build-1",
            "done: prepared a staged migration plan",
            WorkerReportKind::Done,
        )
        .expect("done follow-up should exist");
        assert!(done.contains("build-1 finished its lane"));
        let verify_done = build_operator_followup_prompt(
            "verify-1",
            "done: checked the evidence and the branch is clean",
            WorkerReportKind::Done,
        )
        .expect("verify done follow-up should exist");
        assert!(verify_done.contains("verify-1 finished verification"));
    }

    #[test]
    fn build_verify_handoff_prompt_keeps_the_completion_scope_narrow() {
        let prompt =
            build_verify_handoff_prompt("build-1", "done: prepared a staged migration plan");
        assert!(prompt.contains("Verify the completion report from build-1."));
        assert!(prompt.contains("Completion report: done: prepared a staged migration plan"));
    }

    #[test]
    fn build_verify_blocker_prompt_keeps_the_evidence_scope_narrow() {
        let prompt = build_verify_blocker_prompt(
            "review-1",
            "blocked: need stronger test evidence before claiming done",
        );
        assert!(prompt.contains("Inspect the evidence gap reported by review-1."));
        assert!(prompt.contains("Blocked report: blocked: need stronger test evidence"));
    }

    #[test]
    fn build_dependency_handoff_prompt_targets_the_explore_lane() {
        let prompt =
            build_dependency_handoff_prompt("build-1", "blocked: waiting on an MCP dependency");
        assert!(prompt.contains("Unblock the dependency reported by build-1."));
        assert!(prompt.contains("conductor report explore-1"));
    }

    #[test]
    fn build_scope_reset_prompt_keeps_the_retry_narrow() {
        let prompt =
            build_scope_reset_prompt("review-1", "blocked: scope is too broad and needs context");
        assert!(prompt.contains("Narrow the scope for review-1."));
        assert!(prompt.contains("conductor report review-1"));
    }

    #[test]
    fn worker_lane_status_marks_stalled_non_reporting_workers() {
        let worker = crate::runtime::types::WorkerProjection {
            worker_id: "explore-1".to_string(),
            worker_kind: WorkerKind::Worker,
            state: WorkerState::Working,
            current_task_id: None,
            current_summary: Some("mapped the key docs".to_string()),
            last_heartbeat_at: None,
            terminal_label: Some("explore-1".to_string()),
            reason: Some("stalled_non_reporting".to_string()),
        };
        assert_eq!(worker_lane_status(&worker), "stalled");
    }

    #[test]
    fn worker_lane_status_marks_bootstrapping_workers() {
        let worker = crate::runtime::types::WorkerProjection {
            worker_id: "explore-1".to_string(),
            worker_kind: WorkerKind::Worker,
            state: WorkerState::AwaitingReport,
            current_task_id: None,
            current_summary: Some("direct explore pane ready".to_string()),
            last_heartbeat_at: None,
            terminal_label: Some("explore-1".to_string()),
            reason: Some("direct_team_bootstrapping".to_string()),
        };
        assert_eq!(worker_lane_status(&worker), "bootstrapping");
    }

    #[test]
    fn next_operator_action_prioritizes_stalled_workers() {
        let mut snapshot = sample_snapshot();
        snapshot.workers = vec![crate::runtime::types::WorkerProjection {
            worker_id: "explore-1".to_string(),
            worker_kind: WorkerKind::Worker,
            state: WorkerState::Working,
            current_task_id: None,
            current_summary: Some("mapped the key docs".to_string()),
            last_heartbeat_at: None,
            terminal_label: Some("explore-1".to_string()),
            reason: Some("stalled_non_reporting".to_string()),
        }];
        snapshot.decision.next_action = "relaunch-stalled".to_string();
        snapshot.decision.focus_worker = Some("explore-1".to_string());
        assert_eq!(next_operator_action(&snapshot), "relaunch-stalled");
    }

    #[test]
    fn render_hud_strip_includes_focus_worker() {
        let snapshot = sample_snapshot();
        let strip = render_hud_strip(&snapshot, false);
        assert!(strip.contains("next:read-inbox"));
        assert!(strip.contains("focus:explore-1"));
        assert!(strip.contains("leader:read_inbox"));
        assert!(strip.contains("handoffs:1"));
    }

    #[test]
    fn build_all_workers_idle_prompt_lists_lane_summaries() {
        let prompt = build_all_workers_idle_prompt(
            "demo-run",
            &[
                (
                    "explore-1".to_string(),
                    "mapped the entry points".to_string(),
                ),
                (
                    "review-1".to_string(),
                    "flagged two risky seams".to_string(),
                ),
            ],
        );
        assert!(prompt.contains("All team workers in run demo-run are idle."));
        assert!(prompt.contains("- explore-1: mapped the entry points"));
        assert!(prompt.contains("- review-1: flagged two risky seams"));
        assert!(prompt.contains("decide the next assignments"));
    }

    #[test]
    fn maybe_resume_context_prompt_restores_operator_decision_context() {
        let root = unique_temp_dir("conductor-resume-context");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "review-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Blocked,
                current_task_id: Some("task-review-1".to_string()),
                current_summary: Some("blocked: waiting for operator approval".to_string()),
                terminal_label: Some("review-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("blocked_approval_reported_to_operator".to_string()),
            })
            .expect("failed to upsert worker");
        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");

        let prompt = maybe_resume_context_prompt(&store, "demo-run", true)
            .expect("resume prompt should exist");
        assert!(prompt.contains("Next: unblock."));
        assert!(prompt.contains("Focus: review-1."));
        assert!(prompt.contains("operator approval"));
        assert!(prompt.contains("Active lane context:"));
        assert!(prompt.contains("review-1 (Blocked):"));
        assert!(
            prompt.contains("Suggested command: conductor task-approval demo-run task-review-1")
        );
    }

    #[test]
    fn maybe_resume_context_prompt_suggests_scope_narrowing_for_scope_blockers() {
        let root = unique_temp_dir("conductor-resume-scope");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "explore-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Blocked,
                current_task_id: None,
                current_summary: Some("blocked: scope is too broad and needs context".to_string()),
                terminal_label: Some("explore-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("blocked_scope_reported_to_operator".to_string()),
            })
            .expect("failed to upsert worker");
        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");

        let prompt = maybe_resume_context_prompt(&store, "demo-run", true)
            .expect("resume prompt should exist");
        assert!(prompt.contains("Suggested command: conductor ask explore-1"));
        assert!(prompt.contains("pick one seam only"));
    }

    #[test]
    fn maybe_resume_context_prompt_suggests_accept_for_verified_completion() {
        let root = unique_temp_dir("conductor-resume-accept");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "verify-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Verifier,
                session_ref: None,
                state: WorkerState::VerifiedComplete,
                current_task_id: None,
                current_summary: Some("done: verified the branch and it is clean".to_string()),
                terminal_label: Some("verify-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("completion_reported_to_operator".to_string()),
            })
            .expect("failed to upsert worker");
        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");

        let prompt = maybe_resume_context_prompt(&store, "demo-run", true)
            .expect("resume prompt should exist");
        assert!(prompt.contains("Suggested command: conductor accept verify-1"));
    }

    #[test]
    fn maybe_resume_context_prompt_surfaces_triage_counts() {
        let root = unique_temp_dir("conductor-resume-triage");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .create_task(
                "demo-run",
                "task-review-1",
                "Review approval",
                Some("review-1".to_string()),
            )
            .expect("failed to create task");
        store
            .update_task_approval(
                "demo-run",
                "task-review-1",
                Some(ApprovalStatus::Pending),
                Some("waiting for operator approval".to_string()),
                None,
            )
            .expect("failed to update task approval");
        for worker in [
            WorkerRecord {
                worker_id: "review-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::AwaitingReport,
                current_task_id: Some("task-review-1".to_string()),
                current_summary: Some("waiting on approval".to_string()),
                terminal_label: Some("review-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("stalled_non_reporting".to_string()),
            },
            WorkerRecord {
                worker_id: "build-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::AwaitingReport,
                current_task_id: None,
                current_summary: Some("handoff pending".to_string()),
                terminal_label: Some("build-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("handoff_requested_from_lane".to_string()),
            },
        ] {
            store
                .upsert_worker(worker)
                .expect("failed to upsert worker");
        }
        store
            .create_mailbox_message(
                "demo-run",
                "msg-review-1",
                "review-1",
                "main",
                "review-1: waiting on approval",
            )
            .expect("failed to create mailbox message");
        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");

        let prompt = maybe_resume_context_prompt(&store, "demo-run", true)
            .expect("resume prompt should exist");
        assert!(prompt.contains("Triage:"));
        assert!(prompt.contains("pending approvals: 1"));
        assert!(prompt.contains("stalled lanes: 1"));
        assert!(prompt.contains("handoffs in flight: 1"));
        assert!(prompt.contains("unread reports: 1"));
    }

    #[test]
    fn suggested_operator_command_prefers_relaunch_for_stalled_workers() {
        let mut snapshot = sample_snapshot();
        snapshot.decision.next_action = "relaunch-stalled".to_string();
        snapshot.decision.focus_worker = Some("explore-1".to_string());
        let command =
            suggested_operator_command("demo-run", &snapshot, Some("stalled_non_reporting"))
                .expect("command should exist");
        assert!(command.contains("conductor relaunch explore-1"));
    }

    #[test]
    fn suggested_operator_command_guides_handoff_and_silent_triage() {
        let mut snapshot = sample_snapshot();
        snapshot.decision.next_action = "watch-handoffs".to_string();
        let handoff_command =
            suggested_operator_command("demo-run", &snapshot, None).expect("command should exist");
        assert_eq!(handoff_command, "conductor inbox");

        snapshot.decision.next_action = "poke-silent".to_string();
        snapshot.decision.focus_worker = Some("build-1".to_string());
        let silent_command =
            suggested_operator_command("demo-run", &snapshot, Some("awaiting_report_nudged"))
                .expect("command should exist");
        assert!(silent_command.contains("conductor ask build-1"));
    }

    #[test]
    fn suggested_operator_command_returns_resume_for_stale_operator_with_messages() {
        let mut snapshot = sample_snapshot();
        snapshot.decision.next_action = "resume-operator-now".to_string();
        snapshot.decision.focus_worker = None;
        let command =
            suggested_operator_command("demo-run", &snapshot, None).expect("command should exist");
        assert_eq!(command, "conductor resume");
    }

    #[test]
    fn ralph_stall_signature_ignores_active_progress_lanes() {
        let mut snapshot = sample_snapshot();
        snapshot.decision.next_action = "read-inbox".to_string();
        snapshot.decision.reason = "new worker reports are waiting in the mailbox".to_string();
        snapshot.workers.push(WorkerProjection {
            worker_id: "build-1".to_string(),
            worker_kind: WorkerKind::Worker,
            state: WorkerState::Working,
            current_task_id: None,
            current_summary: Some("implementing the lane".to_string()),
            last_heartbeat_at: Some(Utc::now()),
            terminal_label: Some("build-1".to_string()),
            reason: Some("direct_team_pane".to_string()),
        });

        assert!(ralph_stall_signature(&snapshot, true).is_none());
    }

    #[test]
    fn ralph_stall_signature_primes_when_all_workers_are_idle() {
        let mut snapshot = sample_snapshot();
        snapshot.decision.next_action = "reassign-or-close".to_string();
        snapshot.decision.reason = "all worker lanes are idle".to_string();
        snapshot.monitor.all_workers_idle = true;
        snapshot.workers.push(WorkerProjection {
            worker_id: "explore-1".to_string(),
            worker_kind: WorkerKind::Worker,
            state: WorkerState::Idle,
            current_task_id: None,
            current_summary: Some("lane settled".to_string()),
            last_heartbeat_at: Some(Utc::now() - chrono::Duration::seconds(30)),
            terminal_label: Some("explore-1".to_string()),
            reason: Some("direct_team_pane".to_string()),
        });
        for worker in &mut snapshot.workers {
            if worker.worker_id == "main" || worker.worker_id == "orchestrator-main" {
                worker.last_heartbeat_at = Some(Utc::now() - chrono::Duration::seconds(30));
            }
        }

        let signature =
            ralph_stall_signature(&snapshot, false).expect("stall signature should exist");
        assert!(signature.contains("reassign-or-close"));
    }

    #[test]
    fn ralph_stall_signature_skips_reassign_when_operator_is_recently_active() {
        let mut snapshot = sample_snapshot();
        snapshot.decision.next_action = "reassign-or-close".to_string();
        snapshot.decision.reason = "all worker lanes are idle".to_string();
        snapshot.monitor.all_workers_idle = true;
        snapshot.workers.extend([
            WorkerProjection {
                worker_id: "main".to_string(),
                worker_kind: WorkerKind::Orchestrator,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("operator lane active".to_string()),
                reason: Some("operator_active".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                terminal_label: Some("main".to_string()),
            },
            WorkerProjection {
                worker_id: "orchestrator-main".to_string(),
                worker_kind: WorkerKind::Orchestrator,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("operator lane active".to_string()),
                reason: Some("operator_active".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                terminal_label: Some("orchestrator-main".to_string()),
            },
        ]);

        assert!(ralph_stall_signature(&snapshot, false).is_none());
    }

    #[test]
    fn should_prime_ralph_on_entry_skips_active_operator_loops() {
        let root = unique_temp_dir("conductor-ralph-entry-active");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");

        assert!(!should_prime_ralph_on_entry(&store, "demo-run", true));
    }

    #[test]
    fn should_prime_ralph_on_entry_primes_when_the_loop_is_stalled() {
        let root = unique_temp_dir("conductor-ralph-entry-stalled");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "main".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Orchestrator,
                session_ref: None,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("operator lane stale".to_string()),
                terminal_label: Some("main".to_string()),
                last_heartbeat_at: Some(Utc::now() - chrono::Duration::seconds(30)),
                last_stdout_at: None,
                last_event_at: Some(Utc::now() - chrono::Duration::seconds(30)),
                reason: Some("operator_stale".to_string()),
            })
            .expect("failed to write main worker");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "explore-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("lane settled".to_string()),
                terminal_label: Some("explore-1".to_string()),
                last_heartbeat_at: Some(Utc::now() - chrono::Duration::seconds(30)),
                last_stdout_at: None,
                last_event_at: Some(Utc::now() - chrono::Duration::seconds(30)),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to write explore worker");
        let mut orchestrator = store
            .read_worker("demo-run", "orchestrator-main")
            .expect("failed to read orchestrator worker");
        orchestrator.last_heartbeat_at = Some(Utc::now() - chrono::Duration::seconds(30));
        orchestrator.last_event_at = Some(Utc::now() - chrono::Duration::seconds(30));
        orchestrator.current_summary = Some("operator lane stale".to_string());
        orchestrator.reason = Some("operator_stale".to_string());
        store
            .upsert_worker(orchestrator)
            .expect("failed to update orchestrator worker");
        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");

        assert!(should_prime_ralph_on_entry(&store, "demo-run", true));
    }

    #[test]
    fn should_prime_ralph_on_entry_skips_operator_only_runs() {
        let root = unique_temp_dir("conductor-ralph-entry-operator-only");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");

        let mut orchestrator = store
            .read_worker("demo-run", "orchestrator-main")
            .expect("failed to read orchestrator worker");
        orchestrator.last_heartbeat_at = Some(Utc::now() - chrono::Duration::seconds(90));
        orchestrator.last_event_at = Some(Utc::now() - chrono::Duration::seconds(90));
        store
            .upsert_worker(orchestrator)
            .expect("failed to update orchestrator worker");

        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");

        assert!(!should_prime_ralph_on_entry(&store, "demo-run", true));
    }

    #[test]
    fn maybe_resume_context_prompt_lists_blocked_lanes_before_done_lanes() {
        let root = unique_temp_dir("conductor-resume-order");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        for worker in [
            WorkerRecord {
                worker_id: "build-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::DonePendingVerification,
                current_task_id: None,
                current_summary: Some("done: built the migration path".to_string()),
                terminal_label: Some("build-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("completion_reported_to_operator".to_string()),
            },
            WorkerRecord {
                worker_id: "review-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Blocked,
                current_task_id: None,
                current_summary: Some("blocked: waiting for operator approval".to_string()),
                terminal_label: Some("review-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("blocked_approval_reported_to_operator".to_string()),
            },
        ] {
            store
                .upsert_worker(worker)
                .expect("failed to upsert worker");
        }
        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");

        let prompt = maybe_resume_context_prompt(&store, "demo-run", true)
            .expect("resume prompt should exist");
        let blocked_index = prompt
            .find("review-1 (Blocked)")
            .expect("blocked line should exist");
        let done_index = prompt
            .find("build-1 (DonePendingVerification)")
            .expect("done line should exist");
        assert!(blocked_index < done_index);
    }

    #[test]
    fn settle_worker_lane_stops_tmux_workers_without_a_session() {
        let root = unique_temp_dir("conductor-close-worker");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "review-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::DonePendingVerification,
                current_task_id: None,
                current_summary: Some("done: found the main risks".to_string()),
                terminal_label: Some("review-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("completion_reported_to_operator".to_string()),
            })
            .expect("failed to upsert worker");

        let closed = settle_worker_lane(
            &store,
            "demo-run",
            &store.read_worker("demo-run", "review-1").expect("worker"),
            "closed_by_operator",
            Some("operator closed the lane"),
        )
        .expect("lane settlement should succeed");

        assert!(!closed);
        let worker = store
            .read_worker("demo-run", "review-1")
            .expect("worker should remain readable");
        assert_eq!(worker.state, WorkerState::Stopped);
        assert_eq!(worker.reason.as_deref(), Some("closed_by_operator"));
        assert_eq!(worker.current_task_id, None);
    }

    #[test]
    fn maybe_complete_run_after_operator_settlement_closes_fully_settled_runs() {
        let root = unique_temp_dir("conductor-close-run");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        let previous_state_dir = std::env::var_os("CONDUCTOR_STATE_DIR");
        unsafe {
            std::env::set_var("CONDUCTOR_STATE_DIR", &root);
        }
        disable_ralph_loop_for_root(&root, "demo-run").expect("failed to disable ralph");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "verify-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Verifier,
                session_ref: None,
                state: WorkerState::Stopped,
                current_task_id: None,
                current_summary: Some("accepted after verification".to_string()),
                terminal_label: Some("verify-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("closed_by_operator".to_string()),
            })
            .expect("failed to upsert worker");

        let completed =
            maybe_complete_run_after_operator_settlement(&store, "demo-run", "verify-1")
                .expect("run completion check should succeed");
        assert!(completed);

        let run = store.read_run("demo-run").expect("run should still exist");
        assert!(!run.active);
        assert_eq!(run.current_phase, RunPhase::Complete);
        assert!(
            run.stop_reason
                .as_deref()
                .unwrap_or_default()
                .contains("verify-1")
        );
        match previous_state_dir {
            Some(value) => unsafe {
                std::env::set_var("CONDUCTOR_STATE_DIR", value);
            },
            None => unsafe {
                std::env::remove_var("CONDUCTOR_STATE_DIR");
            },
        }
    }

    #[test]
    fn report_to_main_prompts_the_operator_when_the_last_lane_finishes() {
        let root = unique_temp_dir("conductor-report-followup");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "main".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Orchestrator,
                session_ref: None,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("surface".to_string()),
                terminal_label: Some("main".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: None,
            })
            .expect("failed to upsert main worker");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "explore-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Working,
                current_task_id: None,
                current_summary: Some("direct explore pane ready".to_string()),
                terminal_label: Some("explore-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert worker");

        report_to_main(
            &store,
            "demo-run",
            "explore-1",
            "done: mapped the key docs",
            None,
        )
        .expect("report should succeed");

        let events = store
            .read_events("demo-run")
            .expect("events should be readable");
        assert!(events.iter().any(|event| {
            event.reason.as_deref() == Some("all_workers_idle_prompted")
                && event.worker.as_deref() == Some("main")
        }));
    }

    #[test]
    fn recently_emitted_reason_can_scope_to_a_worker() {
        let root = unique_temp_dir("conductor-recent-reason");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .append_runtime_event(
                "demo-run",
                EventEnvelope {
                    schema_version: SCHEMA_VERSION,
                    event: EventKind::WorkerStateChanged,
                    timestamp: Utc::now(),
                    run_id: Some("demo-run".to_string()),
                    session_id: None,
                    source: "test".to_string(),
                    worker: Some("explore-1".to_string()),
                    task_id: None,
                    message_id: None,
                    reason: Some("worker_report_nudged".to_string()),
                    context: serde_json::Map::new(),
                },
            )
            .expect("failed to append event");

        assert!(
            recently_emitted_reason(
                &store,
                "demo-run",
                Some("explore-1"),
                "worker_report_nudged",
                30,
            )
            .expect("lookup should succeed")
        );
        assert!(
            !recently_emitted_reason(
                &store,
                "demo-run",
                Some("review-1"),
                "worker_report_nudged",
                30,
            )
            .expect("lookup should succeed")
        );
    }

    #[test]
    fn report_to_main_handoffs_completed_lane_to_verify_when_present() {
        let root = unique_temp_dir("conductor-verify-handoff");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "build-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Working,
                current_task_id: None,
                current_summary: Some("direct build pane ready".to_string()),
                terminal_label: Some("build-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert build worker");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "verify-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Verifier,
                session_ref: None,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("direct verify pane ready".to_string()),
                terminal_label: Some("verify-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert verify worker");

        report_to_main(
            &store,
            "demo-run",
            "build-1",
            "done: prepared a staged migration plan",
            None,
        )
        .expect("report should succeed");

        let verify_worker = store
            .read_worker("demo-run", "verify-1")
            .expect("verify worker should be readable");
        assert_eq!(verify_worker.state, WorkerState::AwaitingReport);
        assert_eq!(
            verify_worker.reason.as_deref(),
            Some("operator_followup_sent")
        );
        let events = store
            .read_events("demo-run")
            .expect("events should be readable");
        assert!(events.iter().any(|event| {
            event.event == EventKind::HandoffNeeded
                && event.worker.as_deref() == Some("verify-1")
                && event.reason.as_deref() == Some("completion_verification_handoff")
        }));
    }

    #[test]
    fn report_to_main_turns_approval_blockers_into_pending_approvals() {
        let root = unique_temp_dir("conductor-approval-handoff");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .create_task("demo-run", "task-1", "Ship it", None)
            .expect("failed to create task");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "build-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Working,
                current_task_id: Some("task-1".to_string()),
                current_summary: Some("direct build pane ready".to_string()),
                terminal_label: Some("build-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert worker");

        report_to_main(
            &store,
            "demo-run",
            "build-1",
            "blocked: waiting for operator approval before finishing",
            None,
        )
        .expect("report should succeed");

        let task = store
            .read_task("demo-run", "task-1")
            .expect("task should be readable");
        assert_eq!(task.approval_status, Some(ApprovalStatus::Pending));
        assert!(
            task.approval_reason
                .as_deref()
                .unwrap_or_default()
                .contains("operator approval")
        );
    }

    #[test]
    fn report_to_main_handoffs_evidence_blockers_to_verify_when_present() {
        let root = unique_temp_dir("conductor-evidence-handoff");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "review-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Working,
                current_task_id: None,
                current_summary: Some("direct review pane ready".to_string()),
                terminal_label: Some("review-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert review worker");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "verify-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Verifier,
                session_ref: None,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("direct verify pane ready".to_string()),
                terminal_label: Some("verify-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert verify worker");

        report_to_main(
            &store,
            "demo-run",
            "review-1",
            "blocked: need stronger test evidence before claiming done",
            None,
        )
        .expect("report should succeed");

        let verify_worker = store
            .read_worker("demo-run", "verify-1")
            .expect("verify worker should be readable");
        assert_eq!(verify_worker.state, WorkerState::AwaitingReport);
        assert_eq!(
            verify_worker.reason.as_deref(),
            Some("operator_followup_sent")
        );
        let events = store
            .read_events("demo-run")
            .expect("events should be readable");
        assert!(events.iter().any(|event| {
            event.event == EventKind::HandoffNeeded
                && event.reason.as_deref() == Some("blocked_evidence_handoff")
                && event.worker.as_deref() == Some("verify-1")
        }));
    }

    #[test]
    fn report_to_main_handoffs_dependency_blockers_to_explore_when_present() {
        let root = unique_temp_dir("conductor-dependency-handoff");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "build-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Working,
                current_task_id: None,
                current_summary: Some("direct build pane ready".to_string()),
                terminal_label: Some("build-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert build worker");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "explore-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("direct explore pane ready".to_string()),
                terminal_label: Some("explore-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert explore worker");

        report_to_main(
            &store,
            "demo-run",
            "build-1",
            "blocked: waiting on an MCP dependency",
            None,
        )
        .expect("report should succeed");

        let explore_worker = store
            .read_worker("demo-run", "explore-1")
            .expect("explore worker should be readable");
        assert_eq!(explore_worker.state, WorkerState::AwaitingReport);
        assert_eq!(
            explore_worker.reason.as_deref(),
            Some("operator_followup_sent")
        );
        let events = store
            .read_events("demo-run")
            .expect("events should be readable");
        assert!(events.iter().any(|event| {
            event.event == EventKind::HandoffNeeded
                && event.reason.as_deref() == Some("blocked_dependency_handoff")
                && event.worker.as_deref() == Some("explore-1")
        }));
    }

    #[test]
    fn report_to_main_retries_scope_blockers_in_place() {
        let root = unique_temp_dir("conductor-scope-handoff");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "review-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Working,
                current_task_id: None,
                current_summary: Some("direct review pane ready".to_string()),
                terminal_label: Some("review-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert review worker");

        report_to_main(
            &store,
            "demo-run",
            "review-1",
            "blocked: scope is too broad and needs context",
            None,
        )
        .expect("report should succeed");

        let review_worker = store
            .read_worker("demo-run", "review-1")
            .expect("review worker should be readable");
        assert_eq!(review_worker.state, WorkerState::AwaitingReport);
        assert_eq!(
            review_worker.reason.as_deref(),
            Some("operator_followup_sent")
        );
    }

    #[test]
    fn classify_structured_worker_report_respects_explicit_handoff_kind() {
        let (state, reason, kind) = classify_structured_worker_report(
            WorkerReportKind::Handoff,
            "handoff: verify this branch",
        );
        assert_eq!(state, WorkerState::Working);
        assert_eq!(reason, "handoff_requested_from_lane");
        assert_eq!(kind, WorkerReportKind::Handoff);
    }

    #[test]
    fn suggested_operator_command_uses_real_approval_task_ids() {
        let mut snapshot = sample_snapshot();
        snapshot.decision.next_action = "review-approval".to_string();
        snapshot.decision.focus_worker = Some("build-1".to_string());
        snapshot.workers = vec![crate::runtime::types::WorkerProjection {
            worker_id: "build-1".to_string(),
            worker_kind: WorkerKind::Worker,
            state: WorkerState::Blocked,
            current_task_id: Some("task-build-1".to_string()),
            current_summary: Some("blocked: waiting for operator approval".to_string()),
            last_heartbeat_at: None,
            terminal_label: Some("build-1".to_string()),
            reason: Some("blocked_approval_reported_to_operator".to_string()),
        }];
        let command = suggested_operator_command(
            "demo-run",
            &snapshot,
            Some("blocked_approval_reported_to_operator"),
        )
        .expect("command should exist");
        assert!(command.contains("task-build-1"));
    }

    #[test]
    fn handoff_worker_lane_pushes_work_into_the_target_lane() {
        let root = unique_temp_dir("conductor-handoff");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "explore-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("direct explore pane ready".to_string()),
                terminal_label: Some("explore-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert explore worker");

        let payload = handoff_worker_lane(
            &store,
            "demo-run",
            "review-1",
            "explore-1",
            "inspect the missing dependency only",
        )
        .expect("handoff should succeed");

        assert_eq!(
            payload.get("to_worker").and_then(|value| value.as_str()),
            Some("explore-1")
        );
        let explore_worker = store
            .read_worker("demo-run", "explore-1")
            .expect("explore worker should be readable");
        assert_eq!(explore_worker.state, WorkerState::AwaitingReport);
        assert_eq!(
            explore_worker.reason.as_deref(),
            Some("operator_followup_sent")
        );
        let events = store
            .read_events("demo-run")
            .expect("events should be readable");
        assert!(events.iter().any(|event| {
            event.event == EventKind::HandoffNeeded
                && event.reason.as_deref() == Some("worker_to_worker_handoff")
                && event.worker.as_deref() == Some("explore-1")
        }));
    }

    #[test]
    fn build_relaunch_prompt_for_worker_recovers_the_last_lane_assignment() {
        let worker = WorkerRecord {
            worker_id: "explore-1".to_string(),
            run_id: "demo-run".to_string(),
            worker_kind: WorkerKind::Worker,
            session_ref: None,
            state: WorkerState::AwaitingReport,
            current_task_id: None,
            current_summary: Some(
                "Inspect this repository for structure only. Identify top-level directories."
                    .to_string(),
            ),
            terminal_label: Some("explore-1".to_string()),
            last_heartbeat_at: Some(Utc::now()),
            last_stdout_at: None,
            last_event_at: Some(Utc::now()),
            reason: Some("operator_followup_sent".to_string()),
        };

        let prompt = build_relaunch_prompt_for_worker(&worker);
        assert!(prompt.contains("Inspect this repository for structure only."));
    }

    #[test]
    fn approval_resumption_requeues_the_owner_lane() {
        let root = unique_temp_dir("conductor-approval-resume");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "build-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Blocked,
                current_task_id: Some("task-build-1".to_string()),
                current_summary: Some("blocked: waiting for operator approval".to_string()),
                terminal_label: Some("build-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("blocked_approval_reported_to_operator".to_string()),
            })
            .expect("failed to upsert worker");
        let task = TaskRecord {
            task_id: "task-build-1".to_string(),
            run_id: "demo-run".to_string(),
            title: "Ship the build lane".to_string(),
            description: None,
            status: crate::runtime::types::TaskStatus::Pending,
            owner: Some("build-1".to_string()),
            claim: None,
            depends_on: Vec::new(),
            blocked_by: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            result: None,
            error: None,
            approval_status: Some(ApprovalStatus::Approved),
            approval_reason: Some("approval granted".to_string()),
            approval_reviewer: Some("operator".to_string()),
            approval_updated_at: Some(Utc::now()),
            metadata: serde_json::Map::new(),
        };

        maybe_resume_lane_after_approval(&store, "demo-run", "task-build-1", "approved", &task);

        let worker = store
            .read_worker("demo-run", "build-1")
            .expect("worker should be readable");
        assert_eq!(worker.state, WorkerState::AwaitingReport);
        assert_eq!(worker.reason.as_deref(), Some("operator_followup_sent"));
        let mailbox = store
            .read_mailbox("demo-run", "build-1")
            .expect("mailbox should be readable");
        assert_eq!(mailbox.records.len(), 1);
        assert!(mailbox.records[0].body.contains("Approval granted"));
    }

    #[test]
    fn read_mailbox_returns_empty_records_for_missing_operator_inbox() {
        let root = unique_temp_dir("conductor-inbox-empty");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");

        let mailbox = store
            .read_mailbox("demo-run", "main")
            .expect("mailbox should be readable");
        assert!(mailbox.records.is_empty());
    }

    #[test]
    fn cleanup_default_surface_state_removes_stale_team_workers_and_sessions() {
        let root = unique_temp_dir("conductor-cleanup");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");

        store
            .upsert_worker(WorkerRecord {
                worker_id: "main".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Orchestrator,
                session_ref: None,
                state: WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("surface".to_string()),
                terminal_label: Some("main".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: None,
            })
            .expect("failed to upsert main worker");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "explore-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::Working,
                current_task_id: None,
                current_summary: Some("direct explore pane ready".to_string()),
                terminal_label: Some("explore-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_pane".to_string()),
            })
            .expect("failed to upsert stale worker");

        let base_session = SessionRecord {
            run_id: "demo-run".to_string(),
            session_id: "session-main".to_string(),
            worker_id: "main".to_string(),
            socket_path: root.join("missing.sock").display().to_string(),
            stdout_path: root.join("stdout.log").display().to_string(),
            stderr_path: root.join("stderr.log").display().to_string(),
            pid: std::process::id(),
            child_pid: None,
            program: "/bin/sh".to_string(),
            args: Vec::new(),
            status: SessionStatus::Stopped,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            exited_at: None,
            exit_code: None,
        };
        store
            .write_session(&base_session)
            .expect("failed to write main session");
        store
            .write_session(&SessionRecord {
                session_id: "session-explore-1".to_string(),
                worker_id: "explore-1".to_string(),
                ..base_session.clone()
            })
            .expect("failed to write stale worker session");

        cleanup_default_surface_state(&store, "demo-run").expect("cleanup failed");

        let worker_ids = store
            .list_worker_ids("demo-run")
            .expect("list workers failed");
        assert!(worker_ids.contains(&"main".to_string()));
        assert!(worker_ids.contains(&"orchestrator-main".to_string()));
        assert!(!worker_ids.contains(&"explore-1".to_string()));

        let session_ids = store
            .list_session_ids("demo-run")
            .expect("list sessions failed");
        assert!(!session_ids.contains(&"session-main".to_string()));
        assert!(!session_ids.contains(&"session-explore-1".to_string()));
    }

    #[test]
    fn reclaim_expired_claims_returns_in_progress_tasks_to_pending() {
        let root = unique_temp_dir("conductor-reclaim");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .create_task("demo-run", "task-1", "Inspect", Some("inspect".to_string()))
            .expect("failed to create task");
        let mut task = store
            .read_task("demo-run", "task-1")
            .expect("task should be readable");
        task.status = TaskStatus::InProgress;
        task.owner = Some("explore-1".to_string());
        task.claim = Some(crate::runtime::types::TaskClaim {
            owner: "explore-1".to_string(),
            token: "claim-task-1-explore-1".to_string(),
            leased_until: Utc::now() - chrono::Duration::minutes(1),
        });
        store.write_task(&task).expect("failed to write task");

        let reclaimed = reclaim_expired_claims(&store, "demo-run").expect("reclaim should succeed");
        assert_eq!(reclaimed.len(), 1);

        let reread = store
            .read_task("demo-run", "task-1")
            .expect("task should be readable");
        assert_eq!(reread.status, TaskStatus::Pending);
        assert!(reread.claim.is_none());
        assert!(reread.owner.is_none());

        let snapshot = store
            .read_snapshot("demo-run")
            .expect("snapshot should be readable");
        assert_eq!(snapshot.monitor.reclaimed_claims, 1);
    }

    #[test]
    fn resolve_surface_launch_uses_codex_resume_for_resume_open() {
        let cfg = sample_config_with_workers(BTreeMap::new());
        let launch =
            resolve_surface_launch(&cfg, "demo-run", true).expect("surface launch should resolve");
        assert!(launch.program.ends_with("codex"));
        assert_eq!(launch.args, vec!["resume".to_string()]);
        assert!(launch.stdin_payload.is_none());
    }

    #[test]
    fn native_resume_depends_only_on_tmux_availability() {
        let cfg = sample_config_with_workers(BTreeMap::new());
        assert_eq!(should_use_native_resume(&cfg), !command_available("tmux"));
    }

    #[test]
    fn parse_worker_host_pid_matches_the_target_run() {
        let line = "93970 /opt/homebrew/bin/conductor worker-host ledart-app explore-1 session-explore-1 /tmp/socket /tmp/stdout /tmp/stderr /opt/homebrew/bin/codex";
        assert_eq!(parse_worker_host_pid(line, "ledart-app"), Some(93970));
        assert_eq!(parse_worker_host_pid(line, "other-run"), None);
    }

    #[test]
    fn legacy_ops_session_is_solo_when_only_main_lane_remains() {
        let root = unique_temp_dir("conductor-legacy-solo");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        assert!(
            legacy_ops_session_is_solo(&store, "demo-run").expect("legacy check should succeed")
        );
    }

    #[test]
    fn legacy_ops_session_is_not_solo_when_team_lane_is_live() {
        let root = unique_temp_dir("conductor-legacy-team");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "explore-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Worker,
                session_ref: None,
                state: WorkerState::AwaitingReport,
                current_task_id: None,
                current_summary: Some("direct explore pane ready".to_string()),
                terminal_label: Some("explore-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("direct_team_bootstrapping".to_string()),
            })
            .expect("failed to upsert worker");
        store
            .refresh_snapshot("demo-run")
            .expect("failed to refresh snapshot");
        assert!(
            !legacy_ops_session_is_solo(&store, "demo-run").expect("legacy check should succeed")
        );
    }

    #[test]
    fn maybe_complete_run_after_operator_settlement_keeps_ralph_runs_open_after_accept() {
        let root = unique_temp_dir("conductor-ralph-close-guard");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        let previous_state_dir = std::env::var_os("CONDUCTOR_STATE_DIR");
        unsafe {
            std::env::set_var("CONDUCTOR_STATE_DIR", &root);
        }
        enable_ralph_loop_for_root(&root, "demo-run").expect("failed to enable ralph");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "verify-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Verifier,
                session_ref: None,
                state: WorkerState::Stopped,
                current_task_id: None,
                current_summary: Some("closed after verification".to_string()),
                terminal_label: Some("verify-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("accepted_by_operator".to_string()),
            })
            .expect("failed to upsert worker");

        let completed =
            maybe_complete_run_after_operator_settlement(&store, "demo-run", "verify-1")
                .expect("run completion check should succeed");
        assert!(!completed);

        let run = store.read_run("demo-run").expect("run should still exist");
        assert!(run.active);
        assert_ne!(run.current_phase, RunPhase::Complete);
        match previous_state_dir {
            Some(value) => unsafe {
                std::env::set_var("CONDUCTOR_STATE_DIR", value);
            },
            None => unsafe {
                std::env::remove_var("CONDUCTOR_STATE_DIR");
            },
        }
    }

    #[test]
    fn maybe_complete_run_after_operator_settlement_closes_ralph_runs_after_explicit_close() {
        let root = unique_temp_dir("conductor-ralph-explicit-close");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        let previous_state_dir = std::env::var_os("CONDUCTOR_STATE_DIR");
        unsafe {
            std::env::set_var("CONDUCTOR_STATE_DIR", &root);
        }
        enable_ralph_loop_for_root(&root, "demo-run").expect("failed to enable ralph");
        store
            .upsert_worker(WorkerRecord {
                worker_id: "verify-1".to_string(),
                run_id: "demo-run".to_string(),
                worker_kind: WorkerKind::Verifier,
                session_ref: None,
                state: WorkerState::Stopped,
                current_task_id: None,
                current_summary: Some("closed after verification".to_string()),
                terminal_label: Some("verify-1".to_string()),
                last_heartbeat_at: Some(Utc::now()),
                last_stdout_at: None,
                last_event_at: Some(Utc::now()),
                reason: Some("closed_by_operator".to_string()),
            })
            .expect("failed to upsert worker");

        let completed =
            maybe_complete_run_after_operator_settlement(&store, "demo-run", "verify-1")
                .expect("run completion check should succeed");
        assert!(completed);

        let run = store.read_run("demo-run").expect("run should still exist");
        assert!(!run.active);
        assert_eq!(run.current_phase, RunPhase::Complete);
        let loop_state_path = root.join("runs").join("demo-run").join("ralph_loop.json");
        let loop_state: RalphLoopState = serde_json::from_str(
            &fs::read_to_string(loop_state_path).expect("loop state file should exist"),
        )
        .expect("loop state should deserialize");
        assert!(!loop_state.enabled);
        match previous_state_dir {
            Some(value) => unsafe {
                std::env::set_var("CONDUCTOR_STATE_DIR", value);
            },
            None => unsafe {
                std::env::remove_var("CONDUCTOR_STATE_DIR");
            },
        }
    }

    #[test]
    fn reopen_run_for_ralph_reactivates_completed_runs() {
        let root = unique_temp_dir("conductor-ralph-reopen");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let store = StateStore::new(&root);
        store
            .init_run("demo-run", "orchestrator-main")
            .expect("failed to init run");
        let mut run = store.read_run("demo-run").expect("failed to read run");
        run.active = false;
        run.current_phase = RunPhase::Complete;
        run.completed_at = Some(Utc::now());
        run.stop_reason = Some("settled".to_string());
        store.write_run(&run).expect("failed to write run");

        reopen_run_for_ralph(&store, "demo-run").expect("failed to reopen run");

        let reopened = store.read_run("demo-run").expect("failed to read reopened run");
        assert!(reopened.active);
        assert_eq!(reopened.current_phase, RunPhase::Executing);
        assert!(reopened.completed_at.is_none());
        assert!(reopened.stop_reason.is_none());
    }

    #[test]
    fn team_launch_args_prepend_no_alt_screen_for_codex_workers() {
        let launch = crate::runtime::adapters::WorkerAdapterLaunch {
            program: "/opt/homebrew/bin/codex".to_string(),
            args: vec!["-m".to_string(), "gpt-5.4".to_string()],
            cwd: None,
            stdin_payload: None,
            env: BTreeMap::new(),
        };
        assert_eq!(
            team_launch_args(&launch),
            vec![
                "--no-alt-screen".to_string(),
                "-m".to_string(),
                "gpt-5.4".to_string()
            ]
        );
    }

    #[test]
    fn direct_codex_team_launch_embeds_the_initial_prompt() {
        let launch = crate::runtime::adapters::WorkerAdapterLaunch {
            program: "/opt/homebrew/bin/codex".to_string(),
            args: vec!["-m".to_string(), "gpt-5.4".to_string()],
            cwd: None,
            stdin_payload: None,
            env: BTreeMap::new(),
        };
        let command = build_direct_launch_shell_command(
            Path::new("/tmp/demo"),
            &launch,
            Some("Inspect this repository and report upward."),
        );
        assert!(command.contains("codex"));
        assert!(command.contains("--no-alt-screen"));
        assert!(command.contains("Inspect this repository and report upward."));
    }

    #[test]
    fn find_main_pane_id_falls_back_to_pane_zero_when_titles_drift() {
        let panes = "%166\t0\t⠴ conductor-kit\tcodex-aarch64-a\n%168\t1\tmain\tconductor\n";
        let main = find_main_pane_id("demo-session", panes).expect("main pane should resolve");
        assert_eq!(main, "%166");
    }

    #[test]
    fn find_worker_tmux_pane_id_matches_exact_title() {
        let panes = "%166\tmain\n%167\texplore-1\n%168\treview-1\n";
        let found = panes.lines().find_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let pane_id = parts.next()?.trim().to_string();
            let pane_title = parts.next()?.trim();
            if pane_title == "review-1" {
                Some(pane_id)
            } else {
                None
            }
        });
        assert_eq!(found.as_deref(), Some("%168"));
    }

    #[test]
    fn extract_metric_prefers_capture_group_one() {
        let metric = extract_metric("mean: 12.50 ms", r"mean:\s*([0-9.]+)")
            .expect("regex should parse")
            .expect("metric should match");
        assert_eq!(metric, 12.5);
    }

    #[test]
    fn metric_improved_respects_direction() {
        assert!(metric_improved(9.0, 10.0, MetricDirection::LowerIsBetter));
        assert!(!metric_improved(11.0, 10.0, MetricDirection::LowerIsBetter));
        assert!(metric_improved(11.0, 10.0, MetricDirection::HigherIsBetter));
        assert!(!metric_improved(9.0, 10.0, MetricDirection::HigherIsBetter));
    }

    #[test]
    fn parse_autoresearch_setup_args_accepts_repeated_scope_and_constraints() {
        let parsed = parse_autoresearch_setup_args(&[
            "--run".to_string(),
            "bench-run".to_string(),
            "--goal".to_string(),
            "reduce benchmark time".to_string(),
            "--metric-command".to_string(),
            "cargo bench".to_string(),
            "--metric-regex".to_string(),
            "time:\\s*([0-9.]+)".to_string(),
            "--direction".to_string(),
            "lower".to_string(),
            "--in-scope".to_string(),
            "src".to_string(),
            "--in-scope".to_string(),
            "tests".to_string(),
            "--out-of-scope".to_string(),
            "docs".to_string(),
            "--constraint".to_string(),
            "no new dependencies".to_string(),
            "--constraint".to_string(),
            "keep tests green".to_string(),
            "--max-experiments".to_string(),
            "12".to_string(),
        ])
        .expect("setup args should parse");

        assert_eq!(parsed.run_id, "bench-run");
        assert_eq!(parsed.in_scope_files, vec!["src", "tests"]);
        assert_eq!(parsed.out_of_scope_files, vec!["docs"]);
        assert_eq!(parsed.constraints.len(), 2);
        assert_eq!(parsed.max_experiments, Some(12));
    }

    #[test]
    fn validate_autoresearch_scope_rejects_out_of_scope_changes() {
        let cfg = AutoresearchConfig {
            schema_version: 1,
            run_id: "demo".to_string(),
            repo_root: "/tmp/demo".to_string(),
            branch: "feat/autoresearch-20260408".to_string(),
            goal: "reduce benchmark time".to_string(),
            metric_command: "cargo bench".to_string(),
            metric_regex: "([0-9.]+)".to_string(),
            metric_direction: "lower".to_string(),
            in_scope_files: vec!["src".to_string(), "tests".to_string()],
            out_of_scope_files: vec!["docs".to_string()],
            constraints: Vec::new(),
            max_experiments: None,
            simplicity_policy: "smallest change wins".to_string(),
            baseline_metric: 10.0,
            best_metric: 10.0,
            baseline_commit: "abc".to_string(),
            best_commit: "abc".to_string(),
            experiment_count: 0,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            stopped_at: None,
        };

        let err = validate_autoresearch_scope(
            &cfg,
            &["src/main.rs".to_string(), "docs/README.md".to_string()],
        )
        .expect_err("docs change should be rejected");
        assert!(err.contains("docs/README.md"));
    }

    #[test]
    fn managed_skill_names_include_autoresearch() {
        let names = managed_skill_names();
        assert!(names.contains(&"autoresearch"));
        assert!(names.contains(&"conductor"));
        assert!(names.contains(&"team"));
    }

    #[test]
    fn autoresearch_next_action_guides_setup_progress_and_resume() {
        let now = Utc::now();
        let mut cfg = AutoresearchConfig {
            schema_version: 1,
            run_id: "demo".to_string(),
            repo_root: "/tmp/demo".to_string(),
            branch: "feat/autoresearch-20260408".to_string(),
            goal: "reduce benchmark time".to_string(),
            metric_command: "cargo bench".to_string(),
            metric_regex: "([0-9.]+)".to_string(),
            metric_direction: "lower".to_string(),
            in_scope_files: vec!["src".to_string()],
            out_of_scope_files: Vec::new(),
            constraints: Vec::new(),
            max_experiments: None,
            simplicity_policy: "smallest change wins".to_string(),
            baseline_metric: 10.0,
            best_metric: 10.0,
            baseline_commit: "abc".to_string(),
            best_commit: "abc".to_string(),
            experiment_count: 0,
            started_at: now,
            updated_at: now,
            stopped_at: None,
        };
        assert!(autoresearch_next_action(&cfg).contains("make one focused change"));
        cfg.experiment_count = 2;
        assert!(autoresearch_next_action(&cfg).contains("inspect the latest result"));
        cfg.stopped_at = Some(now);
        assert!(
            autoresearch_next_action(&cfg)
                .contains("resume with `conductor autoresearch continue`")
        );
    }

    #[test]
    fn build_ralph_operator_prompt_includes_focus_and_next_command() {
        let snapshot = sample_snapshot();
        let prompt = build_ralph_operator_prompt("demo-run", &snapshot);
        assert!(prompt.contains("Ralph loop active for run demo-run."));
        assert!(prompt.contains("Current focus: explore-1."));
        assert!(
            prompt.contains(
                "Do not widen into a team unless a worker count was explicitly requested."
            )
        );
        assert!(prompt.contains("Suggested command: conductor handoff main explore-1"));
    }

    #[test]
    fn remove_existing_skill_target_handles_symlink() {
        let root = unique_temp_dir("conductor-remove-symlink");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let source = root.join("source");
        fs::create_dir_all(&source).expect("failed to create source");
        let target = root.join("target");
        std::os::unix::fs::symlink(&source, &target).expect("failed to create symlink");

        remove_existing_skill_target(&target).expect("failed to remove symlink");
        assert!(!target.exists());
    }
}
