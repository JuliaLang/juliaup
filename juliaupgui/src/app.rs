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
use juliaup::operations::get_channel_variations;
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
    Add(String),
    Remove(String),
    Update(Option<String>),
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
    pr_prompt: Option<String>,
    pr_number_input: String,
    avail_expanded: HashSet<String>,

    // Custom launch prompt
    custom_launch_channel: Option<String>,
    custom_launch_project: String,
    custom_launch_args: String,
    custom_launch_env: String,

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
    paths: Arc<GlobalPaths>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, paths: GlobalPaths) -> Self {
        let paths = Arc::new(paths);
        let theme_mode = load_theme_pref(&paths);
        apply_theme(&cc.egui_ctx, theme_mode);
        let (op_tx, op_rx) = mpsc::sync_channel::<(Op, Arc<GlobalPaths>)>(8);
        let (msg_tx, msg_rx) = mpsc::channel::<Msg>();

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
            pr_prompt: None,
            pr_number_input: String::new(),
            avail_expanded: HashSet::new(),
            custom_launch_channel: None,
            custom_launch_project: String::new(),
            custom_launch_args: String::new(),
            custom_launch_env: String::new(),
            interval_input: String::new(),
            terminal_app,
            theme_mode,
            list_tip_dismissed,
            ov_path: String::new(),
            ov_channel: String::new(),
            splash_start: Instant::now(),
            op_tx,
            msg_rx,
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
        let _ = self.op_tx.try_send((op, self.paths.clone()));
    }

    fn poll(&mut self) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
                Msg::Loaded(s) => {
                    self.interval_input = s.settings.versionsdb_update_interval.to_string();
                    self.state = Some(*s);
                    self.loading = false;
                    self.busy = false;
                    self.current_op = None;
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
                    let _ = self.op_tx.try_send((Op::Reload, self.paths.clone()));
                }
                Msg::Err(m) => {
                    self.log.push(LogEntry {
                        text: m.clone(),
                        kind: LogKind::Err,
                    });
                    self.status = Some((m, true));
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
                    // Only paint the static logo once splash is done
                    if !splash_active && ui.is_rect_visible(rect) {
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
                            .color(subtle_text(ui.visuals().dark_mode)),
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
                            Color32::from_rgb(220, 80, 60)
                        } else {
                            Color32::from_rgb(80, 190, 100)
                        };
                        let icon = if *is_err { "x" } else { "ok" };
                        ui.colored_label(col, RichText::new(format!("{icon} {msg}")).size(12.0));
                    } else {
                        ui.label(RichText::new("Ready").size(12.0).weak());
                    }

                    // Right: toggle log button
                    if !self.log.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            let toggle_label = if self.log_open {
                                RichText::new("Hide log v").size(11.0).weak()
                            } else {
                                RichText::new(format!("Log ({}) ^", self.log.len()))
                                    .size(11.0)
                                    .weak()
                            };
                            if ui.small_button(toggle_label).clicked() {
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
                                    LogKind::Output => (subtle_text(ui.visuals().dark_mode), ""),
                                    LogKind::Ok => (Color32::from_rgb(80, 190, 100), "ok "),
                                    LogKind::Err => (Color32::from_rgb(220, 80, 60), "x  "),
                                };
                                ui.label(
                                    RichText::new(format!("{prefix}{}", entry.text))
                                        .size(11.0)
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
                                .hint_text("/path/to/project")
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
                            .weak()
                            .size(11.0),
                    );

                    ui.add_space(6.0);
                    ui.label("Environment variables:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.custom_launch_env)
                            .hint_text("JULIA_DEBUG=Foo BAR=1 ...")
                            .desired_width(ui.available_width().min(380.0))
                            .font(egui::TextStyle::Monospace),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Space-separated KEY=VALUE pairs prepended to the command.")
                            .weak()
                            .size(11.0),
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("julia +{ch}"))
                                .monospace()
                                .color(muted_text(ui.visuals().dark_mode)),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.custom_launch_args)
                                .hint_text("--threads=4 ...")
                                .desired_width(200.0)
                                .font(egui::TextStyle::Monospace),
                        );
                    });
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        if ui
                            .button(RichText::new("Launch").color(Color32::from_rgb(80, 190, 100)))
                            .clicked()
                        {
                            launch_julia(
                                &ch,
                                &self.terminal_app,
                                &self.custom_launch_project,
                                &self.custom_launch_args,
                                &self.custom_launch_env,
                            );
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
    }
}

