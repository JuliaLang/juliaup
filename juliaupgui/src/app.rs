use egui::{Color32, RichText};
use egui_extras::{Column, TableBuilder};
use itertools::Itertools;
use juliaup::command_config_autoinstall::run_command_config_autoinstall;
use juliaup::command_config_manifestversiondetect::run_command_config_manifestversiondetect;
use juliaup::command_config_versionsdbupdate::run_command_config_versionsdbupdate;
use juliaup::command_default::run_command_default;
use juliaup::command_gc::run_command_gc;
use juliaup::command_override::{run_command_override_set, run_command_override_unset};
use juliaup::command_update_version_db::run_command_update_version_db;
use juliaup::config_file::{
    load_config_db, load_mut_config_db, save_config_db, JuliaupConfigChannel, JuliaupConfigSettings,
};
use juliaup::global_paths::GlobalPaths;
use juliaup::jsonstructs_versionsdb::JuliaupVersionDB;
use juliaup::operations::{get_channel_variations, get_julia_pr_title};
use juliaup::versions_file::load_versions_db;
use numeric_sort::cmp;
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Instant;

#[cfg(not(windows))]
use juliaup::command_config_symlinks::run_command_config_symlinks;

/// Julia logo circle data: (cx, cy, radius, R, G, B) in SVG coordinates (viewBox 0 0 350 350).
/// Source: https://github.com/JuliaLang/julia-logo-graphics
const JULIA_DOTS: [(f32, f32, f32, u8, u8, u8); 3] = [
    (88.4, 250.0, 75.0, 0xCB, 0x3C, 0x33),  // red
    (175.0, 100.0, 75.0, 0x38, 0x98, 0x26), // green
    (261.6, 250.0, 75.0, 0x95, 0x58, 0xB2), // purple
];

// ── domain models ─────────────────────────────────────────────────────────────

#[derive(Clone)]
struct InstalledRow {
    name: String,
    version: String,
    is_default: bool,
    update: Option<String>,
    pr_number: Option<String>,
}

#[derive(Clone)]
struct AvailableRow {
    channel: String,
    version: String,
    installed: bool,
}

#[derive(Clone)]
struct OverrideRow {
    path: String,
    channel: String,
}

#[derive(Clone)]
struct AppState {
    installed: Vec<InstalledRow>,
    available: Vec<AvailableRow>,
    overrides: Vec<OverrideRow>,
    settings: JuliaupConfigSettings,
}

// ── worker IPC ────────────────────────────────────────────────────────────────

enum Op {
    Reload,
    Add {
        channel: String,
        approve_pr_codesign: bool,
    },
    Remove(String),
    Update {
        channel: Option<String>,
        approve_pr_codesign: bool,
    },
    SetDefault(String),
    SelfUpdate,
    Gc,
    UpdateVersionDb,
    SetVersionsDbInterval(i64),
    SetAutoInstall(Option<bool>),
    SetManifestDetect(bool),
    #[cfg(not(windows))]
    SetChannelSymlinks(bool),
    SetOverride {
        path: String,
        channel: String,
    },
    UnsetOverride(String),
    UnsetNonexistentOverrides,
    Link {
        channel: String,
        target: String,
        args: Vec<String>,
    },
}

enum Msg {
    Loaded(Box<AppState>),
    PrTitleLoaded {
        number: String,
        title: Option<String>,
    },
    Line(String), // raw output from subprocess
    Ok(String),   // operation succeeded
    Err(String),  // operation failed
}

// Log entry kinds
#[derive(Clone, PartialEq)]
enum LogKind {
    Output,
    Ok,
    Err,
}

#[derive(Clone)]
struct LogEntry {
    text: String,
    kind: LogKind,
}

// ── tab ───────────────────────────────────────────────────────────────────────

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Installed,
    Available,
    Config,
    About,
}

#[derive(PartialEq, Clone, Copy)]
enum InstalledView {
    Tile,
    List,
}

#[derive(PartialEq, Clone, Copy)]
enum ThemeMode {
    System,
    Dark,
    Light,
}

#[derive(Clone)]
enum PendingPrAction {
    Install {
        channel: String,
        pr_number: String,
    },
    Update {
        channel: Option<String>,
        pr_numbers: Vec<String>,
    },
}

// ── app ───────────────────────────────────────────────────────────────────────

pub struct App {
    tab: Tab,
    installed_view: InstalledView,
    state: Option<AppState>,
    loading: bool,
    busy: bool,
    status: Option<(String, bool)>,
    juliaup_version: String,

    // Activity log
    current_op: Option<String>,
    log: Vec<LogEntry>,
    log_open: bool,

    // Installed tab inputs
    link_channel: String,
    link_target: String,
    link_args: String,

    // Available tab inputs
    filter: String,
    channel_prompt: Option<String>,
    channel_prompt_input: String,
    avail_expanded: HashSet<String>,

    // Custom launch prompt
    custom_launch_channel: Option<String>,
    custom_launch_project: String,
    custom_launch_args: String,
    custom_launch_env: String,

    // PR trust and macOS code-signing confirmation
    pending_pr_action: Option<PendingPrAction>,
    pr_titles: HashMap<String, String>,
    pending_pr_titles: HashSet<String>,

    // Config tab inputs
    interval_input: String,
    terminal_app: String,
    theme_mode: ThemeMode,

    // Show-once tips
    list_tip_dismissed: bool,

    // Override tab inputs
    ov_path: String,
    ov_channel: String,

    // Splash animation
    splash_start: Instant,

    op_tx: mpsc::SyncSender<(Op, Arc<GlobalPaths>)>,
    msg_rx: mpsc::Receiver<Msg>,
    msg_tx: mpsc::Sender<Msg>,
    repaint_ctx: egui::Context,
    paths: Arc<GlobalPaths>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, paths: GlobalPaths) -> Self {
        let paths = Arc::new(paths);
        let theme_mode = load_theme_pref(&paths);
        apply_theme(&cc.egui_ctx, theme_mode);
        let (op_tx, op_rx) = mpsc::sync_channel::<(Op, Arc<GlobalPaths>)>(8);
        let (msg_tx, msg_rx) = mpsc::channel::<Msg>();
        let app_msg_tx = msg_tx.clone();

        thread::spawn(move || worker(op_rx, msg_tx));
        let _ = op_tx.try_send((Op::Reload, paths.clone()));

        let juliaup_version = juliaup::get_own_version()
            .map(|v| format!("v{v}"))
            .unwrap_or_default();

        let terminal_app = load_terminal_pref(&paths);
        let installed_view = load_view_pref(&paths);
        let list_tip_dismissed = load_bool_pref(&paths, "juliaupgui_list_tip_dismissed");

        Self {
            tab: Tab::Installed,
            installed_view,
            state: None,
            loading: true,
            busy: true,
            status: None,
            juliaup_version,
            current_op: Some("Loading…".to_string()),
            log: Vec::new(),
            log_open: false,
            link_channel: String::new(),
            link_target: String::new(),
            link_args: String::new(),
            filter: String::new(),
            channel_prompt: None,
            channel_prompt_input: String::new(),
            avail_expanded: HashSet::new(),
            custom_launch_channel: None,
            custom_launch_project: String::new(),
            custom_launch_args: String::new(),
            custom_launch_env: String::new(),
            pending_pr_action: None,
            pr_titles: HashMap::new(),
            pending_pr_titles: HashSet::new(),
            interval_input: String::new(),
            terminal_app,
            theme_mode,
            list_tip_dismissed,
            ov_path: String::new(),
            ov_channel: String::new(),
            splash_start: Instant::now(),
            op_tx,
            msg_rx,
            msg_tx: app_msg_tx,
            repaint_ctx: cc.egui_ctx.clone(),
            paths,
        }
    }

    fn send(&mut self, op: Op) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = None;
        self.current_op = Some(op_label(&op));
        if self.op_tx.try_send((op, self.paths.clone())).is_err() {
            self.status = Some(("Internal error: operation channel full".to_string(), true));
            self.busy = false;
            self.current_op = None;
        }
    }

    fn request_pr_title(&mut self, number: &str) {
        if self.pr_titles.contains_key(number) || self.pending_pr_titles.contains(number) {
            return;
        }
        let Ok(parsed_number) = number.parse::<u64>() else {
            return;
        };

        let number = number.to_string();
        self.pending_pr_titles.insert(number.clone());
        let tx = self.msg_tx.clone();
        let repaint_ctx = self.repaint_ctx.clone();
        thread::spawn(move || {
            let title = get_julia_pr_title(parsed_number).ok();
            let _ = tx.send(Msg::PrTitleLoaded { number, title });
            repaint_ctx.request_repaint();
        });
    }

    fn request_install(&mut self, channel: String) {
        #[cfg(target_os = "macos")]
        if let Some(pr_number) = julia_pr_number(&channel).map(String::from) {
            self.request_pr_title(&pr_number);
            self.pending_pr_action = Some(PendingPrAction::Install { channel, pr_number });
            return;
        }

        self.send(Op::Add {
            channel,
            approve_pr_codesign: false,
        });
    }

    fn request_update(&mut self, channel: Option<String>) {
        #[cfg(target_os = "macos")]
        {
            let pr_numbers: Vec<String> = self
                .state
                .as_ref()
                .into_iter()
                .flat_map(|state| &state.installed)
                .filter(|row| {
                    row.pr_number.is_some()
                        && channel
                            .as_ref()
                            .is_none_or(|name| name == &row.name && row.update.is_some())
                })
                .filter_map(|row| row.pr_number.clone())
                .unique()
                .collect();

            if !pr_numbers.is_empty() {
                self.pending_pr_action = Some(PendingPrAction::Update {
                    channel,
                    pr_numbers,
                });
                return;
            }
        }

        self.send(Op::Update {
            channel,
            approve_pr_codesign: false,
        });
    }

    fn show_pr_confirmation(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending_pr_action.clone() else {
            return;
        };

        let mut open = true;
        let mut approve = false;
        let mut cancel = false;
        let (title, approve_label) = match &action {
            PendingPrAction::Install { .. } => (
                "Install unreviewed PR build?",
                if cfg!(target_os = "macos") {
                    "Install and Code Sign"
                } else {
                    "Install PR Build"
                },
            ),
            PendingPrAction::Update { .. } => {
                ("Update unreviewed PR build?", "Update and Code Sign")
            }
        };

        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_max_width(440.0);
                ui.label(
                    RichText::new(
                        "PR builds contain unmerged code that may not have been reviewed for security issues.",
                    )
                    .strong()
                    .color(warning_color(ui.visuals().dark_mode)),
                );
                ui.add_space(6.0);

                match &action {
                    PendingPrAction::Install {
                        channel,
                        pr_number,
                    } => {
                        ui.label(format!("Channel: {channel}"));
                        ui.horizontal(|ui| {
                            ui.label("Julia pull request:");
                            pr_link(
                                ui,
                                pr_number,
                                self.pr_titles.get(pr_number).map(String::as_str),
                            );
                        });
                    }
                    PendingPrAction::Update {
                        channel,
                        pr_numbers,
                    } => {
                        ui.label(match channel {
                            Some(channel) => format!("Channel: {channel}"),
                            None => "Update All may update installed PR channels.".to_string(),
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Julia pull requests:");
                            for pr_number in pr_numbers {
                                pr_link(
                                    ui,
                                    pr_number,
                                    self.pr_titles.get(pr_number).map(String::as_str),
                                );
                            }
                        });
                    }
                }

                #[cfg(target_os = "macos")]
                {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "The downloaded build is unsigned. Juliaup will apply a local ad-hoc signature to its executable files so macOS can run it. This does not establish that the code is trustworthy.",
                        )
                        .color(secondary_text(ui.visuals().dark_mode)),
                    );
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if accessible_button_name(ui.button(approve_label), approve_label).clicked() {
                        approve = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if approve {
            self.pending_pr_action = None;
            match action {
                PendingPrAction::Install { channel, .. } => self.send(Op::Add {
                    channel,
                    approve_pr_codesign: cfg!(target_os = "macos"),
                }),
                PendingPrAction::Update { channel, .. } => self.send(Op::Update {
                    channel,
                    approve_pr_codesign: true,
                }),
            }
        } else if cancel || !open {
            self.pending_pr_action = None;
        }
    }

    fn poll(&mut self) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
                Msg::Loaded(s) => {
                    let pr_numbers = s
                        .installed
                        .iter()
                        .filter_map(|row| row.pr_number.clone())
                        .unique()
                        .collect_vec();
                    self.interval_input = s.settings.versionsdb_update_interval.to_string();
                    self.state = Some(*s);
                    self.loading = false;
                    self.busy = false;
                    self.current_op = None;
                    for pr_number in pr_numbers {
                        self.request_pr_title(&pr_number);
                    }
                }
                Msg::PrTitleLoaded { number, title } => {
                    self.pending_pr_titles.remove(&number);
                    if let Some(title) = title {
                        self.pr_titles.insert(number, title);
                    }
                }
                Msg::Line(line) => {
                    self.log.push(LogEntry {
                        text: line,
                        kind: LogKind::Output,
                    });
                    self.log_open = true;
                }
                Msg::Ok(m) => {
                    self.log.push(LogEntry {
                        text: m.clone(),
                        kind: LogKind::Ok,
                    });
                    self.status = Some((m, false));
                    self.current_op = Some("Reloading…".to_string());
                    self.busy = true;
                    if self
                        .op_tx
                        .try_send((Op::Reload, self.paths.clone()))
                        .is_err()
                    {
                        self.busy = false;
                        self.current_op = None;
                    }
                }
                Msg::Err(m) => {
                    self.log.push(LogEntry {
                        text: m.clone(),
                        kind: LogKind::Err,
                    });
                    self.status = Some((m, true));
                    self.loading = false;
                    self.busy = false;
                    self.current_op = None;
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();

        let splash_t = self.splash_start.elapsed().as_secs_f32();
        let splash_active = splash_t < SPLASH_DURATION;
        if splash_active || self.loading || self.busy {
            ctx.request_repaint();
        }

        // ── title + tab bar ───────────────────────────────────────────────
        // We need the header logo rect to animate the dots sliding there
        let mut logo_screen_rect: Option<egui::Rect> = None;
        egui::TopBottomPanel::top("top")
            .min_height(40.0)
            .show(ctx, |ui| {
                ui.add_space(5.0);
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(10.0);
                    // Always allocate space for the logo so layout is stable
                    let (rect, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(26.0), egui::Sense::hover());
                    logo_screen_rect = Some(rect);
                    // Always paint the static logo
                    if ui.is_rect_visible(rect) {
                        paint_julia_dots(&ui.painter_at(rect), rect);
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new("Juliaup").size(20.0).strong());
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Installation manager for the Julia programming language")
                            .size(12.0)
                            .color(secondary_text(ui.visuals().dark_mode)),
                    );

                    // Tabs listed in reverse because the layout is right-to-left
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        for (t, label) in [
                            (Tab::About, "About"),
                            (Tab::Config, "Configuration"),
                            (Tab::Available, "Available"),
                            (Tab::Installed, "Installed"),
                        ] {
                            ui.selectable_value(&mut self.tab, t, label);
                            ui.add_space(1.0);
                        }
                    });
                });
                ui.add_space(3.0);
            });

        // ── activity panel ────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("status")
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);

                    // Left: spinner + current state
                    if self.loading || self.busy {
                        ui.add(egui::Spinner::new().size(11.0));
                        ui.add_space(3.0);
                        let label = self.current_op.as_deref().unwrap_or("Working…");
                        ui.label(RichText::new(label).size(12.0));
                    } else if let Some((msg, is_err)) = &self.status {
                        let col = if *is_err {
                            error_color(ui.visuals().dark_mode)
                        } else {
                            success_color(ui.visuals().dark_mode)
                        };
                        let icon = if *is_err { "x" } else { "ok" };
                        ui.colored_label(col, RichText::new(format!("{icon} {msg}")).size(12.0));
                    } else {
                        ui.label(
                            RichText::new("Ready")
                                .size(12.0)
                                .color(secondary_text(ui.visuals().dark_mode)),
                        );
                    }

                    // Right: toggle log button
                    if !self.log.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            let toggle_label = if self.log_open {
                                RichText::new("Hide log").size(12.0)
                            } else {
                                RichText::new(format!("Show log ({})", self.log.len())).size(12.0)
                            };
                            if ui.button(toggle_label).clicked() {
                                self.log_open = !self.log_open;
                            }
                        });
                    }
                });

                if self.log_open && !self.log.is_empty() {
                    ui.add_space(2.0);
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("activity_log")
                        .max_height(120.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.add_space(2.0);
                            for entry in &self.log {
                                let (col, prefix) = match entry.kind {
                                    LogKind::Output => (secondary_text(ui.visuals().dark_mode), ""),
                                    LogKind::Ok => (success_color(ui.visuals().dark_mode), "ok "),
                                    LogKind::Err => (error_color(ui.visuals().dark_mode), "x  "),
                                };
                                ui.label(
                                    RichText::new(format!("{prefix}{}", entry.text))
                                        .size(12.0)
                                        .color(col)
                                        .family(egui::FontFamily::Monospace),
                                );
                            }
                            ui.add_space(2.0);
                        });
                }

                ui.add_space(2.0);
            });

        // ── main content ──────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);
            match self.tab {
                Tab::Installed => tab_installed(self, ui),
                Tab::Available => tab_available(self, ui),
                Tab::Config => tab_config(self, ui),
                Tab::About => tab_about(self, ui),
            }
        });

        // ── splash overlay (renders on top, non-blocking) ─────────
        if splash_active {
            paint_splash(ctx, splash_t, logo_screen_rect);
        }

        // ── custom launch popup ───────────────────────────────────────
        if self.custom_launch_channel.is_some() {
            let mut open = true;
            let ch = self.custom_launch_channel.clone().unwrap();
            egui::Window::new(format!("Launch julia +{ch}"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Project directory:");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.custom_launch_project)
                                .hint_text(
                                    RichText::new("/path/to/project")
                                        .color(secondary_text(ui.visuals().dark_mode)),
                                )
                                .desired_width(260.0),
                        );
                        if ui.button("Browse...").clicked() {
                            let mut dialog = rfd::FileDialog::new();
                            if !self.custom_launch_project.trim().is_empty() {
                                dialog = dialog.set_directory(self.custom_launch_project.trim());
                            }
                            if let Some(path) = dialog.pick_folder() {
                                self.custom_launch_project = path.display().to_string();
                            }
                        }
                    });
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Sets --project and cds into it. Optional.")
                            .color(secondary_text(ui.visuals().dark_mode))
                            .size(12.0),
                    );

                    ui.add_space(6.0);
                    ui.label("Environment variables:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.custom_launch_env)
                            .hint_text(
                                RichText::new("JULIA_DEBUG=Foo BAR=1 ...")
                                    .color(secondary_text(ui.visuals().dark_mode)),
                            )
                            .desired_width(ui.available_width().min(380.0))
                            .font(egui::TextStyle::Monospace),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Space-separated KEY=VALUE pairs prepended to the command.")
                            .color(secondary_text(ui.visuals().dark_mode))
                            .size(12.0),
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("julia +{ch}"))
                                .monospace()
                                .color(secondary_text(ui.visuals().dark_mode)),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.custom_launch_args)
                                .hint_text(
                                    RichText::new("--threads=4 ...")
                                        .color(secondary_text(ui.visuals().dark_mode)),
                                )
                                .desired_width(200.0)
                                .font(egui::TextStyle::Monospace),
                        );
                    });
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                RichText::new("Launch")
                                    .color(success_color(ui.visuals().dark_mode)),
                            )
                            .clicked()
                        {
                            if let Err(e) = launch_julia(
                                &ch,
                                &self.terminal_app,
                                &self.custom_launch_project,
                                &self.custom_launch_args,
                                &self.custom_launch_env,
                            ) {
                                self.status = Some((e, true));
                            }
                            self.custom_launch_channel = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.custom_launch_channel = None;
                        }
                    });
                });
            if !open {
                self.custom_launch_channel = None;
            }
        }

        self.show_pr_confirmation(ctx);
    }
}

