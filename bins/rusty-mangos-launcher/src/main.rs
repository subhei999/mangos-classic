use eframe::egui::{
    self, Align, Button, CentralPanel, Color32, Context, CornerRadius, Frame, Layout, Margin,
    RichText, ScrollArea, SidePanel, Stroke, TextEdit, TopBottomPanel, Ui, Vec2, Visuals,
};
use eframe::{App, CreationContext, NativeOptions};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

fn main() -> eframe::Result {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title("Rusty MaNGOS Launcher"),
        ..Default::default()
    };

    eframe::run_native(
        "Rusty MaNGOS Launcher",
        options,
        Box::new(|cc| Ok(Box::new(LauncherApp::new(cc)))),
    )
}

#[derive(Clone)]
struct LauncherPaths {
    root: PathBuf,
    script: PathBuf,
    settings: PathBuf,
    app_data: PathBuf,
}

impl LauncherPaths {
    fn discover() -> Result<Self, String> {
        let mut current = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| "Could not resolve launcher location.".to_string())?;

        loop {
            let script = current.join("scripts").join("rusty-mangos-launcher.ps1");
            if script.is_file() {
                let app_data = current.join("target").join("launcher");
                return Ok(Self {
                    root: current,
                    script,
                    settings: app_data.join("rusty-mangos.settings.json"),
                    app_data,
                });
            }

            if !current.pop() {
                return Err("Could not find scripts\\rusty-mangos-launcher.ps1 next to this launcher or in a parent folder.".to_string());
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LauncherSettings {
    #[serde(default = "default_database_mode")]
    database_mode: String,
    #[serde(default)]
    client_dir: String,
    #[serde(default)]
    classic_db_path: String,
    #[serde(default = "default_db_port")]
    db_port: u16,
    #[serde(default = "default_world_port")]
    world_port: u16,
    #[serde(default = "default_auth_port")]
    auth_port: u16,
    #[serde(default)]
    debug_build: bool,
    #[serde(default = "default_mariadb_version")]
    maria_db_version: String,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            database_mode: default_database_mode(),
            client_dir: String::new(),
            classic_db_path: String::new(),
            db_port: default_db_port(),
            world_port: default_world_port(),
            auth_port: default_auth_port(),
            debug_build: false,
            maria_db_version: default_mariadb_version(),
        }
    }
}

impl LauncherSettings {
    fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }
}

fn default_database_mode() -> String {
    "Native".to_string()
}

fn default_mariadb_version() -> String {
    "11.4.8".to_string()
}

fn default_db_port() -> u16 {
    3307
}

fn default_auth_port() -> u16 {
    13724
}

fn default_world_port() -> u16 {
    18085
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Setup,
    Logs,
    Advanced,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OperationState {
    Idle,
    Running,
}

#[derive(Default, Clone, Copy)]
struct PortStatus {
    db: bool,
    auth: bool,
    world: bool,
}

impl PortStatus {
    fn label(self) -> &'static str {
        if self.auth && self.world {
            "Online"
        } else if self.db || self.auth || self.world {
            "Partial"
        } else {
            "Offline"
        }
    }

    fn color(self) -> Color32 {
        if self.auth && self.world {
            Color32::from_rgb(32, 196, 130)
        } else if self.db || self.auth || self.world {
            Color32::from_rgb(239, 178, 72)
        } else {
            Color32::from_rgb(122, 132, 148)
        }
    }
}

struct OperationOutput {
    receiver: Receiver<ProcessEvent>,
}

enum ProcessEvent {
    Line(String),
    Finished(i32),
}

struct LauncherApp {
    paths: Result<LauncherPaths, String>,
    settings: LauncherSettings,
    page: Page,
    state: OperationState,
    output: Option<OperationOutput>,
    log: String,
    last_status_check: Instant,
    ports: PortStatus,
    skip_world_import: bool,
    force_world_import: bool,
    no_realmlist_update: bool,
    status_message: String,
}