// ── installed tab ─────────────────────────────────────────────────────────────

fn tab_installed(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!app.busy, egui::Button::new("Refresh").small())
            .clicked()
        {
            app.send(Op::Reload);
        }
        if ui
            .add_enabled(!app.busy, egui::Button::new("Update All").small())
            .on_hover_text("Update every installed channel to its latest version")
            .clicked()
        {
            app.send(Op::Update(None));
        }
        if ui
            .add_enabled(!app.busy, egui::Button::new("GC").small())
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
                                        .weak()
                                        .size(12.0),
                                );
                                if ui.small_button("x").clicked() {
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
            ui.label(RichText::new("No Julia channels installed.").weak());
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
                        let border_col = if row.is_default {
                            Color32::from_rgb(80, 190, 100)
                        } else {
                            tile_border(ui.visuals().dark_mode)
                        };

                        // Allocate a fixed rect for hit-testing the whole tile
                        let tile_outer = egui::vec2(TILE_W + MARGIN * 2.0, TILE_H + MARGIN * 2.0);
                        let tile_id = ui.id().with(("tile", i));
                        let tile_rect = ui.allocate_space(tile_outer).1;
                        let tile_resp = ui.interact(tile_rect, tile_id, egui::Sense::click());
                        let hovered = tile_resp.hovered();

                        // Pointer cursor on hover
                        if hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        // Subtle hover effects
                        let dark = ui.visuals().dark_mode;
                        let fill = tile_bg(dark, hovered);
                        let stroke_w = if hovered { 2.0 } else { 1.5 };
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
                            ui.add(
                                egui::Label::new(RichText::new(&row.name).size(18.0).strong())
                                    .truncate(),
                            );

                            ui.add_space(1.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&row.version)
                                        .size(11.0)
                                        .color(muted_text(ui.visuals().dark_mode)),
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
                                                .size(10.0)
                                                .color(Color32::from_rgb(80, 190, 100)),
                                        );
                                    }
                                    if let Some(upd) = &row.update {
                                        ui.label(
                                            RichText::new(format!("-> {upd}"))
                                                .size(10.0)
                                                .color(Color32::from_rgb(230, 170, 50)),
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
                                if ui
                                    .add_sized(
                                        [launch_w, btn_h],
                                        egui::Button::new(
                                            RichText::new("Launch")
                                                .size(12.0)
                                                .color(Color32::from_rgb(80, 190, 100)),
                                        ),
                                    )
                                    .on_hover_text(format!("Start julia +{}", row.name))
                                    .clicked()
                                {
                                    do_launch = Some(row.name.clone());
                                }
                                if ui
                                    .add_sized(
                                        [launch_w, btn_h],
                                        egui::Button::new(RichText::new("Custom...").size(11.0)),
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
                                    if ui
                                        .add_sized(
                                            [action_w, btn_h],
                                            egui::Button::new(RichText::new("Default").size(11.0)),
                                        )
                                        .on_hover_text(
                                            "Use this channel when no version is specified",
                                        )
                                        .clicked()
                                    {
                                        set_def = Some(row.name.clone());
                                    }
                                    if has_update
                                        && ui
                                            .add_sized(
                                                [action_w, btn_h],
                                                egui::Button::new(
                                                    RichText::new("Update").size(11.0),
                                                ),
                                            )
                                            .on_hover_text("Update to latest")
                                            .clicked()
                                    {
                                        do_update = Some(row.name.clone());
                                    }
                                    if ui
                                        .add_sized(
                                            [action_w, btn_h],
                                            egui::Button::new(RichText::new("Remove").size(11.0)),
                                        )
                                        .on_hover_text("Remove this channel")
                                        .clicked()
                                    {
                                        do_remove = Some(row.name.clone());
                                    }
                                });
                            }
                        }

                        // Whole-tile click = launch default
                        if tile_resp.clicked() {
                            do_launch = Some(state.installed[i].name.clone());
                        }
                    } else {
                        // "Add another channel" tile
                        let add_outer = egui::vec2(TILE_W + MARGIN * 2.0, TILE_H + MARGIN * 2.0);
                        let add_id = ui.id().with("add_tile");
                        let add_rect = ui.allocate_space(add_outer).1;
                        let add_resp = ui.interact(add_rect, add_id, egui::Sense::click());
                        let add_hov = add_resp.hovered();
                        if add_hov {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        let add_dark = ui.visuals().dark_mode;
                        let add_fill = tile_bg(add_dark, add_hov);
                        let add_stroke_w = if add_hov { 2.0 } else { 1.5 };
                        ui.painter().rect(
                            add_rect,
                            egui::Rounding::same(8.0),
                            add_fill,
                            egui::Stroke::new(add_stroke_w, tile_border(add_dark)),
                        );
                        // Center the label in the tile
                        ui.painter().text(
                            add_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "+ Add channel",
                            egui::FontId::proportional(14.0),
                            subtle_text(add_dark),
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
        launch_julia(&ch, &app.terminal_app, "", "", "");
    }
    if let Some(ch) = set_def {
        app.send(Op::SetDefault(ch));
    }
    if let Some(ch) = do_update {
        app.send(Op::Update(Some(ch)));
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
        .column(Column::initial(100.0).at_least(60.0)) // Channel
        .column(Column::remainder().at_least(120.0)) // Version (takes leftover)
        .column(Column::initial(130.0).at_least(80.0)) // Update
        .column(Column::exact(160.0)) // Launch
        .column(Column::exact(150.0)) // Actions
        .min_scrolled_height(0.0)
        .max_scroll_height(body_height)
        .header(18.0, |mut header| {
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
                body.row(22.0, |mut cells| {
                    cells.col(|ui| {
                        if row.is_default {
                            ui.label(
                                RichText::new("*")
                                    .color(Color32::from_rgb(80, 190, 100))
                                    .strong(),
                            );
                        }
                    });
                    cells.col(|ui| {
                        ui.label(RichText::new(&row.name).size(13.0));
                    });
                    cells.col(|ui| {
                        ui.label(RichText::new(&row.version).size(13.0).weak());
                    });
                    cells.col(|ui| {
                        if let Some(upd) = &row.update {
                            ui.label(
                                RichText::new(upd)
                                    .size(12.0)
                                    .color(Color32::from_rgb(230, 170, 50)),
                            );
                        } else {
                            ui.label(RichText::new("-").weak().size(12.0));
                        }
                    });
                    cells.col(|ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Launch")
                                            .size(12.0)
                                            .color(Color32::from_rgb(80, 190, 100)),
                                    )
                                    .small(),
                                )
                                .on_hover_text(format!("Start julia +{}", row.name))
                                .clicked()
                            {
                                do_launch = Some(row.name.clone());
                            }
                            if ui
                                .add(
                                    egui::Button::new(RichText::new("Custom...").size(11.0))
                                        .small(),
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
                                && ui
                                    .add_enabled(
                                        !app.busy,
                                        egui::Button::new(RichText::new("Default").size(11.0))
                                            .small(),
                                    )
                                    .on_hover_text("Set as default channel")
                                    .clicked()
                            {
                                set_def = Some(row.name.clone());
                            }
                            if row.update.is_some()
                                && ui
                                    .add_enabled(
                                        !app.busy,
                                        egui::Button::new(RichText::new("Up").size(11.0)).small(),
                                    )
                                    .on_hover_text("Update this channel")
                                    .clicked()
                            {
                                do_update = Some(row.name.clone());
                            }
                            if !row.is_default
                                && ui
                                    .add_enabled(
                                        !app.busy,
                                        egui::Button::new(RichText::new("Remove").size(11.0))
                                            .small(),
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
        launch_julia(&ch, &app.terminal_app, "", "", "");
    }
    if let Some(ch) = set_def {
        app.send(Op::SetDefault(ch));
    }
    if let Some(ch) = do_update {
        app.send(Op::Update(Some(ch)));
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
                .hint_text("channel name…")
                .desired_width(160.0),
        );
        ui.add_space(8.0);
        if ui
            .add_enabled(!app.busy, egui::Button::new("Refresh DB").small())
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
                            .hint_text("e.g. myjulia")
                            .desired_width(140.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new("Path or +channel:").size(12.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.link_target)
                            .hint_text("/path/to/julia or +1.10")
                            .desired_width(200.0),
                    );
                    ui.end_row();
                    ui.label(RichText::new("Extra args:").size(12.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.link_args)
                            .hint_text("optional")
                            .desired_width(200.0),
                    );
                    ui.end_row();
                });
            let can_link = !app.busy
                && !app.link_channel.trim().is_empty()
                && !app.link_target.trim().is_empty();
            if ui
                .add_enabled(can_link, egui::Button::new("Link").small())
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
        .column(Column::exact(100.0)) // Action
        .min_scrolled_height(0.0)
        .max_scroll_height(table_height)
        .header(18.0, |mut header| {
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

                body.row(22.0, |mut cells| {
                    cells.col(|ui| {
                        ui.horizontal(|ui| {
                            if depth > 0 {
                                ui.add_space(depth as f32 * 14.0);
                            }
                            if has_children && !filtering {
                                let expanded = app.avail_expanded.contains(row.channel.as_str());
                                let icon = if expanded { "v " } else { "> " };
                                if ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(icon).monospace().weak().size(11.0),
                                        )
                                        .sense(egui::Sense::click()),
                                    )
                                    .clicked()
                                {
                                    if expanded {
                                        app.avail_expanded.remove(row.channel.as_str());
                                    } else {
                                        app.avail_expanded.insert(row.channel.clone());
                                    }
                                }
                            } else if !filtering && depth > 0 {
                                ui.add_space(16.0);
                            }
                            ui.add(egui::Label::new(&row.channel).truncate());
                        });
                    });
                    cells.col(|ui| {
                        ui.label(RichText::new(&row.version).weak());
                    });
                    cells.col(|ui| {
                        if row.installed {
                            ui.label(
                                RichText::new("Installed").color(Color32::from_rgb(80, 190, 100)),
                            );
                        } else {
                            ui.label(RichText::new("Not installed").weak());
                        }
                    });
                    cells.col(|ui| {
                        if !row.installed {
                            let is_pr_template = row.channel.contains("{number}");
                            if is_pr_template {
                                if app.pr_prompt.as_deref() == Some(row.channel.as_str()) {
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut app.pr_number_input)
                                                .hint_text("PR #")
                                                .desired_width(60.0),
                                        );
                                        let valid = !app.busy
                                            && !app.pr_number_input.trim().is_empty()
                                            && app
                                                .pr_number_input
                                                .trim()
                                                .chars()
                                                .all(|c| c.is_ascii_digit());
                                        if ui
                                            .add_enabled(valid, egui::Button::new("Go").small())
                                            .clicked()
                                        {
                                            let ch = row
                                                .channel
                                                .replace("{number}", app.pr_number_input.trim());
                                            to_install = Some(ch);
                                            app.pr_prompt = None;
                                            app.pr_number_input.clear();
                                        }
                                        if ui.small_button("Cancel").clicked() {
                                            app.pr_prompt = None;
                                            app.pr_number_input.clear();
                                        }
                                    });
                                } else if ui
                                    .add_enabled(!app.busy, egui::Button::new("Install").small())
                                    .clicked()
                                {
                                    app.pr_prompt = Some(row.channel.clone());
                                    app.pr_number_input.clear();
                                }
                            } else if ui
                                .add_enabled(!app.busy, egui::Button::new("Install").small())
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
        app.send(Op::Add(ch));
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
                ui.label("Versions DB update interval (minutes):")
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
                ui.label("Auto-install channels:").on_hover_text(
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
                ui.label("Manifest version detect:").on_hover_text(
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
                    ui.label("Channel symlinks:")
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
                                .hint_text(default_terminal_hint())
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
                        .weak()
                        .size(11.0),
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
            .weak(),
        );
        ui.add_space(8.0);

        let overrides = app
            .state
            .as_ref()
            .map(|s| s.overrides.clone())
            .unwrap_or_default();
        if overrides.is_empty() {
            ui.label(RichText::new("No overrides configured.").weak());
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
                        if ui
                            .add_enabled(!app.busy, egui::Button::new("x Remove").small())
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
            .add_enabled(
                !app.busy,
                egui::Button::new("Remove non-existent paths").small(),
            )
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
                        .hint_text("/path/to/project")
                        .desired_width(300.0),
                );
                ui.end_row();
                ui.label("Julia channel:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.ov_channel)
                        .hint_text("e.g. 1.10")
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
                    .color(subtle_text(ui.visuals().dark_mode)),
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
                                egui::FontId::proportional(11.0),
                                Color32::WHITE,
                            )
                            .size()
                            .x
                        })
                        + 14.0; // button padding
                    let pad = ((ui.available_width() - est_width) / 2.0).max(0.0);
                    ui.add_space(pad);
                    ui.label(
                        RichText::new(version_text).size(12.0).weak(),
                    );
                    ui.add_space(6.0);
                    if ui
                        .add_enabled(
                            !app.busy,
                            egui::Button::new(
                                RichText::new(btn_text).size(11.0),
                            )
                            .small(),
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
            .weak()
            .size(11.5),
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

/// Shell-quote a string for POSIX shells (wraps in single quotes).
#[cfg(not(target_os = "windows"))]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build a shell command string for POSIX systems.
#[cfg(not(target_os = "windows"))]
fn build_launch_cmd(channel: &str, project: &str, extra_args: &str, env_vars: &str) -> String {
    let arg = format!("+{channel}");
    let mut parts: Vec<String> = Vec::new();
    if !project.trim().is_empty() {
        parts.push(format!("cd {} &&", shell_quote(project.trim())));
    }
    for tok in env_vars.split_whitespace() {
        if let Some((key, val)) = tok.split_once('=') {
            if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                parts.push(format!("{}={}", key, shell_quote(val)));
            }
        }
    }
    parts.push("julia".to_string());
    parts.push(shell_quote(&arg));
    if !project.trim().is_empty() {
        parts.push(format!("--project={}", shell_quote(project.trim())));
    }
    for tok in extra_args.split_whitespace() {
        parts.push(shell_quote(tok));
    }
    parts.join(" ")
}

/// Build a command line string for Windows cmd.exe.
/// Uses double-quote escaping appropriate for cmd.
#[cfg(target_os = "windows")]
fn win_quote(s: &str) -> String {
    if s.contains(' ')
        || s.contains('"')
        || s.contains('&')
        || s.contains('^')
        || s.contains('|')
        || s.contains('<')
        || s.contains('>')
        || s.contains('%')
    {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

#[cfg(target_os = "windows")]
fn build_launch_cmd(channel: &str, project: &str, extra_args: &str, env_vars: &str) -> String {
    let arg = format!("+{channel}");
    let mut parts: Vec<String> = Vec::new();
    if !project.trim().is_empty() {
        parts.push(format!("cd /d {} &", win_quote(project.trim())));
    }
    for tok in env_vars.split_whitespace() {
        if let Some((key, val)) = tok.split_once('=') {
            if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                parts.push(format!("set {}={} &", key, win_quote(val)));
            }
        }
    }
    parts.push("julia".to_string());
    parts.push(win_quote(&arg));
    if !project.trim().is_empty() {
        parts.push(format!("--project={}", win_quote(project.trim())));
    }
    for tok in extra_args.split_whitespace() {
        parts.push(win_quote(tok));
    }
    parts.join(" ")
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
) {
    let full_cmd = build_launch_cmd(channel, project, extra_args, env_vars);

    // Escape for embedding inside AppleScript double-quoted strings
    #[cfg(target_os = "macos")]
    let full_cmd_as = full_cmd.replace('\\', "\\\\").replace('"', "\\\"");

    if !terminal_app.trim().is_empty() {
        let term = terminal_app.trim();

        #[cfg(target_os = "macos")]
        {
            // Extract the app name from a bundle path or bare name
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
            let _ = std::process::Command::new("osascript")
                .args(["-e", &script])
                .spawn();
            return;
        }

        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new(term)
                .args(["cmd", "/k", &full_cmd])
                .spawn();
            return;
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = std::process::Command::new(term)
                .args(["-e", "sh", "-c", &full_cmd])
                .spawn();
            return;
        }
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Terminal\"\n  activate\n  do script \"{}\"\nend tell",
            full_cmd_as
        );
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .spawn();
    }

    #[cfg(target_os = "windows")]
    {
        // Prefer Windows Terminal (opens a new tab by default)
        let wt = std::process::Command::new("wt")
            .args(["cmd", "/k", &full_cmd])
            .spawn();
        if wt.is_err() {
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", "cmd", "/k", &full_cmd])
                .spawn();
        }
    }

    #[cfg(target_os = "linux")]
    {
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
            cmd.arg("sh").arg("-c").arg(&full_cmd);
            if cmd.spawn().is_ok() {
                return;
            }
        }
        let _ = std::process::Command::new("sh")
            .args(["-c", &full_cmd])
            .spawn();
    }
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
    let _ = std::fs::write(gui_prefs_path(paths), value.trim());
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
    let _ = std::fs::write(paths.juliauphome.join("juliaupgui_viewmode"), v);
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
    let _ = std::fs::write(paths.juliauphome.join("juliaupgui_theme"), v);
}

fn load_bool_pref(paths: &GlobalPaths, name: &str) -> bool {
    std::fs::read_to_string(paths.juliauphome.join(name))
        .unwrap_or_default()
        .trim()
        == "true"
}

fn save_bool_pref(paths: &GlobalPaths, name: &str, val: bool) {
    let _ = std::fs::write(
        paths.juliauphome.join(name),
        if val { "true" } else { "false" },
    );
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

fn muted_text(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(130, 135, 155)
    } else {
        Color32::from_rgb(100, 100, 115)
    }
}

fn subtle_text(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(150, 150, 170)
    } else {
        Color32::from_rgb(110, 110, 125)
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
    use std::io::BufRead;
    use std::process::Stdio;

    let bin = juliaup_binary()?;
    let mut child = std::process::Command::new(&bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
        Op::Add(ch) => format!("Installing '{ch}'…"),
        Op::Remove(ch) => format!("Removing '{ch}'…"),
        Op::Update(Some(ch)) => format!("Updating '{ch}'…"),
        Op::Update(None) => "Updating all channels…".into(),
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
        Op::Add(ch) => match spawn_and_stream(&["add", ch], tx) {
            Ok(_) => Msg::Ok(format!("Installed '{ch}'")),
            Err(e) => Msg::Err(format!("{e}")),
        },
        Op::Remove(ch) => match spawn_and_stream(&["remove", ch], tx) {
            Ok(_) => Msg::Ok(format!("Removed '{ch}'")),
            Err(e) => Msg::Err(format!("{e}")),
        },
        Op::Update(ch) => {
            let args: Vec<&str> = match ch {
                Some(c) => vec!["update", c.as_str()],
                None => vec!["update"],
            };
            match spawn_and_stream(&args, tx) {
                Ok(_) => Msg::Ok(match ch {
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
                save_config_db(&mut cfg)?;
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
            name: name.clone(),
        })
        .collect()
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

// Total animation: fly-out (0.5s) -> hold (0.2s) -> slide-to-header (0.5s)
const SPLASH_DURATION: f32 = 1.2;

fn ease_out_back(t: f32) -> f32 {
    let c1: f32 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn paint_splash(ctx: &egui::Context, t: f32, logo_rect: Option<egui::Rect>) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("splash"),
    ));

    let center = screen.center();

    // Phase durations
    let fly_dur = 0.5; // dots fly from center to logo formation
    let hold_dur = 0.2; // hold the center formation
    let slide_dur = 0.5; // slide from center to header position

    // Scrim: opaque at start, fades away once dots start sliding
    let scrim_start = fly_dur + hold_dur * 0.5;
    let scrim_fade_dur = slide_dur;
    let scrim_alpha = if t < scrim_start {
        200u8
    } else {
        let p = ((t - scrim_start) / scrim_fade_dur).clamp(0.0, 1.0);
        (200.0 * (1.0 - ease_out_cubic(p))) as u8
    };
    if scrim_alpha > 0 {
        let (sr, sg, sb) = scrim_base(ctx.style().visuals.dark_mode);
        painter.rect_filled(
            screen,
            0.0,
            Color32::from_rgba_unmultiplied(sr, sg, sb, scrim_alpha),
        );
    }

    // Large logo in center of screen
    let big_size: f32 = 120.0;
    let big_s = big_size / 350.0;
    let big_origin = center - egui::Vec2::new(175.0 * big_s, 175.0 * big_s);

    // Small logo target in header (fall back to a sensible top-left default)
    let header_rect = logo_rect.unwrap_or(egui::Rect::from_min_size(
        egui::Pos2::new(10.0, 8.0),
        egui::Vec2::splat(26.0),
    ));
    let small_size = header_rect.width();
    let small_s = small_size / 350.0;
    let small_origin = header_rect.min;

    // Slide progress: 0 = center formation, 1 = header position
    let slide_start = fly_dur + hold_dur;
    let slide_raw = ((t - slide_start) / slide_dur).clamp(0.0, 1.0);
    let slide_p = ease_out_cubic(slide_raw);

    for (i, &(cx, cy, dot_r, red, green, blue)) in JULIA_DOTS.iter().enumerate() {
        let delay = i as f32 * 0.08;
        let dt = (t - delay).max(0.0);

        // Big-formation target
        let big_target = big_origin + egui::Vec2::new(cx * big_s, cy * big_s);
        let big_r = dot_r * big_s;

        // Small-formation target
        let small_target = small_origin + egui::Vec2::new(cx * small_s, cy * small_s);
        let small_r = dot_r * small_s;

        // Phase 1: fly from screen center to big formation
        let fly_progress = (dt / fly_dur).clamp(0.0, 1.0);
        let eased = ease_out_back(fly_progress);
        let fly_pos = egui::Pos2::new(
            center.x + (big_target.x - center.x) * eased,
            center.y + (big_target.y - center.y) * eased,
        );
        let fly_r = big_r * (0.3 + 0.7 * eased);

        // Phase 2: slide from big formation to header
        let pos = egui::Pos2::new(
            fly_pos.x + (small_target.x - big_target.x) * slide_p,
            fly_pos.y + (small_target.y - big_target.y) * slide_p,
        );
        let radius = fly_r + (small_r - big_r) * slide_p;

        painter.circle_filled(pos, radius, Color32::from_rgb(red, green, blue));
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

fn resolve_dark(mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => {
            #[cfg(target_os = "macos")]
            {
                // AppleInterfaceStyle is only set when dark mode is active
                std::process::Command::new("defaults")
                    .args(["read", "-g", "AppleInterfaceStyle"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(true)
            }
            #[cfg(not(target_os = "macos"))]
            {
                true
            }
        }
    }
}

fn apply_theme(ctx: &egui::Context, mode: ThemeMode) {
    let dark = resolve_dark(mode);

    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    if dark {
        visuals.panel_fill = Color32::from_rgb(28, 30, 36);
        visuals.window_fill = Color32::from_rgb(28, 30, 36);
        visuals.faint_bg_color = Color32::from_rgb(34, 36, 44);
    } else {
        visuals.panel_fill = Color32::from_rgb(248, 249, 252);
        visuals.window_fill = Color32::WHITE;
        visuals.faint_bg_color = Color32::from_rgb(238, 240, 245);
    }

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
    visuals.selection.stroke = egui::Stroke::new(
        1.0,
        if dark {
            Color32::WHITE
        } else {
            Color32::from_gray(20)
        },
    );

    // Ensure text on the purple selection background is always legible
    visuals.widgets.active.fg_stroke = egui::Stroke::new(
        1.0,
        if dark {
            Color32::WHITE
        } else {
            Color32::from_gray(20)
        },
    );
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(
        1.0,
        if dark {
            Color32::WHITE
        } else {
            Color32::from_gray(30)
        },
    );

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
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
    style.spacing.window_margin = egui::Margin::same(8.0);
    ctx.set_style(style);
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

    // ── shell_quote / build_launch_cmd (POSIX) ───────────────────────────

    #[cfg(not(target_os = "windows"))]
    mod posix {
        use super::super::*;

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
            let cmd = build_launch_cmd("release", "", "", "");
            assert_eq!(cmd, "julia '+release'");
        }

        #[test]
        fn build_cmd_with_project() {
            let cmd = build_launch_cmd("1.10", "/tmp/my project", "", "");
            assert_eq!(
                cmd,
                "cd '/tmp/my project' && julia '+1.10' --project='/tmp/my project'"
            );
        }

        #[test]
        fn build_cmd_with_env_vars() {
            let cmd = build_launch_cmd("release", "", "", "JULIA_NUM_THREADS=4");
            assert_eq!(cmd, "JULIA_NUM_THREADS='4' julia '+release'");
        }

        #[test]
        fn build_cmd_skips_invalid_env_key() {
            let cmd = build_launch_cmd("release", "", "", "BAD-KEY=val GOOD_KEY=ok");
            assert_eq!(cmd, "GOOD_KEY='ok' julia '+release'");
        }

        #[test]
        fn build_cmd_with_extra_args() {
            let cmd = build_launch_cmd("release", "", "--threads=4 -q", "");
            assert_eq!(cmd, "julia '+release' '--threads=4' '-q'");
        }

        #[test]
        fn build_cmd_full() {
            let cmd = build_launch_cmd(
                "1.10",
                "/home/user/proj",
                "-q --startup-file=no",
                "JULIA_NUM_THREADS=auto",
            );
            assert!(cmd.starts_with("cd '/home/user/proj' && JULIA_NUM_THREADS='auto' julia"));
            assert!(cmd.contains("--project='/home/user/proj'"));
            assert!(cmd.contains("'-q'"));
            assert!(cmd.contains("'--startup-file=no'"));
        }
    }

    // ── win_quote / build_launch_cmd (Windows) ───────────────────────────

    #[cfg(target_os = "windows")]
    mod windows {
        use super::super::*;

        #[test]
        fn win_quote_simple() {
            assert_eq!(win_quote("hello"), "hello");
        }

        #[test]
        fn win_quote_with_spaces() {
            assert_eq!(win_quote("hello world"), "\"hello world\"");
        }

        #[test]
        fn win_quote_with_ampersand() {
            assert_eq!(win_quote("a&b"), "\"a&b\"");
        }

        #[test]
        fn build_cmd_basic_channel() {
            let cmd = build_launch_cmd("release", "", "", "");
            assert_eq!(cmd, "julia +release");
        }

        #[test]
        fn build_cmd_with_project() {
            let cmd = build_launch_cmd("1.10", "C:\\my project", "", "");
            assert!(cmd.starts_with("cd /d \"C:\\my project\" &"));
            assert!(cmd.contains("julia"));
            assert!(cmd.contains("--project=\"C:\\my project\""));
        }

        #[test]
        fn build_cmd_with_env_vars() {
            let cmd = build_launch_cmd("release", "", "", "JULIA_NUM_THREADS=4");
            assert_eq!(cmd, "set JULIA_NUM_THREADS=4 & julia +release");
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