fn accessible_button_name(response: egui::Response, label: impl Into<String>) -> egui::Response {
    let enabled = response.enabled();
    let label = label.into();
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label.clone())
    });
    response
}

// ── installed tab ─────────────────────────────────────────────────────────────

fn tab_installed(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!app.busy, egui::Button::new("Refresh"))
            .clicked()
        {
            app.send(Op::Reload);
        }
        if ui
            .add_enabled(!app.busy, egui::Button::new("Update All"))
            .on_hover_text("Update every installed channel to its latest version")
            .clicked()
        {
            app.request_update(None);
        }
        if ui
            .add_enabled(!app.busy, egui::Button::new("Garbage Collect"))
            .on_hover_text("Garbage-collect unused Julia versions from disk")
            .clicked()
        {
            app.send(Op::Gc);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let prev = app.installed_view;
            ui.selectable_value(&mut app.installed_view, InstalledView::List, "List");
            let tiles_resp =
                ui.selectable_value(&mut app.installed_view, InstalledView::Tile, "Tiles");
            if app.installed_view != prev {
                save_view_pref(&app.paths, app.installed_view);
                app.list_tip_dismissed = true;
                save_bool_pref(&app.paths, "juliaupgui_list_tip_dismissed", true);
            }

            // Show-once tip bubble to the left of the view selector
            if !app.list_tip_dismissed
                && app.installed_view == InstalledView::Tile
                && app.state.as_ref().is_some_and(|s| s.installed.len() > 5)
            {
                let anchor = tiles_resp.rect.left_center() - egui::Vec2::new(6.0, 0.0);
                egui::Area::new(egui::Id::new("list_tip_bubble"))
                    .fixed_pos(anchor)
                    .constrain(true)
                    .pivot(egui::Align2::RIGHT_CENTER)
                    .order(egui::Order::Foreground)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.label(
                                    RichText::new("Tip: Try List view for many channels")
                                        .size(12.0),
                                );
                                if ui.button("Dismiss").clicked() {
                                    app.list_tip_dismissed = true;
                                    save_bool_pref(
                                        &app.paths,
                                        "juliaupgui_list_tip_dismissed",
                                        true,
                                    );
                                }
                            });
                        });
                    });
            }
        });
    });

    ui.add_space(4.0);

    let state = match app.state.as_ref() {
        Some(s) => s.clone(),
        None => return,
    };

    if state.installed.is_empty() {
        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("No Julia channels installed.")
                    .color(secondary_text(ui.visuals().dark_mode)),
            );
            ui.label("Go to the \"Available\" tab to install one.");
        });
        return;
    }

    match app.installed_view {
        InstalledView::Tile => tab_installed_tiles(app, ui, &state),
        InstalledView::List => tab_installed_list(app, ui, &state),
    }
}