impl LauncherApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        install_style(&cc.egui_ctx);

        let paths = LauncherPaths::discover();
        let settings = paths
            .as_ref()
            .map(|paths| LauncherSettings::load(&paths.settings))
            .unwrap_or_default();
        let mut log = String::new();
        match &paths {
            Ok(paths) => {
                log.push_str(&format!("Launcher root: {}\n", paths.root.display()));
            }
            Err(error) => {
                log.push_str(error);
                log.push('\n');
            }
        }

        Self {
            paths,
            settings,
            page: Page::Home,
            state: OperationState::Idle,
            output: None,
            log,
            last_status_check: Instant::now() - Duration::from_secs(10),
            ports: PortStatus::default(),
            skip_world_import: false,
            force_world_import: false,
            no_realmlist_update: false,
            status_message: "Ready".to_string(),
        }
    }

    fn run_action(&mut self, action: &str) {
        if self.state == OperationState::Running {
            self.append_log("An operation is already running.\n");
            return;
        }

        let Ok(paths) = self.paths.clone() else {
            self.append_log("Launcher paths are not available.\n");
            return;
        };

        if matches!(action, "InstallStart" | "Install" | "Configure")
            && self.settings.client_dir.trim().is_empty()
        {
            self.append_log("Select your World of Warcraft 1.12.1 client folder first.\n");
            self.page = Page::Setup;
            return;
        }

        let args = self.build_args(&paths, action);
        let (tx, rx) = mpsc::channel();
        self.output = Some(OperationOutput { receiver: rx });
        self.state = OperationState::Running;
        self.status_message = format!("{action} running");
        self.append_log(&format!("> rusty-mangos-launcher {action}\n"));

        std::thread::spawn(move || {
            let mut child = match Command::new("powershell.exe")
                .args(args)
                .current_dir(&paths.root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(error) => {
                    let _ = tx.send(ProcessEvent::Line(format!(
                        "Failed to start PowerShell: {error}\n"
                    )));
                    let _ = tx.send(ProcessEvent::Finished(1));
                    return;
                }
            };

            if let Some(stdout) = child.stdout.take() {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        let _ = tx.send(ProcessEvent::Line(format!("{line}\n")));
                    }
                });
            }

            if let Some(stderr) = child.stderr.take() {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines().map_while(Result::ok) {
                        let _ = tx.send(ProcessEvent::Line(format!("{line}\n")));
                    }
                });
            }

            let code = child
                .wait()
                .ok()
                .and_then(|status| status.code())
                .unwrap_or(1);
            let _ = tx.send(ProcessEvent::Finished(code));
        });
    }

    fn build_args(&self, paths: &LauncherPaths, action: &str) -> Vec<String> {
        let mut args = vec![
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
            paths.script.display().to_string(),
            action.to_string(),
            "-DbPort".to_string(),
            self.settings.db_port.to_string(),
            "-AuthPort".to_string(),
            self.settings.auth_port.to_string(),
            "-WorldPort".to_string(),
            self.settings.world_port.to_string(),
        ];

        if !self.settings.client_dir.trim().is_empty() {
            args.push("-ClientDir".to_string());
            args.push(self.settings.client_dir.trim().to_string());
        }
        if !self.settings.classic_db_path.trim().is_empty() {
            args.push("-ClassicDbPath".to_string());
            args.push(self.settings.classic_db_path.trim().to_string());
        }
        if self.skip_world_import {
            args.push("-SkipWorldImport".to_string());
        }
        if self.force_world_import {
            args.push("-ForceWorldImport".to_string());
        }
        if self.no_realmlist_update {
            args.push("-NoRealmlistUpdate".to_string());
        }

        args
    }

    fn poll_output(&mut self, ctx: &Context) {
        if let Some(output) = self.output.take() {
            let mut finished = None;
            while let Ok(event) = output.receiver.try_recv() {
                match event {
                    ProcessEvent::Line(line) => self.append_log(&line),
                    ProcessEvent::Finished(code) => finished = Some(code),
                }
            }

            if let Some(code) = finished {
                self.append_log(&format!("> exited with code {code}\n"));
                self.state = OperationState::Idle;
                self.status_message = if code == 0 {
                    "Ready".to_string()
                } else {
                    format!("Last command failed ({code})")
                };
                if let Ok(paths) = &self.paths {
                    self.settings = LauncherSettings::load(&paths.settings);
                }
                self.refresh_status();
            } else {
                self.output = Some(output);
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
    }

    fn refresh_status(&mut self) {
        self.ports = PortStatus {
            db: is_port_open(self.settings.db_port),
            auth: is_port_open(self.settings.auth_port),
            world: is_port_open(self.settings.world_port),
        };
        self.last_status_check = Instant::now();
    }

    fn append_log(&mut self, text: &str) {
        self.log.push_str(text);
        if self.log.len() > 160_000 {
            let keep_from = self.log.len().saturating_sub(120_000);
            self.log = self.log[keep_from..].to_string();
        }
    }

    fn open_client_picker(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Select World of Warcraft 1.12.1 folder")
            .pick_folder()
        {
            self.settings.client_dir = path.display().to_string();
        }
    }

    fn open_app_data(&mut self) {
        if let Ok(paths) = &self.paths {
            let _ = fs::create_dir_all(&paths.app_data);
            open_path(&paths.app_data);
        }
    }

    fn open_dashboard(&mut self) {
        open_url("http://127.0.0.1:9091/dashboard");
    }

    fn draw_sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.label(
            RichText::new("RUSTY")
                .size(13.0)
                .color(Color32::from_rgb(132, 162, 210)),
        );
        ui.label(
            RichText::new("MaNGOS")
                .size(29.0)
                .strong()
                .color(Color32::from_rgb(235, 241, 250)),
        );
        ui.add_space(26.0);

        nav_button(ui, &mut self.page, Page::Home, "SERVER");
        nav_button(ui, &mut self.page, Page::Setup, "SETUP");
        nav_button(ui, &mut self.page, Page::Logs, "LOGS");
        nav_button(ui, &mut self.page, Page::Advanced, "ADVANCED");

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.label(
                RichText::new("RUSTAUTH / RUSTPASS")
                    .size(11.0)
                    .color(muted()),
            );
            ui.label(
                RichText::new(self.ports.label())
                    .color(self.ports.color())
                    .strong(),
            );
        });
    }

    fn draw_topbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.status_message.clone()).color(muted()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(enabled_button(
                        self.state == OperationState::Idle,
                        "Refresh",
                    ))
                    .clicked()
                {
                    self.refresh_status();
                }
                ui.label(
                    RichText::new(format!("World {}", self.settings.world_port)).color(muted()),
                );
                ui.label(RichText::new(format!("Auth {}", self.settings.auth_port)).color(muted()));
            });
        });
    }

    fn draw_home(&mut self, ui: &mut Ui) {
        ui.vertical_centered_justified(|ui| {
            Frame::new()
                .fill(Color32::from_rgb(24, 31, 42))
                .corner_radius(CornerRadius::same(10))
                .stroke(Stroke::new(1.0, Color32::from_rgb(54, 70, 94)))
                .inner_margin(Margin::same(24))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("Northshire local realm")
                                    .size(28.0)
                                    .strong()
                                    .color(Color32::from_rgb(244, 248, 255)),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(
                                    "Run a local Rusty MaNGOS server for WoW 1.12.1 playtesting.",
                                )
                                .size(15.0)
                                .color(muted()),
                            );
                            ui.add_space(22.0);
                            ui.horizontal(|ui| {
                                status_pill(ui, "Database", self.ports.db);
                                status_pill(ui, "Auth", self.ports.auth);
                                status_pill(ui, "World", self.ports.world);
                            });
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.vertical(|ui| {
                                let install_text = if self.ports.auth && self.ports.world {
                                    "RESTART"
                                } else {
                                    "INSTALL / START"
                                };
                                if ui
                                    .add(primary_button(
                                        self.state == OperationState::Idle,
                                        install_text,
                                        Vec2::new(220.0, 48.0),
                                    ))
                                    .clicked()
                                {
                                    let action = if self.ports.auth && self.ports.world {
                                        "Restart"
                                    } else {
                                        "InstallStart"
                                    };
                                    self.run_action(action);
                                }
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(enabled_button(
                                            self.state == OperationState::Idle,
                                            "Start",
                                        ))
                                        .clicked()
                                    {
                                        self.run_action("Start");
                                    }
                                    if ui
                                        .add(enabled_button(
                                            self.state == OperationState::Idle,
                                            "Stop",
                                        ))
                                        .clicked()
                                    {
                                        self.run_action("Stop");
                                    }
                                });
                            });
                        });
                    });
                });
        });

        ui.add_space(18.0);
        ui.columns(2, |columns| {
            panel(&mut columns[0], "Client", |ui| {
                if self.settings.client_dir.trim().is_empty() {
                    ui.label(
                        RichText::new("No WoW client selected")
                            .color(Color32::from_rgb(239, 178, 72)),
                    );
                } else {
                    ui.label(
                        RichText::new(self.settings.client_dir.clone())
                            .color(Color32::from_rgb(220, 229, 242)),
                    );
                }
                ui.add_space(10.0);
                if ui.button("Choose Folder").clicked() {
                    self.open_client_picker();
                }
            });

            panel(&mut columns[1], "Quick Access", |ui| {
                if ui.button("Open Dashboard").clicked() {
                    self.open_dashboard();
                }
                if ui.button("Open Launcher Data").clicked() {
                    self.open_app_data();
                }
                if ui.button("Show Status").clicked() {
                    self.run_action("Status");
                    self.page = Page::Logs;
                }
            });
        });
    }

    fn draw_setup(&mut self, ui: &mut Ui) {
        panel(ui, "First Run Setup", |ui| {
            ui.label(RichText::new("World of Warcraft 1.12.1 client folder").color(muted()));
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.settings.client_dir)
                        .desired_width(f32::INFINITY),
                );
                if ui.button("Browse").clicked() {
                    self.open_client_picker();
                }
            });
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui
                    .add(primary_button(
                        self.state == OperationState::Idle,
                        "SAVE SETUP",
                        Vec2::new(160.0, 38.0),
                    ))
                    .clicked()
                {
                    self.run_action("Configure");
                }
                if ui
                    .add(enabled_button(
                        self.state == OperationState::Idle,
                        "Install without starting",
                    ))
                    .clicked()
                {
                    self.run_action("Install");
                }
            });
        });
    }

    fn draw_logs(&mut self, ui: &mut Ui) {
        panel(ui, "Launcher Log", |ui| {
            ui.horizontal(|ui| {
                if ui.button("Clear").clicked() {
                    self.log.clear();
                }
                if ui.button("Status").clicked() {
                    self.run_action("Status");
                }
            });
            ui.add_space(8.0);
            ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.log)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(24)
                            .interactive(false),
                    );
                });
        });
    }

    fn draw_advanced(&mut self, ui: &mut Ui) {
        panel(ui, "Advanced", |ui| {
            ui.horizontal(|ui| {
                port_field(ui, "DB", &mut self.settings.db_port);
                port_field(ui, "Auth", &mut self.settings.auth_port);
                port_field(ui, "World", &mut self.settings.world_port);
            });
            ui.add_space(12.0);
            ui.checkbox(&mut self.skip_world_import, "Skip world import");
            ui.checkbox(&mut self.force_world_import, "Force world re-import");
            ui.checkbox(&mut self.no_realmlist_update, "Do not update realmlist");
            ui.add_space(12.0);
            ui.label(RichText::new("ClassicDB path").color(muted()));
            ui.add(
                TextEdit::singleline(&mut self.settings.classic_db_path)
                    .desired_width(f32::INFINITY),
            );
        });
    }
}

