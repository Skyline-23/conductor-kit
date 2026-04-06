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
use crate::runtime::claims::{acquire_claim, release_claim};
use crate::runtime::hooks::{event_name_of, filter_events, watch_and_run_hooks};
use crate::runtime::phases::transition_phase;
use crate::runtime::sessions::{
    SessionCommand, run_worker_host, send_session_command, spawn_session,
};
use crate::runtime::state_store::StateStore;
use crate::runtime::types::{
    DispatchStatus, EventEnvelope, EventKind, RunPhase, SCHEMA_VERSION, SessionStatus, WorkerKind,
    WorkerRecord, WorkerState,
};
use crate::runtime::workers::{WorkerLaunchSpec, execute_worker};
use chrono::Utc;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::cursor::MoveTo;
use crossterm::terminal::{
    Clear as TerminalClear, ClearType as TerminalClearType, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::fs::File;
use std::io::{IsTerminal, Stdout, stdin, stdout};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
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
        "init" => run_start(&args[1..]),
        "resume" => run_open(&args[1..]),
        "team" => run_team(&args[1..]),
        "ralph" => run_ralph(&args[1..]),
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
        "task-release" => run_task_release(&args[1..]),
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
        "worker-host" => run_worker_host_command(&args[1..]),
        "dispatch-route" => run_dispatch_route(&args[1..]),
        "hud-view" => run_hud_view(&args[1..]),
        "hud-watch" => run_hud_watch(&args[1..]),
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

fn run_default() -> Result<(), String> {
    let run_id = default_run_id();
    let store = StateStore::new(resolve_state_root()?);
    if store
        .root()
        .join("runs")
        .join(&run_id)
        .join("run.json")
        .exists()
    {
        run_open(&[])
    } else {
        run_start(&[])
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
    run_surface_ops_open(&run_id)
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
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, &run_id)?;
    run_surface_ops_open(&run_id)
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
    let (run_id, count_index) = match args.first().map(String::as_str) {
        Some(first) if first.parse::<usize>().is_ok() => (default_run_id(), 0),
        Some(first) if !first.trim().is_empty() => (first.to_string(), 1),
        _ => {
            return Err(
                "team requires <count> <agent> [agent...] or <run_id> <count> <agent> [agent...]"
                    .to_string(),
            );
        }
    };

    let team_size = args
        .get(count_index)
        .ok_or_else(|| {
            "team requires <count> <agent> [agent...] or <run_id> <count> <agent> [agent...]"
                .to_string()
        })?
        .parse::<usize>()
        .map_err(|err| err.to_string())?;
    if team_size == 0 {
        return Err("team count must be at least 1".to_string());
    }

    let agent_names = args
        .iter()
        .skip(count_index + 1)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if agent_names.is_empty() {
        return Err("team requires at least one agent name".to_string());
    }

    ensure_explicit_team_sessions(&run_id, team_size, &agent_names)?;
    let tmux_session_name =
        current_tmux_session_hint().unwrap_or_else(|| default_tmux_session_name(&run_id));
    run_ops_open_with_filter(&run_id, &tmux_session_name, None)
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
    ensure_team_sessions(&run_id, TeamMode::Ralph, requested_width)?;
    let tmux_session_name =
        current_tmux_session_hint().unwrap_or_else(|| default_tmux_session_name(&run_id));
    run_ops_open_with_filter(&run_id, &tmux_session_name, None)
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
    let launch = resolve_surface_launch(cfg, run_id)?;
    let desired_kind = WorkerKind::Orchestrator;
    let worker_id = "main";
    if let Ok(existing) = store.read_session(run_id, &format!("session-{worker_id}")) {
        let mut worker = store.read_worker(run_id, worker_id)?;
        if worker.worker_kind != desired_kind {
            worker.worker_kind = desired_kind.clone();
            let _ = store.upsert_worker(worker)?;
        }
        if existing.status == SessionStatus::Running || existing.status == SessionStatus::Starting
        {
            if existing.program == launch.program && existing.args == launch.args {
                return Ok(());
            }
            let _ = send_session_command(
                Path::new(&existing.socket_path),
                &SessionCommand::Stop,
            );
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

fn run_surface_ops_open(run_id: &str) -> Result<(), String> {
    let tmux_session_name = format!("conductor-{run_id}-surface");
    let (_, cfg) = load_resolved_config()?;
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let state_root = resolve_state_root()?;
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
    let launch = resolve_surface_launch(&cfg, run_id)?;
    let surface_cmd = build_launch_shell_command(&cwd, &launch);
    let pane_specs = vec![("main".to_string(), "surface".to_string(), surface_cmd)];
    if tmux_session_exists(&tmux_session_name)? {
        run_tmux(["kill-session", "-t", &tmux_session_name])?;
    }
    if command_available("tmux")
        && env::var_os("TMUX").is_none()
        && env::var("CONDUCTOR_OPS_NO_ATTACH").ok().as_deref() != Some("1")
    {
        return run_surface_attached_tmux_session(
            &tmux_session_name,
            &hud_status_cmd,
            pane_specs[0].0.as_str(),
            pane_specs[0].2.as_str(),
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
    run_tmux(["set-option", "-t", session_name, "status-position", "bottom"])?;
    run_tmux(["set-option", "-t", session_name, "status-justify", "left"])?;
    run_tmux(["set-option", "-t", session_name, "status-left-length", "240"])?;
    run_tmux(["set-option", "-t", session_name, "status-right", ""])?;
    run_tmux(["set-option", "-t", session_name, "status-interval", "1"])?;
    run_tmux([
        "set-option",
        "-t",
        session_name,
        "status-left",
        &format!("#({hud_status_cmd})"),
    ])?;
    run_tmux(["select-pane", "-t", main_pane_id.trim(), "-T", main_title])?;
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
    attach_tmux_ops_session(session_name)
}

fn resolve_surface_launch(
    cfg: &Config,
    run_id: &str,
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
        format!("conductor-{run_id}-surface"),
    );
    resolve_worker_adapter(&adapter, run_id, "main", None, None)
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
            pane_specs.push((worker.worker_id, session_id, attach_cmd));
        }
    }
    pane_specs.sort_by_key(|(worker_id, _, _)| pane_sort_key(worker_id));

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
            "sessions": pane_specs.iter().map(|(worker_id, session_id, _)| json!({
                "worker_id": worker_id,
                "session_id": session_id
            })).collect::<Vec<_>>()
        }))
    } else {
        let terminal_app = env::var("CONDUCTOR_TERMINAL_APP")
            .ok()
            .unwrap_or_else(|| "Terminal".to_string());
        open_terminal_script(&terminal_app, &hud_cmd)?;
        for (_, _, attach_cmd) in &pane_specs {
            open_terminal_script(&terminal_app, attach_cmd)?;
        }
        print_json(&json!({
            "ok": true,
            "run_id": run_id,
            "terminal_app": terminal_app,
            "fallback": "terminal_windows",
            "hud": true,
            "sessions": pane_specs.iter().map(|(worker_id, session_id, _)| json!({
                "worker_id": worker_id,
                "session_id": session_id
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
    println!();
    println!("workers");
    println!("-------");
    for worker in snapshot.workers {
        println!(
            "{} | kind={:?} | state={:?} | task={} | summary={}",
            worker.worker_id,
            worker.worker_kind,
            worker.state,
            worker.current_task_id.unwrap_or_else(|| "-".to_string()),
            worker.current_summary.unwrap_or_else(|| "-".to_string())
        );
    }
    Ok(())
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
        println!();
        println!("workers");
        println!("-------");
        for worker in snapshot.workers {
            println!(
                "{} | kind={:?} | state={:?} | task={} | summary={}",
                worker.worker_id,
                worker.worker_kind,
                worker.state,
                worker.current_task_id.unwrap_or_else(|| "-".to_string()),
                worker.current_summary.unwrap_or_else(|| "-".to_string())
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

fn render_hud_strip(
    snapshot: &crate::runtime::types::RuntimeSnapshot,
    ansi: bool,
) -> String {
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
                crate::runtime::types::WorkerState::Working | crate::runtime::types::WorkerState::Blocked
            )
        })
        .count();
    if ansi {
        format!(
            "\x1b[38;5;111m{}\x1b[0m  \x1b[38;5;150m{:?}\x1b[0m  auth:{}  tasks {}/{}/{}/{}/{}  workers {}/{}  mail:{}",
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
            snapshot.mailbox.unread,
        )
    } else {
        format!(
            "{}  {:?}  auth:{}  tasks {}/{}/{}/{}/{}  workers {}/{}  mail:{}",
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
            snapshot.mailbox.unread,
        )
    }
}

fn run_events_list(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "events-list requires <run_id> [event_name]")?;
    let event_name = args.get(1).map(String::as_str);
    let store = StateStore::new(resolve_state_root()?);
    let events = filter_events(store.read_events(run_id)?, event_name);
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
        env_parts.push(format!("CONDUCTOR_WORKER_STDIN={}", shell_quote_str(payload)));
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
    pane_specs: &[(String, String, String)],
) -> Result<bool, String> {
    if tmux_session_exists(session_name)? {
        run_tmux(["set-option", "-t", session_name, "mouse", "on"])?;
        run_tmux(["set-option", "-t", session_name, "set-clipboard", "on"])?;
        sync_existing_tmux_ops_session(session_name, pane_specs)?;
        return Ok(false);
    }

    let (main_title, main_cmd) = if let Some((worker_id, _, attach_cmd)) = pane_specs.first() {
        (worker_id.as_str(), attach_cmd.as_str())
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
        for (_, _, attach_cmd) in pane_specs.iter().skip(1) {
            run_tmux([
                "split-window",
                "-h",
                "-t",
                &format!("{session_name}:0.0"),
                attach_cmd,
            ])?;
        }
    }

    if include_hud && pane_specs.len() > 1 {
        run_tmux([
            "select-layout",
            "-t",
            &format!("{session_name}:0"),
            "tiled",
        ])?;
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
    for (index, (worker_id, _, _)) in pane_specs.iter().skip(1).enumerate() {
        run_tmux([
            "select-pane",
            "-t",
            &format!("{session_name}:0.{}", index + pane_title_offset),
            "-T",
            worker_id,
        ])?;
    }

    Ok(true)
}

fn sync_existing_tmux_ops_session(
    session_name: &str,
    pane_specs: &[(String, String, String)],
) -> Result<(), String> {
    let panes = run_tmux_capture([
        "list-panes",
        "-t",
        &format!("{session_name}:0"),
        "-F",
        "#{pane_id}\t#{pane_title}",
    ])?;
    let mut title_to_pane = BTreeMap::new();
    let mut main_pane_id = None::<String>;
    for line in panes.lines() {
        let mut parts = line.splitn(2, '\t');
        let pane_id = parts.next().unwrap_or_default().trim().to_string();
        let pane_title = parts.next().unwrap_or_default().trim().to_string();
        if pane_title == "main" || pane_title == "conductor-kit" {
            main_pane_id = Some(pane_id.clone());
        }
        if !pane_title.is_empty() {
            title_to_pane.insert(pane_title, pane_id);
        }
    }
    let main_pane_id = main_pane_id.unwrap_or_else(|| format!("{session_name}:0.0"));
    let mut stack_target = title_to_pane
        .iter()
        .filter(|(title, _)| title.starts_with("explore-") || title.starts_with("build-") || title.starts_with("review-") || title.starts_with("verify-"))
        .map(|(_, pane_id)| pane_id.clone())
        .last()
        .unwrap_or_else(|| main_pane_id.clone());

    for (worker_id, _, attach_cmd) in pane_specs {
        if title_to_pane.contains_key(worker_id) {
            continue;
        }
        let split_direction = if stack_target == main_pane_id { "-h" } else { "-v" };
        let new_pane_id = run_tmux_capture([
            "split-window",
            split_direction,
            "-d",
            "-P",
            "-F",
            "#{pane_id}",
            "-t",
            &stack_target,
            attach_cmd,
        ])?;
        let new_pane_id = new_pane_id.trim().to_string();
        run_tmux(["select-pane", "-t", &new_pane_id, "-T", worker_id])?;
        stack_target = new_pane_id;
    }
    Ok(())
}

fn attach_tmux_ops_session(session_name: &str) -> Result<(), String> {
    let has_tty = stdin().is_terminal() && stdout().is_terminal();
    if env::var_os("TMUX").is_some() {
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

fn current_tmux_session_hint() -> Option<String> {
    env::var("CONDUCTOR_TMUX_SESSION")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
    let status = Command::new("tmux")
        .args(args_vec.iter().map(|value| value.as_str()))
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux command failed: {}", args_vec.join(" ")))
    }
}

fn default_tmux_session_name(run_id: &str) -> String {
    let sanitized = run_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("conductor-{sanitized}")
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
        RunSnapshot, RuntimeSnapshot, TaskCounts,
    };
    use chrono::Utc;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

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
            },
        }
    }

    #[test]
    fn sync_profile_for_cli_replaces_mismatched_model_and_reasoning() {
        let mut host_catalog = HostCatalog::default();
        host_catalog.claude = VendorCatalog {
            default_model: Some("claude-sonnet-4-6".to_string()),
            models: vec!["claude-sonnet-4-6".to_string(), "claude-opus-4-1".to_string()],
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
}