fn tab_installed_tiles(app: &mut App, ui: &mut egui::Ui, state: &AppState) {
    let mut set_def: Option<String> = None;
    let mut do_update: Option<String> = None;
    let mut do_remove: Option<String> = None;
    let mut do_launch: Option<String> = None;
    let mut go_available = false;

    const TILE_W: f32 = 168.0;
    const TILE_H: f32 = 168.0;
    const MARGIN: f32 = 14.0;

    egui::ScrollArea::vertical().show(ui, |ui| {
        let tile_total = TILE_W + MARGIN * 2.0;
        let gap = 12.0;
        let avail = ui.available_width();
        let cols = ((avail + gap) / (tile_total + gap)).floor().max(1.0) as usize;

        // Build items: installed tiles + one "add" sentinel
        let n = state.installed.len() + 1; // +1 for the "add" tile
        let rows = n.div_ceil(cols);

        for row_idx in 0..rows {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                let start = row_idx * cols;
                for i in start..(start + cols).min(n) {
                    if i < state.installed.len() {
                        let row = state.installed[i].clone();
                        let dark = ui.visuals().dark_mode;
                        let border_col = if row.is_default {
                            success_color(dark)
                        } else {
                            tile_border(dark)
                        };

                        // Allocate a fixed rect for hit-testing the whole tile
                        let tile_outer = egui::vec2(TILE_W + MARGIN * 2.0, TILE_H + MARGIN * 2.0);
                        let tile_id = ui.id().with(("tile", i));
                        let tile_rect = ui.allocate_space(tile_outer).1;
                        let tile_resp = ui.interact(tile_rect, tile_id, egui::Sense::hover());
                        let hovered = tile_resp.hovered();

                        // Subtle hover effects
                        let fill = tile_bg(dark, hovered);
                        let stroke_w = if hovered { 2.0_f32 } else { 1.5_f32 };
                        let rounding = egui::Rounding::same(8.0);

                        // Paint the frame manually at the allocated rect
                        ui.painter().rect(
                            tile_rect,
                            rounding,
                            fill,
                            egui::Stroke::new(stroke_w, border_col),
                        );

                        // Place child UI inside the tile rect
                        let inner_rect = tile_rect.shrink(MARGIN);
                        let mut child_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(inner_rect)
                                .layout(egui::Layout::top_down(egui::Align::LEFT)),
                        );
                        child_ui.set_min_size(egui::vec2(TILE_W, TILE_H));
                        child_ui.set_max_height(TILE_H);
                        {
                            let ui = &mut child_ui;
                            ui.set_width(TILE_W);

                            // ── header: name + version + badges ──
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(&row.name).size(18.0).strong())
                                        .truncate(),
                                );
                                if let Some(pr_number) = &row.pr_number {
                                    pr_link(
                                        ui,
                                        pr_number,
                                        app.pr_titles.get(pr_number).map(String::as_str),
                                    );
                                }
                            });

                            ui.add_space(1.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&row.version)
                                        .size(12.0)
                                        .color(secondary_text(ui.visuals().dark_mode)),
                                )
                                .truncate(),
                            );

                            if row.is_default || row.update.is_some() {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    if row.is_default {
                                        ui.label(
                                            RichText::new("default")
                                                .size(12.0)
                                                .color(success_color(ui.visuals().dark_mode)),
                                        );
                                    }
                                    if let Some(upd) = &row.update {
                                        ui.label(
                                            RichText::new(format!("Update: {upd}"))
                                                .size(12.0)
                                                .color(warning_color(ui.visuals().dark_mode)),
                                        );
                                    }
                                });
                            }

                            // ── push buttons to bottom ──
                            let used = ui.min_rect().height();
                            let btn_h = 24.0;
                            let btn_rows = if row.is_default { 1 } else { 2 };
                            let buttons_h = btn_h * btn_rows as f32 + 4.0 * (btn_rows - 1) as f32;
                            let remaining = (TILE_H - used - buttons_h).max(0.0);
                            ui.add_space(remaining);

                            // ── Launch row ──
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let launch_w = (TILE_W - 4.0) / 2.0;
                                if accessible_button_name(
                                    ui.add_sized(
                                        [launch_w, btn_h],
                                        egui::Button::new(
                                            RichText::new("Launch")
                                                .size(12.0)
                                                .color(success_color(ui.visuals().dark_mode)),
                                        ),
                                    ),
                                    format!("Launch Julia channel {}", row.name),
                                )
                                .on_hover_text(format!("Start julia +{}", row.name))
                                .clicked()
                                {
                                    do_launch = Some(row.name.clone());
                                }
                                if accessible_button_name(
                                    ui.add_sized(
                                        [launch_w, btn_h],
                                        egui::Button::new(RichText::new("Custom...").size(12.0)),
                                    ),
                                    format!(
                                        "Launch Julia channel {} with custom options",
                                        row.name
                                    ),
                                )
                                .on_hover_text("Launch with custom project & args")
                                .clicked()
                                {
                                    app.custom_launch_channel = Some(row.name.clone());
                                }
                            });

                            // ── secondary actions row ──
                            if !row.is_default {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    let has_update = row.update.is_some();
                                    let action_w = if has_update {
                                        (TILE_W - 8.0) / 3.0
                                    } else {
                                        (TILE_W - 4.0) / 2.0
                                    };
                                    if accessible_button_name(
                                        ui.add_sized(
                                            [action_w, btn_h],
                                            egui::Button::new(RichText::new("Default").size(12.0)),
                                        ),
                                        format!("Set Julia channel {} as default", row.name),
                                    )
                                    .on_hover_text("Use this channel when no version is specified")
                                    .clicked()
                                    {
                                        set_def = Some(row.name.clone());
                                    }
                                    if has_update
                                        && accessible_button_name(
                                            ui.add_sized(
                                                [action_w, btn_h],
                                                egui::Button::new(
                                                    RichText::new("Update").size(12.0),
                                                ),
                                            ),
                                            format!("Update Julia channel {}", row.name),
                                        )
                                        .on_hover_text("Update to latest")
                                        .clicked()
                                    {
                                        do_update = Some(row.name.clone());
                                    }
                                    if accessible_button_name(
                                        ui.add_sized(
                                            [action_w, btn_h],
                                            egui::Button::new(RichText::new("Remove").size(12.0)),
                                        ),
                                        format!("Remove Julia channel {}", row.name),
                                    )
                                    .on_hover_text("Remove this channel")
                                    .clicked()
                                    {
                                        do_remove = Some(row.name.clone());
                                    }
                                });
                            }
                        }
                    } else {
                        // "Add another channel" tile
                        let add_outer = egui::vec2(TILE_W + MARGIN * 2.0, TILE_H + MARGIN * 2.0);
                        let add_id = ui.id().with("add_tile");
                        let add_rect = ui.allocate_space(add_outer).1;
                        let add_resp = ui.interact(add_rect, add_id, egui::Sense::click());
                        add_resp.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                true,
                                "Add Julia channel",
                            )
                        });
                        let add_hov = add_resp.hovered();
                        let add_focused = add_resp.has_focus();
                        if add_hov {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        let add_dark = ui.visuals().dark_mode;
                        let add_fill = tile_bg(add_dark, add_hov || add_focused);
                        let add_stroke_w = if add_hov || add_focused {
                            2.0_f32
                        } else {
                            1.5_f32
                        };
                        let add_border = if add_focused {
                            focus_color(add_dark)
                        } else {
                            tile_border(add_dark)
                        };
                        ui.painter().rect(
                            add_rect,
                            egui::Rounding::same(8.0),
                            add_fill,
                            egui::Stroke::new(add_stroke_w, add_border),
                        );
                        // Center the label in the tile
                        ui.painter().text(
                            add_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "+ Add channel",
                            egui::FontId::proportional(14.0),
                            secondary_text(add_dark),
                        );
                        if add_resp.clicked() {
                            go_available = true;
                        }
                    }
                }
            });
            ui.add_space(gap);
        }
    });

    if go_available {
        app.tab = Tab::Available;
    }
    if let Some(ch) = do_launch {
        if let Err(e) = launch_julia(&ch, &app.terminal_app, "", "", "") {
            app.status = Some((e, true));
        }
    }
    if let Some(ch) = set_def {
        app.send(Op::SetDefault(ch));
    }
    if let Some(ch) = do_update {
        app.request_update(Some(ch));
    }
    if let Some(ch) = do_remove {
        app.send(Op::Remove(ch));
    }
}

fn tab_installed_list(app: &mut App, ui: &mut egui::Ui, state: &AppState) {
    let mut set_def: Option<String> = None;
    let mut do_update: Option<String> = None;
    let mut do_remove: Option<String> = None;
    let mut do_launch: Option<String> = None;

    let body_height = ui.available_height();

    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(55.0)) // Default
        .column(Column::auto_with_initial_suggestion(140.0).at_least(90.0)) // Channel
        .column(Column::remainder().at_least(90.0)) // Version (takes leftover)
        .column(Column::initial(100.0).at_least(70.0)) // Update
        .column(Column::exact(150.0)) // Launch
        .column(Column::exact(200.0)) // Actions
        .min_scrolled_height(0.0)
        .max_scroll_height(body_height)
        .header(24.0, |mut header| {
            header.col(|ui| {
                ui.strong("Default");
            });
            header.col(|ui| {
                ui.strong("Channel");
            });
            header.col(|ui| {
                ui.strong("Version");
            });
            header.col(|ui| {
                ui.strong("Update");
            });
            header.col(|ui| {
                ui.strong("Launch");
            });
            header.col(|ui| {
                ui.strong("Actions");
            });
        })
        .body(|mut body| {
            for row in &state.installed {
                let row = row.clone();
                body.row(30.0, |mut cells| {
                    cells.col(|ui| {
                        if row.is_default {
                            ui.label(
                                RichText::new("Yes")
                                    .color(success_color(ui.visuals().dark_mode))
                                    .strong(),
                            );
                        }
                    });
                    cells.col(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&row.name).size(13.0));
                            if let Some(pr_number) = &row.pr_number {
                                pr_link(
                                    ui,
                                    pr_number,
                                    app.pr_titles.get(pr_number).map(String::as_str),
                                );
                            }
                        });
                    });
                    cells.col(|ui| {
                        ui.label(
                            RichText::new(&row.version)
                                .size(13.0)
                                .color(secondary_text(ui.visuals().dark_mode)),
                        );
                    });
                    cells.col(|ui| {
                        if let Some(upd) = &row.update {
                            ui.label(
                                RichText::new(upd)
                                    .size(12.0)
                                    .color(warning_color(ui.visuals().dark_mode)),
                            );
                        } else {
                            ui.label(
                                RichText::new("Current")
                                    .size(12.0)
                                    .color(secondary_text(ui.visuals().dark_mode)),
                            );
                        }
                    });
                    cells.col(|ui| {
                        ui.horizontal(|ui| {
                            if accessible_button_name(
                                ui.add(egui::Button::new(
                                    RichText::new("Launch")
                                        .size(12.0)
                                        .color(success_color(ui.visuals().dark_mode)),
                                )),
                                format!("Launch Julia channel {}", row.name),
                            )
                            .on_hover_text(format!("Start julia +{}", row.name))
                            .clicked()
                            {
                                do_launch = Some(row.name.clone());
                            }
                            if accessible_button_name(
                                ui.add(egui::Button::new(RichText::new("Custom...").size(12.0))),
                                format!("Launch Julia channel {} with custom options", row.name),
                            )
                            .on_hover_text("Launch with custom project & args")
                            .clicked()
                            {
                                app.custom_launch_channel = Some(row.name.clone());
                            }
                        });
                    });
                    cells.col(|ui| {
                        ui.horizontal(|ui| {
                            if !row.is_default
                                && accessible_button_name(
                                    ui.add_enabled(
                                        !app.busy,
                                        egui::Button::new(RichText::new("Default").size(12.0)),
                                    ),
                                    format!("Set Julia channel {} as default", row.name),
                                )
                                .on_hover_text("Set as default channel")
                                .clicked()
                            {
                                set_def = Some(row.name.clone());
                            }
                            if row.update.is_some()
                                && accessible_button_name(
                                    ui.add_enabled(
                                        !app.busy,
                                        egui::Button::new(RichText::new("Update").size(12.0)),
                                    ),
                                    format!("Update Julia channel {}", row.name),
                                )
                                .on_hover_text("Update this channel")
                                .clicked()
                            {
                                do_update = Some(row.name.clone());
                            }
                            if !row.is_default
                                && accessible_button_name(
                                    ui.add_enabled(
                                        !app.busy,
                                        egui::Button::new(RichText::new("Remove").size(12.0)),
                                    ),
                                    format!("Remove Julia channel {}", row.name),
                                )
                                .on_hover_text("Remove this channel")
                                .clicked()
                            {
                                do_remove = Some(row.name.clone());
                            }
                        });
                    });
                });
            }
        });

    if let Some(ch) = do_launch {
        if let Err(e) = launch_julia(&ch, &app.terminal_app, "", "", "") {
            app.status = Some((e, true));
        }
    }
    if let Some(ch) = set_def {
        app.send(Op::SetDefault(ch));
    }
    if let Some(ch) = do_update {
        app.request_update(Some(ch));
    }
    if let Some(ch) = do_remove {
        app.send(Op::Remove(ch));
    }
}

// ── available tab ─────────────────────────────────────────────────────────────