impl App for LauncherApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_output(ctx);
        if self.state == OperationState::Idle
            && self.last_status_check.elapsed() >= Duration::from_secs(3)
        {
            self.refresh_status();
        }

        SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(190.0)
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(13, 18, 26))
                    .inner_margin(Margin::same(18)),
            )
            .show(ctx, |ui| self.draw_sidebar(ui));

        TopBottomPanel::top("topbar")
            .resizable(false)
            .exact_height(48.0)
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(18, 24, 34))
                    .inner_margin(Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| self.draw_topbar(ui));

        CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(18, 24, 34))
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| match self.page {
                Page::Home => self.draw_home(ui),
                Page::Setup => self.draw_setup(ui),
                Page::Logs => self.draw_logs(ui),
                Page::Advanced => self.draw_advanced(ui),
            });
    }
}

fn install_style(ctx: &Context) {
    let mut visuals = Visuals::dark();
    visuals.window_fill = Color32::from_rgb(18, 24, 34);
    visuals.panel_fill = Color32::from_rgb(18, 24, 34);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(32, 42, 56);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(45, 62, 84);
    visuals.widgets.active.bg_fill = Color32::from_rgb(17, 128, 222);
    visuals.selection.bg_fill = Color32::from_rgb(22, 116, 210);
    ctx.set_visuals(visuals);
}