fn tab_available(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.add(
            egui::TextEdit::singleline(&mut app.filter)
                .hint_text(
                    RichText::new("channel name…").color(secondary_text(ui.visuals().dark_mode)),
                )
                .desired_width(160.0),
        );
        ui.add_space(8.0);
        if ui
            .add_enabled(!app.busy, egui::Button::new("Refresh DB"))
            .on_hover_text("Download the latest Julia versions database from the server")
            .clicked()
        {
            app.send(Op::UpdateVersionDb);
        }
    });

    ui.add_space(6.0);

    let state = match app.state.as_ref() {
        Some(s) => s.clone(),
        None => return,
    };

    let filter = app.filter.to_lowercase();

    let mut rows: Vec<AvailableRow> = state
        .available
        .iter()
        .filter(|r| filter.is_empty() || r.channel.to_lowercase().contains(&filter))
        .cloned()
        .collect();
    rows.sort_by(|a, b| cmp(&a.channel, &b.channel));

    let mut to_install: Option<String> = None;

    ui.collapsing(
        "Link an existing Julia binary or channel to a custom channel name",
        |ui| {
            egui::Grid::new("link_grid")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Channel name:").size(12.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.link_channel)
                            .hint_text(
                                RichText::new("e.g. myjulia")
                                    .color(secondary_text(ui.visuals().dark_mode)),
                            )
                            .desired_width(140.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new("Path or +channel:").size(12.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.link_target)
                            .hint_text(
                                RichText::new("/path/to/julia or +1.10")
                                    .color(secondary_text(ui.visuals().dark_mode)),
                            )
                            .desired_width(200.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new("Extra args:").size(12.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.link_args)
                            .hint_text(
                                RichText::new("optional")
                                    .color(secondary_text(ui.visuals().dark_mode)),
                            )
                            .desired_width(200.0),
                    );
                    ui.end_row();
                });
            let can_link = !app.busy
                && !app.link_channel.trim().is_empty()
                && !app.link_target.trim().is_empty();
            if ui
                .add_enabled(can_link, egui::Button::new("Link"))
                .clicked()
            {
                let args: Vec<String> =
                    app.link_args.split_whitespace().map(String::from).collect();
                app.send(Op::Link {
                    channel: app.link_channel.trim().to_string(),
                    target: app.link_target.trim().to_string(),
                    args,
                });
                app.link_channel.clear();
                app.link_target.clear();
                app.link_args.clear();
            }
        },
    );

    ui.separator();

    // ── build parent-child tree ───────────────────────────────────────
    let channel_set: HashSet<&str> = rows.iter().map(|r| r.channel.as_str()).collect();

    // For each channel, find its direct parent (longest prefix match separated by . - ~)
    let mut parent_of: HashMap<&str, &str> = HashMap::new();
    let mut children_of: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        let ch = row.channel.as_str();
        let mut best: Option<&str> = None;
        let mut best_len = 0;
        for &other in &channel_set {
            if other.len() < ch.len()
                && ch.starts_with(other)
                && matches!(
                    ch.as_bytes().get(other.len()),
                    Some(b'.') | Some(b'-') | Some(b'~')
                )
                && other.len() > best_len
            {
                best = Some(other);
                best_len = other.len();
            }
        }
        if let Some(p) = best {
            parent_of.insert(ch, p);
            children_of.entry(p).or_default().push(i);
        }
    }

    // DFS flatten into (index, depth) preserving sorted order
    struct FlatEntry {
        index: usize,
        depth: usize,
        has_children: bool,
    }

    fn dfs_flatten(
        idx: usize,
        depth: usize,
        rows: &[AvailableRow],
        children_of: &HashMap<&str, Vec<usize>>,
        out: &mut Vec<FlatEntry>,
    ) {
        let ch = rows[idx].channel.as_str();
        let kids = children_of.get(ch);
        out.push(FlatEntry {
            index: idx,
            depth,
            has_children: kids.is_some_and(|v| !v.is_empty()),
        });
        if let Some(kids) = kids {
            for &kid in kids {
                dfs_flatten(kid, depth + 1, rows, children_of, out);
            }
        }
    }

    let mut flat: Vec<FlatEntry> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if !parent_of.contains_key(row.channel.as_str()) {
            dfs_flatten(i, 0, &rows, &children_of, &mut flat);
        }
    }

    // Determine visibility: all ancestors must be expanded (or filter active = show flat)
    let filtering = !filter.is_empty();
    let visible: Vec<&FlatEntry> = flat
        .iter()
        .filter(|e| {
            if filtering {
                return true;
            }
            let mut cur = rows[e.index].channel.as_str();
            while let Some(&p) = parent_of.get(cur) {
                if !app.avail_expanded.contains(p) {
                    return false;
                }
                cur = p;
            }
            true
        })
        .collect();

    // ── render table ──────────────────────────────────────────────────
    let table_height = ui.available_height();
    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(220.0).at_least(140.0).clip(true)) // Channel
        .column(Column::remainder().at_least(120.0)) // Version
        .column(Column::initial(110.0).at_least(80.0)) // Status
        .column(Column::exact(190.0)) // Action
        .min_scrolled_height(0.0)
        .max_scroll_height(table_height)
        .header(24.0, |mut header| {
            header.col(|ui| {
                ui.strong("Channel");
            });
            header.col(|ui| {
                ui.strong("Version");
            });
            header.col(|ui| {
                ui.strong("Status");
            });
            header.col(|ui| {
                ui.strong("Action");
            });
        })
        .body(|mut body| {
            for entry in &visible {
                let row = rows[entry.index].clone();
                let depth = if filtering { 0 } else { entry.depth };
                let has_children = entry.has_children;

                body.row(30.0, |mut cells| {
                    cells.col(|ui| {
                        ui.horizontal(|ui| {
                            if depth > 0 {
                                ui.add_space(depth as f32 * 14.0);
                            }
                            if has_children && !filtering {
                                let expanded = app.avail_expanded.contains(row.channel.as_str());
                                let icon = if expanded { "−" } else { "+" };
                                let action = if expanded { "Collapse" } else { "Expand" };
                                let response = ui.add_sized(
                                    [24.0, 24.0],
                                    egui::Button::new(RichText::new(icon).size(13.0)),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        true,
                                        format!("{action} versions for {}", row.channel),
                                    )
                                });
                                if response
                                    .on_hover_text(format!("{action} versions for {}", row.channel))
                                    .clicked()
                                {
                                    if expanded {
                                        app.avail_expanded.remove(row.channel.as_str());
                                    } else {
                                        app.avail_expanded.insert(row.channel.clone());
                                    }
                                }
                            } else if !filtering && depth > 0 {
                                ui.add_space(24.0);
                            }
                            ui.add(egui::Label::new(&row.channel).truncate());
                        });
                    });
                    cells.col(|ui| {
                        ui.label(
                            RichText::new(&row.version)
                                .color(secondary_text(ui.visuals().dark_mode)),
                        );
                    });
                    cells.col(|ui| {
                        if row.installed {
                            ui.label(
                                RichText::new("Installed")
                                    .color(success_color(ui.visuals().dark_mode)),
                            );
                        } else {
                            ui.label(
                                RichText::new("Not installed")
                                    .color(secondary_text(ui.visuals().dark_mode)),
                            );
                        }
                    });
                    cells.col(|ui| {
                        if !row.installed {
                            let is_template =
                                row.channel.contains("{number}") || row.channel.starts_with("x.y");
                            if is_template {
                                if app.channel_prompt.as_deref() == Some(row.channel.as_str()) {
                                    ui.horizontal(|ui| {
                                        let (hint, placeholder) =
                                            if row.channel.contains("{number}") {
                                                ("PR #", "{number}")
                                            } else {
                                                ("e.g. 1.12", "x.y")
                                            };
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut app.channel_prompt_input,
                                            )
                                            .hint_text(
                                                RichText::new(hint)
                                                    .color(secondary_text(ui.visuals().dark_mode)),
                                            )
                                            .desired_width(60.0),
                                        );
                                        let input = app.channel_prompt_input.trim();
                                        let valid = !app.busy
                                            && !input.is_empty()
                                            && if placeholder == "{number}" {
                                                input.chars().all(|c| c.is_ascii_digit())
                                            } else {
                                                // x.y format: digits.digits
                                                input.split_once('.').is_some_and(|(a, b)| {
                                                    !a.is_empty()
                                                        && !b.is_empty()
                                                        && a.chars().all(|c| c.is_ascii_digit())
                                                        && b.chars().all(|c| c.is_ascii_digit())
                                                })
                                            };
                                        if accessible_button_name(
                                            ui.add_enabled(valid, egui::Button::new("Go")),
                                            format!(
                                                "Install Julia channel {} with the entered value",
                                                row.channel
                                            ),
                                        )
                                        .clicked()
                                        {
                                            let ch = row.channel.replace(placeholder, input);
                                            to_install = Some(ch);
                                            app.channel_prompt = None;
                                            app.channel_prompt_input.clear();
                                        }
                                        if accessible_button_name(
                                            ui.button("Cancel"),
                                            format!(
                                                "Cancel installing Julia channel {}",
                                                row.channel
                                            ),
                                        )
                                        .clicked()
                                        {
                                            app.channel_prompt = None;
                                            app.channel_prompt_input.clear();
                                        }
                                    });
                                } else if accessible_button_name(
                                    ui.add_enabled(!app.busy, egui::Button::new("Install…")),
                                    format!("Choose a value for Julia channel {}", row.channel),
                                )
                                .clicked()
                                {
                                    app.channel_prompt = Some(row.channel.clone());
                                    app.channel_prompt_input.clear();
                                }
                            } else if accessible_button_name(
                                ui.add_enabled(!app.busy, egui::Button::new("Install")),
                                format!("Install Julia channel {}", row.channel),
                            )
                            .clicked()
                            {
                                to_install = Some(row.channel.clone());
                            }
                        }
                    });
                });
            }
        });

    if let Some(ch) = to_install {
        app.request_install(ch);
    }
}

// ── config tab ────────────────────────────────────────────────────────────────

fn tab_config(app: &mut App, ui: &mut egui::Ui) {
    let settings = match app.state.as_ref().map(|s| s.settings.clone()) {
        Some(s) => s,
        None => {
            ui.spinner();
            return;
        }
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(4.0);
        ui.heading("Settings");
        ui.add_space(8.0);

        egui::Grid::new("cfg_grid")
            .num_columns(2)
            .spacing([24.0, 10.0])
            .min_col_width(200.0)
            .show(ui, |ui| {
                // ── Versions DB update interval ──────────────────────────
                ui.label("Version database refresh (minutes; 0 disables):")
                    .on_hover_text(
                        "How often juliaup refreshes the Julia versions database. 0 = disabled.",
                    );
                ui.horizontal(|ui| {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut app.interval_input).desired_width(80.0),
                    );
                    if resp.lost_focus() {
                        if let Ok(v) = app.interval_input.parse::<i64>() {
                            if v != settings.versionsdb_update_interval {
                                app.send(Op::SetVersionsDbInterval(v));
                            }
                        } else {
                            // Reset to current value on invalid input
                            app.interval_input = settings.versionsdb_update_interval.to_string();
                        }
                    }
                });
                ui.end_row();

                // ── Auto-install channels ────────────────────────────────
                ui.label("Auto-install missing channels:").on_hover_text(
                    "Whether `julia +channel` automatically installs missing channels.",
                );
                {
                    let mut idx: usize = match settings.auto_install_channels {
                        None => 0,
                        Some(true) => 1,
                        Some(false) => 2,
                    };
                    let prev = idx;
                    egui::ComboBox::from_id_salt("auto_install_cb")
                        .selected_text(["Default (prompt)", "Always", "Never"][idx])
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut idx, 0, "Default (prompt)");
                            ui.selectable_value(&mut idx, 1, "Always");
                            ui.selectable_value(&mut idx, 2, "Never");
                        });
                    if idx != prev && !app.busy {
                        let v = match idx {
                            0 => None,
                            1 => Some(true),
                            _ => Some(false),
                        };
                        app.send(Op::SetAutoInstall(v));
                    }
                }
                ui.end_row();

                // ── Manifest version detect ──────────────────────────────
                ui.label("Detect version from project manifest:").on_hover_text(
                    "Pick the Julia version from Project.toml/Manifest.toml when present.",
                );
                {
                    let mut v = settings.manifest_version_detect;
                    if ui
                        .add_enabled(!app.busy, egui::Checkbox::new(&mut v, "Enabled"))
                        .changed()
                    {
                        app.send(Op::SetManifestDetect(v));
                    }
                }
                ui.end_row();

                // ── Channel symlinks (non-Windows) ───────────────────────
                #[cfg(not(windows))]
                {
                    ui.label("Create channel symlinks:")
                        .on_hover_text("Create a separate symlink per installed channel.");
                    let mut v = settings.create_channel_symlinks;
                    if ui
                        .add_enabled(!app.busy, egui::Checkbox::new(&mut v, "Enabled"))
                        .changed()
                    {
                        app.send(Op::SetChannelSymlinks(v));
                    }
                    ui.end_row();
                }

                // ── Terminal application ──────────────────────────────
                ui.label("Terminal application:").on_hover_text(
                    "Terminal emulator to use for Launch. Leave blank for platform default.",
                );
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut app.terminal_app)
                                .hint_text(
                                    RichText::new(default_terminal_hint())
                                        .color(secondary_text(ui.visuals().dark_mode)),
                                )
                                .desired_width(200.0),
                        );
                        if resp.lost_focus() {
                            save_terminal_pref(&app.paths, &app.terminal_app);
                        }
                        if ui.button("Browse...").clicked() {
                            #[allow(unused_mut)]
                            let mut dialog = rfd::FileDialog::new();
                            #[cfg(target_os = "macos")]
                            {
                                dialog = dialog.set_directory("/Applications");
                            }
                            #[cfg(target_os = "linux")]
                            {
                                dialog = dialog.set_directory("/usr/bin");
                            }
                            if let Some(path) = dialog.pick_file() {
                                app.terminal_app = path.display().to_string();
                                save_terminal_pref(&app.paths, &app.terminal_app);
                            }
                        }
                    });
                    ui.label(
                        RichText::new(
                            "App name or path (e.g. iTerm, Terminal, /usr/bin/xterm). Blank = platform default.",
                        )
                        .color(secondary_text(ui.visuals().dark_mode))
                        .size(12.0),
                    );
                });
                ui.end_row();

                // ── Theme ────────────────────────────────────────────────
                ui.label("Appearance:").on_hover_text("Color theme for the GUI.");
                {
                    let prev = app.theme_mode;
                    egui::ComboBox::from_id_salt("theme_cb")
                        .selected_text(match app.theme_mode {
                            ThemeMode::System => "System",
                            ThemeMode::Dark => "Dark",
                            ThemeMode::Light => "Light",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.theme_mode, ThemeMode::System, "System");
                            ui.selectable_value(&mut app.theme_mode, ThemeMode::Dark, "Dark");
                            ui.selectable_value(&mut app.theme_mode, ThemeMode::Light, "Light");
                        });
                    if app.theme_mode != prev {
                        save_theme_pref(&app.paths, app.theme_mode);
                        apply_theme(ui.ctx(), app.theme_mode);
                    }
                }
                ui.end_row();

                // ── Reset UI ──────────────────────────────────────────
                ui.label("");
                if ui.button("Reset UI").clicked() {
                    app.list_tip_dismissed = false;
                    save_bool_pref(&app.paths, "juliaupgui_list_tip_dismissed", false);
                    app.installed_view = InstalledView::Tile;
                    save_view_pref(&app.paths, app.installed_view);
                    app.theme_mode = ThemeMode::System;
                    save_theme_pref(&app.paths, app.theme_mode);
                    apply_theme(ui.ctx(), app.theme_mode);
                }
                ui.end_row();
            });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        ui.heading("Maintenance");
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!app.busy, egui::Button::new("Refresh Version Database"))
                .on_hover_text("Download the latest Julia channel/version data from the server")
                .clicked()
            {
                app.send(Op::UpdateVersionDb);
            }
            if ui
                .add_enabled(!app.busy, egui::Button::new("Garbage Collect"))
                .on_hover_text("Remove Julia versions no longer referenced by any channel")
                .clicked()
            {
                app.send(Op::Gc);
            }
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        ui.heading("Directory Overrides");
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Make juliaup use a specific Julia channel when invoked within a directory tree.",
            )
            .color(secondary_text(ui.visuals().dark_mode)),
        );
        ui.add_space(8.0);

        let overrides = app
            .state
            .as_ref()
            .map(|s| s.overrides.clone())
            .unwrap_or_default();
        if overrides.is_empty() {
            ui.label(
                RichText::new("No overrides configured.")
                    .color(secondary_text(ui.visuals().dark_mode)),
            );
        } else {
            egui::Grid::new("ov_grid")
                .num_columns(3)
                .striped(true)
                .spacing([12.0, 5.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Directory").strong());
                    ui.label(RichText::new("Channel").strong());
                    ui.label(RichText::new("Action").strong());
                    ui.end_row();

                    let mut to_unset: Option<String> = None;
                    for ov in &overrides {
                        ui.label(&ov.path);
                        ui.label(&ov.channel);
                        if accessible_button_name(
                            ui.add_enabled(!app.busy, egui::Button::new("Remove")),
                            format!("Remove override for {}", ov.path),
                        )
                            .clicked()
                        {
                            to_unset = Some(ov.path.clone());
                        }
                        ui.end_row();
                    }
                    if let Some(p) = to_unset {
                        app.send(Op::UnsetOverride(p));
                    }
                });
        }

        ui.add_space(6.0);
        if ui
            .add_enabled(!app.busy, egui::Button::new("Remove non-existent paths"))
            .on_hover_text("Remove all overrides whose directories no longer exist on disk")
            .clicked()
        {
            app.send(Op::UnsetNonexistentOverrides);
        }

        ui.add_space(10.0);
        ui.label(RichText::new("Add override").strong());
        ui.add_space(4.0);

        egui::Grid::new("add_ov_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Directory path:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.ov_path)
                        .hint_text(
                            RichText::new("/path/to/project")
                                .color(secondary_text(ui.visuals().dark_mode)),
                        )
                        .desired_width(300.0),
                );
                ui.end_row();
                ui.label("Julia channel:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.ov_channel)
                        .hint_text(
                            RichText::new("e.g. 1.10")
                                .color(secondary_text(ui.visuals().dark_mode)),
                        )
                        .desired_width(120.0),
                );
                ui.end_row();
            });
        ui.add_space(4.0);

        let can_add =
            !app.busy && !app.ov_path.trim().is_empty() && !app.ov_channel.trim().is_empty();
        if ui
            .add_enabled(can_add, egui::Button::new("Add override"))
            .clicked()
        {
            app.send(Op::SetOverride {
                path: app.ov_path.trim().to_string(),
                channel: app.ov_channel.trim().to_string(),
            });
            app.ov_path.clear();
            app.ov_channel.clear();
        }
    });
}

// ── about tab ─────────────────────────────────────────────────────────────────

fn tab_about(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(16.0);

        ui.vertical_centered(|ui| {
            julia_logo_large(ui, 80.0);
            ui.add_space(10.0);
            ui.label(RichText::new("Juliaup").size(28.0).strong());
            ui.add_space(2.0);
            ui.label(
                RichText::new("Installation manager for the Julia programming language")
                    .size(13.0)
                    .color(secondary_text(ui.visuals().dark_mode)),
            );
            if !app.juliaup_version.is_empty() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let version_text = format!("Version {}", app.juliaup_version);
                    let btn_text = "Check for updates";
                    let est_width = ui.fonts(|f| {
                        f.layout_no_wrap(
                            version_text.clone(),
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        )
                        .size()
                        .x
                    }) + 6.0
                        + ui.fonts(|f| {
                            f.layout_no_wrap(
                                btn_text.into(),
                                egui::FontId::proportional(12.0),
                                Color32::WHITE,
                            )
                            .size()
                            .x
                        })
                        + 14.0; // button padding
                    let pad = ((ui.available_width() - est_width) / 2.0).max(0.0);
                    ui.add_space(pad);
                    ui.label(
                        RichText::new(version_text)
                            .size(12.0)
                            .color(secondary_text(ui.visuals().dark_mode)),
                    );
                    ui.add_space(6.0);
                    if ui
                        .add_enabled(
                            !app.busy,
                            egui::Button::new(RichText::new(btn_text).size(12.0)),
                        )
                        .on_hover_text(
                            "Check for and install the latest juliaup release",
                        )
                        .clicked()
                    {
                        app.send(Op::SelfUpdate);
                    }
                });
            }
        });

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(12.0);

        ui.heading("Links");
        ui.add_space(6.0);

        egui::Grid::new("about_links")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Julia language:");
                ui.hyperlink_to("julialang.org", "https://julialang.org");
                ui.end_row();

                ui.label("Julia GitHub:");
                ui.hyperlink_to(
                    "github.com/JuliaLang/julia",
                    "https://github.com/JuliaLang/julia",
                );
                ui.end_row();

                ui.label("Juliaup GitHub:");
                ui.hyperlink_to(
                    "github.com/JuliaLang/juliaup",
                    "https://github.com/JuliaLang/juliaup",
                );
                ui.end_row();

                ui.label("Julia Discourse:");
                ui.hyperlink_to("discourse.julialang.org", "https://discourse.julialang.org");
                ui.end_row();
            });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        ui.heading("Feedback & Issues");
        ui.add_space(6.0);
        ui.label(
            "Found a bug or have a feature request? Please file an issue on the Juliaup repository:",
        );
        ui.add_space(4.0);
        ui.hyperlink_to(
            "github.com/JuliaLang/juliaup/issues",
            "https://github.com/JuliaLang/juliaup/issues",
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "When reporting issues, include your OS, juliaup version, and steps to reproduce.",
            )
            .color(secondary_text(ui.visuals().dark_mode))
            .size(12.0),
        );

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        ui.heading("License");
        ui.add_space(6.0);
        ui.label("Juliaup is open-source software released under the MIT license.");

        ui.add_space(24.0);
    });
}

fn julia_logo_large(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_julia_dots(&ui.painter_at(rect), rect);
    }
}

// ── worker thread ─────────────────────────────────────────────────────────────

#[derive(Debug)]
struct LaunchSpec {
    julia_bin: std::path::PathBuf,
    args: Vec<String>,
    env: Vec<(String, String)>,
    project: Option<std::path::PathBuf>,
}

/// Split user-entered arguments while preserving quoted whitespace and Windows
/// path separators. Quotes group text but are not included in the result.
fn split_user_input(input: &str) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote: Option<char> = None;
    let mut token_started = false;

    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                    token_started = true;
                } else if c == '\\' && q == '"' {
                    match chars.peek().copied() {
                        Some('"' | '\\') => {
                            current.push(chars.next().expect("peeked character disappeared"));
                        }
                        _ => current.push(c),
                    }
                } else {
                    current.push(c);
                }
            }
            None if c.is_whitespace() => {
                if token_started {
                    result.push(std::mem::take(&mut current));
                    token_started = false;
                }
            }
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                token_started = true;
            }
            None if c == '\\' => {
                match chars.peek().copied() {
                    Some(next) if next.is_whitespace() || matches!(next, '\'' | '"' | '\\') => {
                        current.push(chars.next().expect("peeked character disappeared"));
                    }
                    _ => current.push(c),
                }
                token_started = true;
            }
            None => {
                current.push(c);
                token_started = true;
            }
        }
    }

    if let Some(q) = quote {
        return Err(format!("Unterminated {q} quote in launch options"));
    }
    if token_started {
        result.push(current);
    }

    Ok(result)
}

fn valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn build_launch_spec(
    julia_bin: std::path::PathBuf,
    channel: &str,
    project: &str,
    extra_args: &str,
    env_vars: &str,
) -> Result<LaunchSpec, String> {
    let project = (!project.trim().is_empty()).then(|| std::path::PathBuf::from(project.trim()));

    let mut args = vec![format!("+{channel}")];
    if let Some(project) = &project {
        args.push(format!("--project={}", project.display()));
    }
    args.extend(split_user_input(extra_args)?);

    let mut env = Vec::new();
    for assignment in split_user_input(env_vars)? {
        let (key, value) = assignment.split_once('=').ok_or_else(|| {
            format!("Environment variable '{assignment}' must use the KEY=VALUE format")
        })?;
        if !valid_env_key(key) {
            return Err(format!("Invalid environment variable name '{key}'"));
        }
        env.push((key.to_string(), value.to_string()));
    }

    Ok(LaunchSpec {
        julia_bin,
        args,
        env,
        project,
    })
}

fn julia_binary() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not determine the Juliaup GUI path: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "Could not determine the Juliaup binary directory".to_string())?;

    for name in ["julia", "julialauncher"] {
        let candidate = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "Could not find Julia next to the GUI at '{}'",
        dir.display()
    ))
}

/// Shell-quote a string for POSIX shells (wraps in single quotes).
#[cfg(not(target_os = "windows"))]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build a shell command string for POSIX systems.
#[cfg(not(target_os = "windows"))]
fn build_launch_cmd(spec: &LaunchSpec) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(project) = &spec.project {
        parts.push(format!(
            "cd {} &&",
            shell_quote(project.to_string_lossy().as_ref())
        ));
    }
    for (key, value) in &spec.env {
        parts.push(format!("{key}={}", shell_quote(value)));
    }
    parts.push(shell_quote(spec.julia_bin.to_string_lossy().as_ref()));
    for arg in &spec.args {
        parts.push(shell_quote(arg));
    }
    parts.join(" ")
}

#[cfg(target_os = "windows")]
fn configure_windows_command(command: &mut std::process::Command, spec: &LaunchSpec) {
    command.args(&spec.args).envs(&spec.env);
    if let Some(project) = &spec.project {
        command.current_dir(project);
    }
}

#[cfg(target_os = "windows")]
fn launch_windows_console(spec: &LaunchSpec) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

    let mut command = std::process::Command::new(&spec.julia_bin);
    configure_windows_command(&mut command, spec);
    command.creation_flags(CREATE_NEW_CONSOLE);
    command.spawn().map(|_| ()).map_err(|e| {
        format!(
            "Failed to launch Julia at '{}': {e}",
            spec.julia_bin.display()
        )
    })
}

#[cfg(target_os = "windows")]
fn launch_windows_terminal(term: &str, spec: &LaunchSpec) -> Result<(), String> {
    let terminal_name = std::path::Path::new(term)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(term)
        .to_ascii_lowercase();

    if terminal_name == "cmd" {
        return launch_windows_console(spec);
    }

    let mut command = std::process::Command::new(term);
    if matches!(terminal_name.as_str(), "wt" | "windowsterminal") {
        if let Some(project) = &spec.project {
            command.arg("-d").arg(project);
        }
    } else if let Some(project) = &spec.project {
        command.current_dir(project);
    }
    command
        .arg(&spec.julia_bin)
        .args(&spec.args)
        .envs(&spec.env)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch {term}: {e}"))
}