fn nav_button(ui: &mut Ui, page: &mut Page, target: Page, text: &str) {
    let selected = *page == target;
    let fill = if selected {
        Color32::from_rgb(24, 82, 145)
    } else {
        Color32::TRANSPARENT
    };
    let button = Button::new(RichText::new(text).size(14.0).strong())
        .fill(fill)
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(150.0, 38.0));
    if ui.add(button).clicked() {
        *page = target;
    }
    ui.add_space(6.0);
}

fn panel(ui: &mut Ui, title: &str, content: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(Color32::from_rgb(24, 31, 42))
        .corner_radius(CornerRadius::same(8))
        .stroke(Stroke::new(1.0, Color32::from_rgb(46, 60, 82)))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .size(18.0)
                    .strong()
                    .color(Color32::from_rgb(235, 241, 250)),
            );
            ui.add_space(12.0);
            content(ui);
        });
}

fn primary_button(enabled: bool, text: &str, size: Vec2) -> Button<'_> {
    Button::new(RichText::new(text).strong().color(Color32::WHITE))
        .fill(if enabled {
            Color32::from_rgb(0, 112, 221)
        } else {
            Color32::from_rgb(54, 65, 78)
        })
        .corner_radius(CornerRadius::same(5))
        .min_size(size)
}

fn enabled_button(enabled: bool, text: &str) -> Button<'_> {
    Button::new(RichText::new(text).color(if enabled {
        Color32::from_rgb(230, 238, 250)
    } else {
        Color32::from_rgb(120, 128, 140)
    }))
    .fill(Color32::from_rgb(34, 45, 60))
    .corner_radius(CornerRadius::same(5))
}