/// Spawn `julia +channel` in a new terminal window (fire-and-forget).
/// If `terminal_app` is non-empty it is used as the terminal emulator;
/// otherwise platform-specific defaults are tried.
/// `project` sets `--project=<dir>` and cds into it.
/// `extra_args` are appended after the channel arg.
/// `env_vars` is a space-separated list of KEY=VALUE pairs.
fn launch_julia(
    channel: &str,
    terminal_app: &str,
    project: &str,
    extra_args: &str,
    env_vars: &str,
) -> Result<(), String> {
    let spec = build_launch_spec(julia_binary()?, channel, project, extra_args, env_vars)?;

    #[cfg(target_os = "windows")]
    {
        if !terminal_app.trim().is_empty() {
            launch_in_terminal(terminal_app.trim(), &spec)
        } else {
            launch_default_terminal(&spec)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let full_cmd = build_launch_cmd(&spec);
        if !terminal_app.trim().is_empty() {
            launch_in_terminal(terminal_app.trim(), &full_cmd)
        } else {
            launch_default_terminal(&full_cmd)
        }
    }
}

#[cfg(target_os = "macos")]
fn launch_in_terminal(term: &str, full_cmd: &str) -> Result<(), String> {
    let full_cmd_as = full_cmd.replace('\\', "\\\\").replace('"', "\\\"");
    let app_name = std::path::Path::new(term)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(term);
    let lower = app_name.to_lowercase();
    let script = if lower == "iterm" || lower == "iterm2" {
        format!(
            "tell application \"{app_name}\"\n  activate\n  \
             if (count of windows) = 0 then\n    \
             create window with default profile\n  \
             else\n    \
             tell current window\n      \
             create tab with default profile\n    \
             end tell\n  \
             end if\n  \
             tell current session of current window\n    \
             write text \"{full_cmd_as}\"\n  \
             end tell\n\
             end tell"
        )
    } else {
        format!(
            "tell application \"{app_name}\"\n  activate\n  \
             do script \"{full_cmd_as}\"\n\
             end tell"
        )
    };
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch via {app_name}: {e}"))
}

#[cfg(target_os = "windows")]
fn launch_in_terminal(term: &str, spec: &LaunchSpec) -> Result<(), String> {
    launch_windows_terminal(term, spec)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn launch_in_terminal(term: &str, full_cmd: &str) -> Result<(), String> {
    std::process::Command::new(term)
        .args(["-e", "sh", "-c", full_cmd])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch {term}: {e}"))
}

#[cfg(target_os = "macos")]
fn launch_default_terminal(full_cmd: &str) -> Result<(), String> {
    let full_cmd_as = full_cmd.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "tell application \"Terminal\"\n  activate\n  do script \"{full_cmd_as}\"\nend tell"
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch Terminal: {e}"))
}

#[cfg(target_os = "windows")]
fn launch_default_terminal(spec: &LaunchSpec) -> Result<(), String> {
    if launch_windows_terminal("wt", spec).is_ok() {
        return Ok(());
    }
    launch_windows_console(spec)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn launch_default_terminal(full_cmd: &str) -> Result<(), String> {
    let terminals = [
        ("x-terminal-emulator", vec!["-e"]),
        ("gnome-terminal", vec!["--"]),
        ("konsole", vec!["-e"]),
        ("xfce4-terminal", vec!["-e"]),
        ("xterm", vec!["-e"]),
    ];
    for (term, prefix_args) in &terminals {
        let mut cmd = std::process::Command::new(term);
        for a in prefix_args {
            cmd.arg(a);
        }
        cmd.arg("sh").arg("-c").arg(full_cmd);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
    }
    std::process::Command::new("sh")
        .args(["-c", full_cmd])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not find a terminal emulator: {e}"))
}

fn default_terminal_hint() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Terminal"
    }
    #[cfg(target_os = "windows")]
    {
        "cmd"
    }
    #[cfg(target_os = "linux")]
    {
        "x-terminal-emulator"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "xterm"
    }
}

fn gui_prefs_path(paths: &GlobalPaths) -> std::path::PathBuf {
    paths.juliauphome.join("juliaupgui_terminal")
}

fn load_terminal_pref(paths: &GlobalPaths) -> String {
    std::fs::read_to_string(gui_prefs_path(paths))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn save_terminal_pref(paths: &GlobalPaths, value: &str) {
    if let Err(e) = std::fs::write(gui_prefs_path(paths), value.trim()) {
        eprintln!("Warning: could not save terminal preference: {e}");
    }
}

fn load_view_pref(paths: &GlobalPaths) -> InstalledView {
    match std::fs::read_to_string(paths.juliauphome.join("juliaupgui_viewmode"))
        .unwrap_or_default()
        .trim()
    {
        "list" => InstalledView::List,
        _ => InstalledView::Tile,
    }
}

fn save_view_pref(paths: &GlobalPaths, view: InstalledView) {
    let v = match view {
        InstalledView::Tile => "tile",
        InstalledView::List => "list",
    };
    if let Err(e) = std::fs::write(paths.juliauphome.join("juliaupgui_viewmode"), v) {
        eprintln!("Warning: could not save view preference: {e}");
    }
}

fn load_theme_pref(paths: &GlobalPaths) -> ThemeMode {
    match std::fs::read_to_string(paths.juliauphome.join("juliaupgui_theme"))
        .unwrap_or_default()
        .trim()
    {
        "dark" => ThemeMode::Dark,
        "light" => ThemeMode::Light,
        _ => ThemeMode::System,
    }
}

fn save_theme_pref(paths: &GlobalPaths, mode: ThemeMode) {
    let v = match mode {
        ThemeMode::System => "system",
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
    };
    if let Err(e) = std::fs::write(paths.juliauphome.join("juliaupgui_theme"), v) {
        eprintln!("Warning: could not save theme preference: {e}");
    }
}

fn load_bool_pref(paths: &GlobalPaths, name: &str) -> bool {
    std::fs::read_to_string(paths.juliauphome.join(name))
        .unwrap_or_default()
        .trim()
        == "true"
}

fn save_bool_pref(paths: &GlobalPaths, name: &str, val: bool) {
    if let Err(e) = std::fs::write(
        paths.juliauphome.join(name),
        if val { "true" } else { "false" },
    ) {
        eprintln!("Warning: could not save preference '{name}': {e}");
    }
}

// ── theme-aware colors ───────────────────────────────────────────────────────

fn tile_bg(dark: bool, hovered: bool) -> Color32 {
    match (dark, hovered) {
        (true, true) => Color32::from_rgb(42, 45, 58),
        (true, false) => Color32::from_rgb(36, 38, 50),
        (false, true) => Color32::from_rgb(222, 224, 230),
        (false, false) => Color32::from_rgb(240, 241, 245),
    }
}

fn tile_border(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(55, 58, 72)
    } else {
        Color32::from_rgb(195, 198, 210)
    }
}

/// Secondary text colors with at least 4.5:1 contrast against every custom
/// panel and tile surface in the corresponding theme.
fn secondary_text(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(150, 155, 175)
    } else {
        Color32::from_rgb(85, 85, 100)
    }
}

fn success_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(105, 210, 125)
    } else {
        Color32::from_rgb(10, 105, 45)
    }
}

fn warning_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(245, 185, 70)
    } else {
        Color32::from_rgb(125, 75, 0)
    }
}

fn error_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(255, 120, 105)
    } else {
        Color32::from_rgb(170, 35, 30)
    }
}

fn link_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(110, 185, 255)
    } else {
        Color32::from_rgb(0, 85, 155)
    }
}

fn focus_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(180, 140, 255)
    } else {
        Color32::from_rgb(90, 60, 170)
    }
}

fn scrim_base(dark: bool) -> (u8, u8, u8) {
    if dark {
        (28, 30, 36)
    } else {
        (220, 222, 228)
    }
}

/// Path to the juliaup binary (same directory as juliaupgui).
fn juliaup_binary() -> anyhow::Result<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent dir"))?;
    Ok(dir.join("juliaup"))
}

/// Strip ANSI escape codes and handle carriage-return overwriting.
fn clean_line(s: &str) -> String {
    // Take only the last segment when CR is used for in-place progress
    let s = s.split('\r').next_back().unwrap_or(s);
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // ESC [ ... final_byte  (CSI sequences)
            if chars.peek() == Some(&'[') {
                chars.next();
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                chars.next(); // skip 2-char ESC sequence
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Spawn the juliaup binary with `args`, send each output line as `Msg::Line`,
/// return `Ok(())` on success or `Err` with the exit status / spawn error.
fn spawn_and_stream(args: &[&str], tx: &mpsc::Sender<Msg>) -> anyhow::Result<()> {
    spawn_and_stream_with_pr_codesign(args, tx, false)
}

fn spawn_and_stream_with_pr_codesign(
    args: &[&str],
    tx: &mpsc::Sender<Msg>,
    approve_pr_codesign: bool,
) -> anyhow::Result<()> {
    use std::io::BufRead;
    use std::process::Stdio;

    let bin = juliaup_binary()?;
    let mut command = std::process::Command::new(&bin);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("JULIAUP_PR_CODESIGN");
    if approve_pr_codesign {
        command.env("JULIAUP_PR_CODESIGN", "yes");
    }
    let mut child = command
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn {}: {e}", bin.display()))?;

    // Read stderr in a background thread to avoid deadlock
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture stderr"))?;
    let tx2 = tx.clone();
    let stderr_thread = thread::spawn(move || {
        for l in std::io::BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
        {
            let clean = clean_line(&l);
            if !clean.trim().is_empty() {
                let _ = tx2.send(Msg::Line(clean));
            }
        }
    });

    // Read stdout in this thread
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to capture stdout"))?;
    for l in std::io::BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
    {
        let clean = clean_line(&l);
        if !clean.trim().is_empty() {
            let _ = tx.send(Msg::Line(clean));
        }
    }

    stderr_thread.join().ok();

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("exited with {status}"))
    }
}

fn op_label(op: &Op) -> String {
    match op {
        Op::Reload => "Reloading…".into(),
        Op::Add { channel, .. } => format!("Installing '{channel}'…"),
        Op::Remove(ch) => format!("Removing '{ch}'…"),
        Op::Update {
            channel: Some(ch), ..
        } => format!("Updating '{ch}'…"),
        Op::Update { channel: None, .. } => "Updating all channels…".into(),
        Op::SetDefault(ch) => format!("Setting default to '{ch}'…"),
        Op::SelfUpdate => "Updating juliaup…".into(),
        Op::Gc => "Running garbage collection…".into(),
        Op::UpdateVersionDb => "Refreshing version database…".into(),
        Op::SetVersionsDbInterval(_) => "Saving interval setting…".into(),
        Op::SetAutoInstall(_) => "Saving auto-install setting…".into(),
        Op::SetManifestDetect(_) => "Saving manifest detect setting…".into(),
        #[cfg(not(windows))]
        Op::SetChannelSymlinks(_) => "Saving symlinks setting…".into(),
        Op::SetOverride { path, channel } => format!("Setting override '{path}' -> '{channel}'…"),
        Op::UnsetOverride(p) => format!("Removing override '{p}'…"),
        Op::UnsetNonexistentOverrides => "Removing stale overrides…".into(),
        Op::Link { channel, .. } => format!("Linking channel '{channel}'…"),
    }
}

fn worker(rx: mpsc::Receiver<(Op, Arc<GlobalPaths>)>, tx: mpsc::Sender<Msg>) {
    while let Ok((op, paths)) = rx.recv() {
        let msg = exec(&op, &paths, &tx);
        if tx.send(msg).is_err() {
            break;
        }
    }
}