fn status_pill(ui: &mut Ui, label: &str, online: bool) {
    let color = if online {
        Color32::from_rgb(32, 196, 130)
    } else {
        Color32::from_rgb(122, 132, 148)
    };
    Frame::new()
        .fill(Color32::from_rgb(31, 40, 54))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{label} {}",
                    if online { "ONLINE" } else { "OFFLINE" }
                ))
                .color(color)
                .size(12.0),
            );
        });
}

fn port_field(ui: &mut Ui, label: &str, value: &mut u16) {
    let mut text = value.to_string();
    ui.vertical(|ui| {
        ui.label(RichText::new(label).color(muted()));
        if ui
            .add(TextEdit::singleline(&mut text).desired_width(82.0))
            .changed()
        {
            if let Ok(parsed) = text.parse::<u16>() {
                *value = parsed;
            }
        }
    });
}

fn muted() -> Color32 {
    Color32::from_rgb(157, 170, 190)
}

fn is_port_open(port: u16) -> bool {
    let Ok(mut addrs) = ("127.0.0.1", port).to_socket_addrs() else {
        return false;
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(180)).is_ok()
}

fn open_path(path: &Path) {
    let _ = Command::new("explorer.exe").arg(path).spawn();
}

fn open_url(url: &str) {
    let _ = Command::new("cmd.exe")
        .args(["/C", "start", "", url])
        .spawn();
}