// Operations that produce streaming output (add, remove, update, link, self-update)
// spawn a juliaup subprocess to relay progress lines to the UI.
// Quick config changes are called directly as library functions.
fn exec(op: &Op, paths: &GlobalPaths, tx: &mpsc::Sender<Msg>) -> Msg {
    match op {
        Op::Reload => match load_state(paths) {
            Ok(s) => Msg::Loaded(Box::new(s)),
            Err(e) => Msg::Err(format!("Failed to load: {e}")),
        },
        Op::Add {
            channel,
            approve_pr_codesign,
        } => match spawn_and_stream_with_pr_codesign(&["add", channel], tx, *approve_pr_codesign) {
            Ok(_) => Msg::Ok(format!("Installed '{channel}'")),
            Err(e) => Msg::Err(format!("{e}")),
        },
        Op::Remove(ch) => match spawn_and_stream(&["remove", ch], tx) {
            Ok(_) => Msg::Ok(format!("Removed '{ch}'")),
            Err(e) => Msg::Err(format!("{e}")),
        },
        Op::Update {
            channel,
            approve_pr_codesign,
        } => {
            let args: Vec<&str> = match channel {
                Some(c) => vec!["update", c.as_str()],
                None => vec!["update"],
            };
            match spawn_and_stream_with_pr_codesign(&args, tx, *approve_pr_codesign) {
                Ok(_) => Msg::Ok(match channel {
                    Some(c) => format!("Updated '{c}'"),
                    None => "Updated all channels".to_string(),
                }),
                Err(e) => Msg::Err(format!("{e}")),
            }
        }
        Op::SetDefault(ch) => match run_command_default(ch, paths) {
            Ok(_) => Msg::Ok(format!("Default set to '{ch}'")),
            Err(e) => Msg::Err(format!("{e}")),
        },
        Op::SelfUpdate => match spawn_and_stream(&["self", "update"], tx) {
            Ok(_) => Msg::Ok("Juliaup updated successfully".to_string()),
            Err(e) => Msg::Err(format!("{e}")),
        },
        Op::Gc => match run_command_gc(false, paths) {
            Ok(_) => Msg::Ok("Garbage collection complete".to_string()),
            Err(e) => Msg::Err(format!("{e}")),
        },
        Op::UpdateVersionDb => match run_command_update_version_db(paths) {
            Ok(_) => Msg::Ok("Version database updated".to_string()),
            Err(e) => Msg::Err(format!("{e}")),
        },
        Op::SetVersionsDbInterval(v) => {
            match run_command_config_versionsdbupdate(Some(*v), false, paths) {
                Ok(_) => Msg::Ok(format!("DB update interval set to {v} min")),
                Err(e) => Msg::Err(format!("{e}")),
            }
        }
        Op::SetAutoInstall(v) => {
            let s = v.map(|b| {
                if b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            });
            match run_command_config_autoinstall(s, false, paths) {
                Ok(_) => Msg::Ok("Auto-install setting saved".to_string()),
                Err(e) => Msg::Err(format!("{e}")),
            }
        }
        Op::SetManifestDetect(v) => {
            match run_command_config_manifestversiondetect(Some(*v), false, paths) {
                Ok(_) => Msg::Ok("Manifest version detect updated".to_string()),
                Err(e) => Msg::Err(format!("{e}")),
            }
        }
        #[cfg(not(windows))]
        Op::SetChannelSymlinks(v) => match run_command_config_symlinks(Some(*v), false, paths) {
            Ok(_) => Msg::Ok("Channel symlinks setting updated".to_string()),
            Err(e) => Msg::Err(format!("{e}")),
        },
        Op::SetOverride { path, channel } => {
            match run_command_override_set(paths, channel.clone(), Some(path.clone())) {
                Ok(_) => Msg::Ok(format!("Override set: {path} → {channel}")),
                Err(e) => Msg::Err(format!("{e}")),
            }
        }
        Op::UnsetOverride(path) => {
            match run_command_override_unset(paths, false, Some(path.clone())) {
                Ok(_) => Msg::Ok(format!("Override removed for '{path}'")),
                Err(e) => Msg::Err(format!("{e}")),
            }
        }
        Op::UnsetNonexistentOverrides => {
            let result = (|| -> anyhow::Result<()> {
                let mut cfg = load_mut_config_db(paths)?;
                cfg.data
                    .overrides
                    .retain(|o| std::path::Path::new(&o.path).is_dir());
                save_config_db(&mut cfg, paths)?;
                Ok(())
            })();
            match result {
                Ok(_) => Msg::Ok("Non-existent overrides removed".to_string()),
                Err(e) => Msg::Err(format!("{e}")),
            }
        }
        Op::Link {
            channel,
            target,
            args,
        } => {
            let mut cli_args = vec!["link", channel.as_str(), target.as_str()];
            let arg_strs: Vec<&str> = args.iter().map(String::as_str).collect();
            cli_args.extend_from_slice(&arg_strs);
            match spawn_and_stream(&cli_args, tx) {
                Ok(_) => Msg::Ok(format!("Linked '{channel}' → '{target}'")),
                Err(e) => Msg::Err(format!("{e}")),
            }
        }
    }
}

// ── data loading ──────────────────────────────────────────────────────────────

fn load_state(paths: &GlobalPaths) -> anyhow::Result<AppState> {
    let config = load_config_db(paths, None)?;
    let versiondb = load_versions_db(paths)?;

    let installed = build_installed(&config, &versiondb);
    let installed_keys: std::collections::HashSet<_> =
        config.data.installed_channels.keys().cloned().collect();
    let available = build_available(&versiondb, &installed_keys)?;
    let overrides = config
        .data
        .overrides
        .iter()
        .map(|o| OverrideRow {
            path: o.path.clone(),
            channel: o.channel.clone(),
        })
        .collect();

    Ok(AppState {
        installed,
        available,
        overrides,
        settings: config.data.settings.clone(),
    })
}

fn build_installed(
    config: &juliaup::config_file::JuliaupReadonlyConfigFile,
    versiondb: &JuliaupVersionDB,
) -> Vec<InstalledRow> {
    config
        .data
        .installed_channels
        .iter()
        .sorted_by(|(a, _), (b, _)| cmp(a, b))
        .map(|(name, ch)| InstalledRow {
            version: fmt_version(ch),
            is_default: config.data.default.as_deref() == Some(name.as_str()),
            update: update_info(name, ch, config, versiondb),
            pr_number: installed_pr_number(name, ch, config),
            name: name.clone(),
        })
        .collect()
}

fn installed_pr_number(
    name: &str,
    channel: &JuliaupConfigChannel,
    config: &juliaup::config_file::JuliaupReadonlyConfigFile,
) -> Option<String> {
    match channel {
        JuliaupConfigChannel::DirectDownloadChannel { .. } => {
            julia_pr_number(name).map(String::from)
        }
        JuliaupConfigChannel::AliasChannel { target, .. } => config
            .data
            .installed_channels
            .get(target)
            .and_then(|target_channel| installed_pr_number(target, target_channel, config)),
        JuliaupConfigChannel::SystemChannel { .. } | JuliaupConfigChannel::LinkedChannel { .. } => {
            None
        }
    }
}

fn julia_pr_number(channel: &str) -> Option<&str> {
    let channel = channel.split_once('~').map_or(channel, |(base, _)| base);
    let number = channel.strip_prefix("pr")?;
    (!number.is_empty() && number.chars().all(|c| c.is_ascii_digit())).then_some(number)
}

fn julia_pr_url(number: &str) -> String {
    format!("https://github.com/JuliaLang/julia/pull/{number}")
}

fn pr_link(ui: &mut egui::Ui, number: &str, title: Option<&str>) -> egui::Response {
    ui.hyperlink_to(format!("#{number}"), julia_pr_url(number))
        .on_hover_text(pr_link_hover_text(number, title))
}

fn pr_link_hover_text(number: &str, title: Option<&str>) -> String {
    title.map_or_else(
        || format!("Open Julia pull request #{number} on GitHub"),
        |title| format!("#{number}: {title}\nOpen on GitHub"),
    )
}

fn build_available(
    versiondb: &JuliaupVersionDB,
    installed: &std::collections::HashSet<String>,
) -> anyhow::Result<Vec<AvailableRow>> {
    let non_db: Vec<String> = get_channel_variations("nightly")?
        .into_iter()
        .chain(get_channel_variations("x.y-nightly")?)
        .chain(get_channel_variations("pr{number}")?)
        .collect();

    let rows: Vec<AvailableRow> = versiondb
        .available_channels
        .iter()
        .sorted_by(|(a, _), (b, _)| cmp(a, b))
        .map(|(ch, info)| AvailableRow {
            channel: ch.clone(),
            version: info.version.clone(),
            installed: installed.contains(ch),
        })
        .chain(non_db.into_iter().map(|ch| AvailableRow {
            installed: installed.contains(&ch),
            version: "dynamic".to_string(),
            channel: ch,
        }))
        .collect();

    Ok(rows)
}

fn fmt_version(ch: &JuliaupConfigChannel) -> String {
    match ch {
        JuliaupConfigChannel::DirectDownloadChannel { version, .. } => {
            format!("Dev {version}")
        }
        JuliaupConfigChannel::SystemChannel { version } => version.clone(),
        JuliaupConfigChannel::LinkedChannel { command, args } => {
            let suffix = args
                .as_ref()
                .map(|a| format!(" {}", a.join(" ")))
                .unwrap_or_default();
            format!("Linked → {command}{suffix}")
        }
        JuliaupConfigChannel::AliasChannel { target, args } => {
            let suffix = args
                .as_ref()
                .filter(|a| !a.is_empty())
                .map(|a| format!(" ({})", a.join(" ")))
                .unwrap_or_default();
            format!("Alias → {target}{suffix}")
        }
    }
}

fn update_info(
    name: &str,
    ch: &JuliaupConfigChannel,
    config: &juliaup::config_file::JuliaupReadonlyConfigFile,
    versiondb: &JuliaupVersionDB,
) -> Option<String> {
    match ch {
        JuliaupConfigChannel::DirectDownloadChannel {
            local_etag,
            server_etag,
            ..
        } => (local_etag != server_etag).then(|| "Update available".to_string()),
        JuliaupConfigChannel::SystemChannel { version } => versiondb
            .available_channels
            .get(name)
            .filter(|c| &c.version != version)
            .map(|c| format!("→ {}", c.version)),
        JuliaupConfigChannel::LinkedChannel { .. } => None,
        JuliaupConfigChannel::AliasChannel { target, .. } => config
            .data
            .installed_channels
            .get(target)
            .and_then(|tc| update_info(target, tc, config, versiondb)),
    }
}

// ── splash animation ──────────────────────────────────────────────────────────

// Total animation: hold (0.3s) -> fade out (0.4s)
const SPLASH_DURATION: f32 = 0.7;

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn paint_splash(ctx: &egui::Context, t: f32, _logo_rect: Option<egui::Rect>) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("splash"),
    ));

    let hold_dur = 0.3;
    let fade_dur = 0.4;

    // Scrim: opaque during hold, fades out
    let alpha = if t < hold_dur {
        200u8
    } else {
        let p = ((t - hold_dur) / fade_dur).clamp(0.0, 1.0);
        (200.0 * (1.0 - ease_out_cubic(p))) as u8
    };
    if alpha > 0 {
        let (sr, sg, sb) = scrim_base(ctx.style().visuals.dark_mode);
        painter.rect_filled(
            screen,
            0.0,
            Color32::from_rgba_unmultiplied(sr, sg, sb, alpha),
        );
    }

    // Centered logo, fading with the scrim
    let center = screen.center();
    let big_size: f32 = 120.0;
    let s = big_size / 350.0;
    let origin = center - egui::Vec2::new(175.0 * s, 175.0 * s);

    for (i, &(cx, cy, dot_r, red, green, blue)) in JULIA_DOTS.iter().enumerate() {
        let delay = i as f32 * 0.06;
        let dt = (t - delay).max(0.0);
        let grow = ease_out_cubic((dt / 0.3).clamp(0.0, 1.0));

        let pos = origin + egui::Vec2::new(cx * s, cy * s);
        let r = dot_r * s * (0.3 + 0.7 * grow);
        let col = if alpha >= 200 {
            Color32::from_rgb(red, green, blue)
        } else {
            Color32::from_rgba_unmultiplied(red, green, blue, alpha)
        };
        painter.circle_filled(pos, r, col);
    }
}

// ── Julia logo ───────────────────────────────────────────────────────────────

/// Paint the three Julia dots into a given rect.
fn paint_julia_dots(p: &egui::Painter, rect: egui::Rect) {
    let s = rect.width() / 350.0;
    let origin = rect.min;
    for &(cx, cy, r, red, green, blue) in &JULIA_DOTS {
        let center = origin + egui::Vec2::new(cx * s, cy * s);
        p.circle_filled(center, r * s, Color32::from_rgb(red, green, blue));
    }
}

// ── theme ─────────────────────────────────────────────────────────────────────

fn apply_theme(ctx: &egui::Context, mode: ThemeMode) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let dark = theme == egui::Theme::Dark;
        let mut style = theme.default_style();
        let visuals = &mut style.visuals;

        if dark {
            visuals.panel_fill = Color32::from_rgb(28, 30, 36);
            visuals.window_fill = Color32::from_rgb(28, 30, 36);
            visuals.faint_bg_color = Color32::from_rgb(34, 36, 44);
        } else {
            visuals.panel_fill = Color32::from_rgb(248, 249, 252);
            visuals.window_fill = Color32::WHITE;
            visuals.faint_bg_color = Color32::from_rgb(238, 240, 245);
        }

        let primary_text = if dark {
            Color32::from_rgb(205, 205, 215)
        } else {
            Color32::from_rgb(70, 70, 80)
        };
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, primary_text);
        visuals.hyperlink_color = link_color(dark);
        visuals.warn_fg_color = warning_color(dark);
        visuals.error_fg_color = error_color(dark);

        // Rounded controls
        let r = egui::Rounding::same(5.0);
        visuals.widgets.noninteractive.rounding = r;
        visuals.widgets.inactive.rounding = r;
        visuals.widgets.hovered.rounding = r;
        visuals.widgets.active.rounding = r;
        visuals.menu_rounding = r;
        visuals.window_rounding = egui::Rounding::same(8.0);

        // Accent colour for selected items (Julia purple-ish)
        visuals.selection.bg_fill = Color32::from_rgb(90, 60, 170);
        visuals.selection.stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);

        // Ensure text on the purple selection background is always legible
        visuals.widgets.active.fg_stroke = egui::Stroke::new(
            1.0_f32,
            if dark {
                Color32::WHITE
            } else {
                Color32::from_gray(20)
            },
        );
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(
            1.0_f32,
            if dark {
                Color32::WHITE
            } else {
                Color32::from_gray(30)
            },
        );

        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(16.0, egui::FontFamily::Proportional),
        );
        style.spacing.button_padding = egui::Vec2::new(7.0, 3.0);
        style.spacing.item_spacing = egui::Vec2::new(6.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.spacing.window_margin = egui::Margin::same(8.0);
        style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

        ctx.set_style_of(theme, style);
    }

    ctx.set_theme(match mode {
        ThemeMode::Dark => egui::ThemePreference::Dark,
        ThemeMode::Light => egui::ThemePreference::Light,
        ThemeMode::System => egui::ThemePreference::System,
    });
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(match mode {
        ThemeMode::Dark => egui::SystemTheme::Dark,
        ThemeMode::Light => egui::SystemTheme::Light,
        ThemeMode::System => egui::SystemTheme::SystemDefault,
    }));
}

// ── public API ────────────────────────────────────────────────────────────────

/// Rasterise the Julia three-dot logo into a square RGBA icon.
/// Uses the exact SVG circle positions (viewBox 0 0 350 350) with
/// per-pixel anti-aliasing for smooth edges.
fn julia_logo_icon(size: u32) -> egui::IconData {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let scale = 350.0 / size as f32;

    for py in 0..size {
        for px in 0..size {
            let fx = (px as f32 + 0.5) * scale;
            let fy = (py as f32 + 0.5) * scale;
            let idx = ((py * size + px) * 4) as usize;

            for &(cx, cy, r, red, green, blue) in &JULIA_DOTS {
                let dist = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() - r;
                if dist < 1.0 {
                    let alpha = ((1.0 - dist).clamp(0.0, 1.0) * 255.0) as u8;
                    rgba[idx] = red;
                    rgba[idx + 1] = green;
                    rgba[idx + 2] = blue;
                    rgba[idx + 3] = alpha;
                    break;
                }
            }
        }
    }

    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

pub fn run(paths: GlobalPaths) -> anyhow::Result<()> {
    let icon = std::sync::Arc::new(julia_logo_icon(256));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Juliaup")
            .with_icon(icon)
            .with_inner_size([860.0, 540.0])
            .with_min_inner_size([754.0, 380.0]),
        ..Default::default()
    };

    eframe::run_native(
        "juliaup",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc, paths)))),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── clean_line ────────────────────────────────────────────────────────

    #[test]
    fn clean_line_plain_text() {
        assert_eq!(clean_line("hello world"), "hello world");
    }

    #[test]
    fn clean_line_empty() {
        assert_eq!(clean_line(""), "");
    }

    #[test]
    fn clean_line_strips_ansi_color() {
        assert_eq!(clean_line("\x1b[32mOK\x1b[0m"), "OK");
    }

    #[test]
    fn clean_line_strips_multipart_ansi() {
        assert_eq!(
            clean_line("\x1b[1;34mblue\x1b[0m and \x1b[31mred\x1b[0m"),
            "blue and red"
        );
    }

    #[test]
    fn clean_line_handles_cr_overwrite() {
        assert_eq!(clean_line("old\rnew"), "new");
    }

    #[test]
    fn clean_line_cr_with_ansi() {
        assert_eq!(clean_line("first\r\x1b[32msecond\x1b[0m"), "second");
    }

    // ── launch options ───────────────────────────────────────────────────

    #[test]
    fn split_launch_options_preserves_quoted_values_and_paths() {
        assert_eq!(
            split_user_input(r#"--threads auto -e 'println("hello world")' "" C:\Julia\sys.dll"#)
                .unwrap(),
            vec![
                "--threads",
                "auto",
                "-e",
                r#"println("hello world")"#,
                "",
                r"C:\Julia\sys.dll",
            ]
        );
    }

    #[test]
    fn split_launch_options_supports_escaped_whitespace() {
        assert_eq!(
            split_user_input(r"--project=my\ project").unwrap(),
            vec!["--project=my project"]
        );
    }

    #[test]
    fn split_launch_options_rejects_unterminated_quotes() {
        assert_eq!(
            split_user_input(r#""unfinished"#).unwrap_err(),
            "Unterminated \" quote in launch options"
        );
    }

    #[test]
    fn build_launch_spec_preserves_structured_arguments() {
        let spec = build_launch_spec(
            std::path::PathBuf::from("/opt/juliaup/bin/julia"),
            "release",
            "/tmp/my project",
            r#"-e 'println("hello world")'"#,
            r#"JULIA_NUM_THREADS=4 GREETING="hello world""#,
        )
        .unwrap();

        assert_eq!(
            spec.args,
            vec![
                "+release",
                "--project=/tmp/my project",
                "-e",
                r#"println("hello world")"#,
            ]
        );
        assert_eq!(
            spec.env,
            vec![
                ("JULIA_NUM_THREADS".to_string(), "4".to_string()),
                ("GREETING".to_string(), "hello world".to_string()),
            ]
        );
        assert_eq!(
            spec.project,
            Some(std::path::PathBuf::from("/tmp/my project"))
        );
    }

    #[test]
    fn build_launch_spec_rejects_malformed_environment_variables() {
        let julia = std::path::PathBuf::from("/opt/juliaup/bin/julia");

        assert_eq!(
            build_launch_spec(julia.clone(), "release", "", "", "NO_EQUALS").unwrap_err(),
            "Environment variable 'NO_EQUALS' must use the KEY=VALUE format"
        );
        assert_eq!(
            build_launch_spec(julia, "release", "", "", "1INVALID=value").unwrap_err(),
            "Invalid environment variable name '1INVALID'"
        );
    }

    #[test]
    fn julia_pr_channels_link_to_the_matching_github_pr() {
        assert_eq!(julia_pr_number("pr123"), Some("123"));
        assert_eq!(julia_pr_number("pr123~x64"), Some("123"));
        assert_eq!(julia_pr_number("pr00123~aarch64"), Some("00123"));
        assert_eq!(
            julia_pr_url("123"),
            "https://github.com/JuliaLang/julia/pull/123"
        );
    }

    #[test]
    fn non_pr_channels_do_not_get_github_pr_links() {
        for channel in ["pr", "pr{number}", "pr123-extra", "xpr123", "release"] {
            assert_eq!(julia_pr_number(channel), None, "{channel}");
        }
    }

    #[test]
    fn pr_link_hover_text_includes_the_pr_title_when_available() {
        assert_eq!(
            pr_link_hover_text("123", Some("Fix macOS PR installs")),
            "#123: Fix macOS PR installs\nOpen on GitHub"
        );
        assert_eq!(
            pr_link_hover_text("123", None),
            "Open Julia pull request #123 on GitHub"
        );
    }

    #[test]
    fn system_theme_uses_the_theme_reported_by_the_os() {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput {
            system_theme: Some(egui::Theme::Light),
            ..Default::default()
        });

        apply_theme(&ctx, ThemeMode::System);

        assert_eq!(ctx.theme(), egui::Theme::Light);
        assert_eq!(
            ctx.style().visuals.panel_fill,
            Color32::from_rgb(248, 249, 252)
        );
        let _ = ctx.end_pass();
    }

    fn relative_luminance(color: Color32) -> f32 {
        let linear = |component: u8| {
            let value = component as f32 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }

    fn contrast_ratio(a: Color32, b: Color32) -> f32 {
        let (lighter, darker) = {
            let a = relative_luminance(a);
            let b = relative_luminance(b);
            if a >= b {
                (a, b)
            } else {
                (b, a)
            }
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    fn assert_contrast(label: &str, foreground: Color32, background: Color32, minimum: f32) {
        let ratio = contrast_ratio(foreground, background);
        assert!(
            ratio >= minimum,
            "{label} contrast was {ratio:.2}:1, expected at least {minimum:.1}:1"
        );
    }

    #[test]
    fn custom_palette_meets_wcag_contrast_targets() {
        for dark in [true, false] {
            let panel = if dark {
                Color32::from_rgb(28, 30, 36)
            } else {
                Color32::from_rgb(248, 249, 252)
            };
            let tile = tile_bg(dark, false);
            let hovered_tile = tile_bg(dark, true);
            let button = if dark {
                Color32::from_gray(60)
            } else {
                Color32::from_gray(230)
            };
            let text_surfaces = [panel, tile, hovered_tile];

            for surface in text_surfaces {
                assert_contrast("secondary text", secondary_text(dark), surface, 4.5);
                assert_contrast("success text", success_color(dark), surface, 4.5);
                assert_contrast("warning text", warning_color(dark), surface, 4.5);
                assert_contrast("error text", error_color(dark), surface, 4.5);
                assert_contrast("link text", link_color(dark), surface, 4.5);
                assert_contrast("focus indicator", focus_color(dark), surface, 3.0);
            }
            assert_contrast("success button text", success_color(dark), button, 4.5);
        }

        assert_contrast(
            "selected text",
            Color32::WHITE,
            Color32::from_rgb(90, 60, 170),
            4.5,
        );
    }

    // ── shell_quote / build_launch_cmd (POSIX) ───────────────────────────

    #[cfg(not(target_os = "windows"))]
    mod posix {
        use super::super::*;

        fn launch_spec(
            channel: &str,
            project: &str,
            extra_args: &str,
            env_vars: &str,
        ) -> LaunchSpec {
            build_launch_spec(
                std::path::PathBuf::from("/opt/juliaup/bin/julia"),
                channel,
                project,
                extra_args,
                env_vars,
            )
            .unwrap()
        }

        #[test]
        fn shell_quote_simple() {
            assert_eq!(shell_quote("hello"), "'hello'");
        }

        #[test]
        fn shell_quote_with_spaces() {
            assert_eq!(shell_quote("hello world"), "'hello world'");
        }

        #[test]
        fn shell_quote_with_single_quotes() {
            assert_eq!(shell_quote("it's"), "'it'\\''s'");
        }

        #[test]
        fn shell_quote_empty() {
            assert_eq!(shell_quote(""), "''");
        }

        #[test]
        fn build_cmd_basic_channel() {
            let spec = launch_spec("release", "", "", "");
            assert_eq!(
                build_launch_cmd(&spec),
                "'/opt/juliaup/bin/julia' '+release'"
            );
        }

        #[test]
        fn build_cmd_with_project() {
            let spec = launch_spec("1.10", "/tmp/my project", "", "");
            assert_eq!(
                build_launch_cmd(&spec),
                "cd '/tmp/my project' && '/opt/juliaup/bin/julia' '+1.10' \
                 '--project=/tmp/my project'"
            );
        }

        #[test]
        fn build_cmd_with_env_vars() {
            let spec = launch_spec("release", "", "", "JULIA_NUM_THREADS=4");
            assert_eq!(
                build_launch_cmd(&spec),
                "JULIA_NUM_THREADS='4' '/opt/juliaup/bin/julia' '+release'"
            );
        }

        #[test]
        fn build_cmd_with_extra_args() {
            let spec = launch_spec("release", "", "--threads=4 -q", "");
            assert_eq!(
                build_launch_cmd(&spec),
                "'/opt/juliaup/bin/julia' '+release' '--threads=4' '-q'"
            );
        }

        #[test]
        fn build_cmd_full() {
            let spec = launch_spec(
                "1.10",
                "/home/user/proj",
                "-q --startup-file=no",
                "JULIA_NUM_THREADS=auto",
            );
            assert_eq!(
                build_launch_cmd(&spec),
                "cd '/home/user/proj' && JULIA_NUM_THREADS='auto' \
                 '/opt/juliaup/bin/julia' '+1.10' '--project=/home/user/proj' '-q' \
                 '--startup-file=no'"
            );
        }
    }

    // ── julia_logo_icon ──────────────────────────────────────────────────

    #[test]
    fn logo_icon_correct_dimensions() {
        let icon = julia_logo_icon(64);
        assert_eq!(icon.width, 64);
        assert_eq!(icon.height, 64);
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
    }

    #[test]
    fn logo_icon_has_nonzero_pixels() {
        let icon = julia_logo_icon(64);
        let nonzero = icon.rgba.chunks(4).any(|px| px[3] > 0);
        assert!(
            nonzero,
            "icon should contain at least one non-transparent pixel"
        );
    }

    // ── JULIA_DOTS constant ──────────────────────────────────────────────

    #[test]
    fn julia_dots_has_three_entries() {
        assert_eq!(JULIA_DOTS.len(), 3);
    }

    #[test]
    fn julia_dots_colors_are_distinct() {
        let colors: Vec<_> = JULIA_DOTS.iter().map(|d| (d.3, d.4, d.5)).collect();
        assert_ne!(colors[0], colors[1]);
        assert_ne!(colors[1], colors[2]);
        assert_ne!(colors[0], colors[2]);
    }

    // ── default_terminal_hint ────────────────────────────────────────────

    #[test]
    fn default_terminal_hint_is_nonempty() {
        assert!(!default_terminal_hint().is_empty());
    }
}
