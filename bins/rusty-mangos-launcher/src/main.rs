#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{
    self, Align, Button, CentralPanel, Color32, Context, CornerRadius, Frame, Layout, Margin,
    ProgressBar, RichText, ScrollArea, SidePanel, Stroke, TextEdit, TopBottomPanel, Ui, Vec2,
    Visuals,
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

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
    build_info: PathBuf,
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
                let build_info = current.join("BUILD_INFO.txt");
                return Ok(Self {
                    root: current,
                    script,
                    settings: app_data.join("rusty-mangos.settings.json"),
                    app_data,
                    build_info,
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
    #[serde(default)]
    data_dir: String,
    #[serde(default = "default_mmap_maps")]
    mmap_maps: String,
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
            data_dir: String::new(),
            mmap_maps: default_mmap_maps(),
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

fn default_mmap_maps() -> String {
    "0 1".to_string()
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
    Health,
    Logs,
    Repair,
    Updates,
    Advanced,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogView {
    Launcher,
    Auth,
    World,
    MariaDb,
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

#[derive(Clone)]
struct OperationProgress {
    action: String,
    phase: String,
    detail: String,
    index: usize,
    total: usize,
    started: Instant,
}

impl OperationProgress {
    fn new(action: &str) -> Self {
        Self {
            action: action.to_string(),
            phase: "Starting".to_string(),
            detail: "Preparing launcher command".to_string(),
            index: 0,
            total: progress_total_for_action(action),
            started: Instant::now(),
        }
    }

    fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.index as f32 / self.total as f32).clamp(0.04, 0.98)
    }
}

#[derive(Default, Clone)]
struct HealthSnapshot {
    client_ok: bool,
    maps_ok: bool,
    vmaps_ok: bool,
    mmaps_ok: bool,
    mariadb_data_ok: bool,
    realmlist_ok: bool,
    build_id: String,
    data_dir: PathBuf,
    checked_at: String,
}

#[derive(Clone)]
struct UpdateSnapshot {
    local_build: String,
    release_tag: String,
    release_commit: String,
    published_at: String,
    setup_url: String,
    setup_size: String,
    app_zip_url: String,
    app_zip_size: String,
    release_url: String,
    available: Option<bool>,
    last_download: String,
}

impl UpdateSnapshot {
    fn new(local_build: String) -> Self {
        Self {
            local_build,
            release_tag: "Not checked".to_string(),
            release_commit: String::new(),
            published_at: String::new(),
            setup_url: String::new(),
            setup_size: String::new(),
            app_zip_url: String::new(),
            app_zip_size: String::new(),
            release_url: String::new(),
            available: None,
            last_download: String::new(),
        }
    }

    fn status_label(&self) -> &'static str {
        match self.available {
            Some(true) => "Update available",
            Some(false) => "Current",
            None => "Not checked",
        }
    }

    fn status_color(&self) -> Color32 {
        match self.available {
            Some(true) => Color32::from_rgb(239, 178, 72),
            Some(false) => Color32::from_rgb(32, 196, 130),
            None => muted(),
        }
    }
}

struct LauncherApp {
    paths: Result<LauncherPaths, String>,
    settings: LauncherSettings,
    page: Page,
    state: OperationState,
    output: Option<OperationOutput>,
    progress: Option<OperationProgress>,
    log: String,
    server_log: String,
    selected_log: LogView,
    last_log_file_refresh: Instant,
    realm_label: String,
    last_status_check: Instant,
    ports: PortStatus,
    health: HealthSnapshot,
    update: UpdateSnapshot,
    last_health_refresh: Instant,
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
        let build_id = paths
            .as_ref()
            .map(read_build_id)
            .unwrap_or_else(|_| "local".to_string());
        let realm_label = format!("Pre-alpha Test Realm {build_id}");
        let mut settings = settings;
        let autodetected_client = if settings.client_dir.trim().is_empty() {
            discover_wow_client_dir().map(|path| path.display().to_string())
        } else {
            None
        };
        if let Some(path) = &autodetected_client {
            settings.client_dir = path.clone();
        }
        let mut log = String::new();
        match &paths {
            Ok(paths) => {
                log.push_str(&format!("Launcher root: {}\n", paths.root.display()));
                log.push_str(&format!("Realm: {realm_label}\n"));
                if let Some(path) = &autodetected_client {
                    log.push_str(&format!("Auto-detected WoW client: {path}\n"));
                }
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
            progress: None,
            log,
            server_log: String::new(),
            selected_log: LogView::Launcher,
            last_log_file_refresh: Instant::now() - Duration::from_secs(10),
            realm_label,
            last_status_check: Instant::now() - Duration::from_secs(10),
            ports: PortStatus::default(),
            health: HealthSnapshot::default(),
            update: UpdateSnapshot::new(build_id),
            last_health_refresh: Instant::now() - Duration::from_secs(10),
            skip_world_import: false,
            force_world_import: false,
            no_realmlist_update: false,
            status_message: "Ready".to_string(),
        }
    }

    fn run_action(&mut self, action: &str) {
        self.run_action_with_update_asset(action, None);
    }

    fn run_action_with_update_asset(&mut self, action: &str, update_asset: Option<&str>) {
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

        let args = self.build_args(&paths, action, update_asset);
        let (tx, rx) = mpsc::channel();
        self.output = Some(OperationOutput { receiver: rx });
        self.state = OperationState::Running;
        self.progress = Some(OperationProgress::new(action));
        self.status_message = format!("{action} running");
        self.append_log(&format!("> rusty-mangos-launcher {action}\n"));

        std::thread::spawn(move || {
            let mut command = Command::new("powershell.exe");
            command
                .args(args)
                .current_dir(&paths.root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            hide_process_window(&mut command);

            let mut child = match command.spawn() {
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

    fn build_args(
        &self,
        paths: &LauncherPaths,
        action: &str,
        update_asset: Option<&str>,
    ) -> Vec<String> {
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
        if !self.settings.mmap_maps.trim().is_empty() {
            args.push("-MMapMaps".to_string());
            args.push(self.settings.mmap_maps.trim().to_string());
        }
        if let Some(update_asset) = update_asset {
            args.push("-UpdateAsset".to_string());
            args.push(update_asset.to_string());
        }

        args
    }

    fn poll_output(&mut self, ctx: &Context) {
        if let Some(output) = self.output.take() {
            let mut finished = None;
            while let Ok(event) = output.receiver.try_recv() {
                match event {
                    ProcessEvent::Line(line) => {
                        self.update_progress_from_line(&line);
                        self.update_release_from_line(&line);
                        self.append_log(&line);
                    }
                    ProcessEvent::Finished(code) => finished = Some(code),
                }
            }

            if let Some(code) = finished {
                self.append_log(&format!("> exited with code {code}\n"));
                self.state = OperationState::Idle;
                if let Some(progress) = &mut self.progress {
                    progress.index = progress.total;
                    progress.detail = if code == 0 {
                        "Completed".to_string()
                    } else {
                        format!("Failed with exit code {code}")
                    };
                }
                self.status_message = if code == 0 {
                    "Ready".to_string()
                } else {
                    format!("Last command failed ({code})")
                };
                if let Ok(paths) = &self.paths {
                    self.settings = LauncherSettings::load(&paths.settings);
                }
                self.refresh_status();
                self.refresh_health();
            } else {
                self.output = Some(output);
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
    }

    fn update_progress_from_line(&mut self, line: &str) {
        let Some(progress) = &mut self.progress else {
            return;
        };
        let clean = line.trim();
        if clean.is_empty() {
            return;
        }

        progress.detail = clean.to_string();
        if let Some(phase) = clean.strip_prefix("==> ") {
            progress.phase = phase.to_string();
            progress.index = progress_index_for_phase(&progress.action, phase);
            return;
        }

        for (needle, phase) in [
            ("Extracting server dbc/maps", "Extracting maps"),
            ("Extracting server vmaps", "Extracting vmaps"),
            ("Assembling server vmaps", "Assembling vmaps"),
            ("Generating server mmaps", "Generating mmaps"),
            ("Checking launcher MariaDB tables", "Repairing databases"),
            ("Importing ClassicDB", "Importing ClassicDB"),
            ("World database already has content", "World database ready"),
            ("Authserver is listening", "Starting servers"),
            ("Worldserver is listening", "Starting servers"),
            ("Rusty MaNGOS is ready", "Ready"),
        ] {
            if clean.contains(needle) {
                progress.phase = phase.to_string();
                progress.index = progress_index_for_phase(&progress.action, phase);
                break;
            }
        }
    }

    fn update_release_from_line(&mut self, line: &str) {
        let clean = line.trim();
        let Some((key, value)) = clean.split_once('=') else {
            return;
        };
        if !key.starts_with("UPDATE_") {
            return;
        }

        match key {
            "UPDATE_LOCAL_BUILD" => self.update.local_build = value.to_string(),
            "UPDATE_RELEASE_TAG" => self.update.release_tag = value.to_string(),
            "UPDATE_RELEASE_COMMIT" => self.update.release_commit = value.to_string(),
            "UPDATE_PUBLISHED_AT" => self.update.published_at = value.to_string(),
            "UPDATE_SETUP_URL" => self.update.setup_url = value.to_string(),
            "UPDATE_SETUP_SIZE" => self.update.setup_size = value.to_string(),
            "UPDATE_APP_URL" => self.update.app_zip_url = value.to_string(),
            "UPDATE_APP_SIZE" => self.update.app_zip_size = value.to_string(),
            "UPDATE_RELEASE_URL" => self.update.release_url = value.to_string(),
            "UPDATE_DOWNLOAD_PATH" => self.update.last_download = value.to_string(),
            "UPDATE_AVAILABLE" => {
                self.update.available = match value {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                };
            }
            _ => {}
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

    fn refresh_health(&mut self) {
        self.health = build_health_snapshot(&self.paths, &self.settings, self.ports);
        self.last_health_refresh = Instant::now();
    }

    fn refresh_file_log(&mut self) {
        if let Ok(paths) = &self.paths {
            let log_dir = paths.app_data.join("logs");
            self.server_log = match self.selected_log {
                LogView::Launcher => self.log.clone(),
                LogView::Auth => read_log_group(
                    "Authserver",
                    &[
                        log_dir.join("authserver.log"),
                        log_dir.join("authserver.err.log"),
                    ],
                ),
                LogView::World => read_log_group(
                    "Worldserver",
                    &[
                        log_dir.join("worldserver.log"),
                        log_dir.join("worldserver.err.log"),
                    ],
                ),
                LogView::MariaDb => read_log_group(
                    "MariaDB",
                    &[log_dir.join("mariadb.log"), log_dir.join("mariadb.err.log")],
                ),
            };
        }
        self.last_log_file_refresh = Instant::now();
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

    fn auto_detect_client(&mut self) {
        if let Some(path) = discover_wow_client_dir() {
            self.settings.client_dir = path.display().to_string();
            self.status_message = "WoW client detected".to_string();
            self.append_log(&format!(
                "Auto-detected WoW client: {}\n",
                self.settings.client_dir
            ));
        } else {
            self.status_message = "No WoW client found".to_string();
            self.append_log("Could not auto-detect a WoW client folder.\n");
        }
    }

    fn launch_client(&mut self) {
        let client_dir = PathBuf::from(self.settings.client_dir.trim());
        let wow_exe = client_dir.join("WoW.exe");
        if !wow_exe.is_file() {
            self.status_message = "WoW.exe not found".to_string();
            self.append_log("Cannot launch client: select a folder containing WoW.exe first.\n");
            self.page = Page::Setup;
            return;
        }

        match Command::new(&wow_exe).current_dir(&client_dir).spawn() {
            Ok(_) => {
                self.status_message = "WoW client launched".to_string();
                self.append_log(&format!("Launched {}\n", wow_exe.display()));
            }
            Err(error) => {
                self.status_message = "Client launch failed".to_string();
                self.append_log(&format!(
                    "Failed to launch {}: {error}\n",
                    wow_exe.display()
                ));
            }
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
        nav_button(ui, &mut self.page, Page::Health, "HEALTH");
        nav_button(ui, &mut self.page, Page::Logs, "LOGS");
        nav_button(ui, &mut self.page, Page::Repair, "REPAIR");
        nav_button(ui, &mut self.page, Page::Updates, "UPDATES");
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
                                RichText::new(self.realm_label.clone())
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

        self.draw_progress(ui);

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
                ui.horizontal(|ui| {
                    if ui.button("Choose Folder").clicked() {
                        self.open_client_picker();
                    }
                    if ui.button("Auto-detect").clicked() {
                        self.auto_detect_client();
                    }
                    if ui
                        .add(enabled_button(
                            !self.settings.client_dir.trim().is_empty(),
                            "Launch WoW",
                        ))
                        .clicked()
                    {
                        self.launch_client();
                    }
                });
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
                if ui.button("Auto-detect").clicked() {
                    self.auto_detect_client();
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

    fn draw_progress(&mut self, ui: &mut Ui) {
        let Some(progress) = &self.progress else {
            return;
        };

        ui.add_space(14.0);
        panel(ui, "Progress", |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(progress.phase.clone())
                        .strong()
                        .color(Color32::from_rgb(235, 241, 250)),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{}s", progress.started.elapsed().as_secs()))
                            .color(muted()),
                    );
                });
            });
            ui.add_space(8.0);
            ui.add(
                ProgressBar::new(progress.fraction())
                    .desired_width(f32::INFINITY)
                    .show_percentage(),
            );
            ui.add_space(8.0);
            ui.label(RichText::new(progress.detail.clone()).color(muted()));
        });
    }

    fn draw_health(&mut self, ui: &mut Ui) {
        panel(ui, "Server Health", |ui| {
            ui.horizontal(|ui| {
                if ui.button("Refresh").clicked() {
                    self.refresh_status();
                    self.refresh_health();
                }
                ui.label(
                    RichText::new(format!("Last check: {}", self.health.checked_at)).color(muted()),
                );
            });
            ui.add_space(12.0);
            ui.columns(2, |columns| {
                health_row(&mut columns[0], "WoW client", self.health.client_ok);
                health_row(&mut columns[0], "DBC and maps", self.health.maps_ok);
                health_row(&mut columns[0], "VMaps", self.health.vmaps_ok);
                health_row(&mut columns[0], "MMaps", self.health.mmaps_ok);
                health_row(&mut columns[1], "MariaDB data", self.health.mariadb_data_ok);
                health_row(&mut columns[1], "MariaDB port", self.ports.db);
                health_row(&mut columns[1], "Auth port", self.ports.auth);
                health_row(&mut columns[1], "World port", self.ports.world);
                health_row(
                    &mut columns[1],
                    "Client realmlist",
                    self.health.realmlist_ok,
                );
            });
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("Data: {}", self.health.data_dir.display())).color(muted()),
            );
            ui.label(RichText::new(format!("Build: {}", self.health.build_id)).color(muted()));
        });
    }

    fn draw_logs(&mut self, ui: &mut Ui) {
        panel(ui, "Logs", |ui| {
            ui.horizontal(|ui| {
                log_tab(ui, &mut self.selected_log, LogView::Launcher, "Launcher");
                log_tab(ui, &mut self.selected_log, LogView::Auth, "Auth");
                log_tab(ui, &mut self.selected_log, LogView::World, "World");
                log_tab(ui, &mut self.selected_log, LogView::MariaDb, "MariaDB");
                ui.separator();
                if ui.button("Refresh").clicked() {
                    self.refresh_file_log();
                }
                if self.selected_log == LogView::Launcher && ui.button("Clear").clicked() {
                    self.log.clear();
                }
                if ui.button("Status").clicked() {
                    self.run_action("Status");
                }
            });
            ui.add_space(8.0);
            let rows = 24;
            if self.selected_log == LogView::Launcher {
                ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            TextEdit::multiline(&mut self.log)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(rows)
                                .interactive(false),
                        );
                    });
            } else {
                ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            TextEdit::multiline(&mut self.server_log)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(rows)
                                .interactive(false),
                        );
                    });
            }
        });
    }

    fn draw_repair(&mut self, ui: &mut Ui) {
        panel(ui, "Repair", |ui| {
            ui.label(
                RichText::new(
                    "Use these when a smoke test leaves part of the local stack in a bad state.",
                )
                .color(muted()),
            );
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(enabled_button(
                        self.state == OperationState::Idle,
                        "Repair Database",
                    ))
                    .clicked()
                {
                    self.run_action("RepairDatabase");
                }
                if ui
                    .add(enabled_button(
                        self.state == OperationState::Idle,
                        "Re-extract VMaps",
                    ))
                    .clicked()
                {
                    self.run_action("ReextractVMaps");
                }
                if ui
                    .add(enabled_button(
                        self.state == OperationState::Idle,
                        "Rebuild MMaps",
                    ))
                    .clicked()
                {
                    self.run_action("RebuildMMaps");
                }
                if ui
                    .add(enabled_button(
                        self.state == OperationState::Idle,
                        "Reset Seeded Characters",
                    ))
                    .clicked()
                {
                    self.run_action("ResetSeededCharacters");
                }
            });
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(14.0);
            ui.label(RichText::new("World database").color(muted()));
            ui.horizontal(|ui| {
                if ui
                    .add(enabled_button(
                        self.state == OperationState::Idle,
                        "Reimport ClassicDB World",
                    ))
                    .clicked()
                {
                    self.run_action("ReimportWorld");
                }
                ui.checkbox(&mut self.no_realmlist_update, "Preserve realmlist");
            });
        });
    }

    fn draw_updates(&mut self, ui: &mut Ui) {
        panel(ui, "Updates", |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.update.status_label())
                        .strong()
                        .color(self.update.status_color()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(enabled_button(
                            self.state == OperationState::Idle,
                            "Check Updates",
                        ))
                        .clicked()
                    {
                        self.run_action("CheckUpdates");
                    }
                });
            });
            ui.add_space(12.0);
            ui.columns(2, |columns| {
                columns[0].label(RichText::new("Installed").color(muted()));
                columns[0].label(
                    RichText::new(self.update.local_build.clone())
                        .strong()
                        .color(Color32::from_rgb(235, 241, 250)),
                );
                columns[1].label(RichText::new("Release").color(muted()));
                columns[1].label(
                    RichText::new(if self.update.release_commit.is_empty() {
                        self.update.release_tag.clone()
                    } else {
                        format!(
                            "{} {}",
                            self.update.release_tag,
                            short_commit(&self.update.release_commit)
                        )
                    })
                    .strong()
                    .color(Color32::from_rgb(235, 241, 250)),
                );
            });
            if !self.update.published_at.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("Published: {}", self.update.published_at))
                        .color(muted()),
                );
            }
            if !self.update.last_download.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("Downloaded: {}", self.update.last_download))
                        .color(Color32::from_rgb(32, 196, 130)),
                );
            }
            ui.add_space(16.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(enabled_button(
                        self.state == OperationState::Idle && !self.update.app_zip_url.is_empty(),
                        "Download App Zip",
                    ))
                    .clicked()
                {
                    self.run_action_with_update_asset("DownloadUpdate", Some("AppZip"));
                }
                if ui
                    .add(enabled_button(
                        self.state == OperationState::Idle && !self.update.setup_url.is_empty(),
                        "Download Installer",
                    ))
                    .clicked()
                {
                    self.run_action_with_update_asset("DownloadUpdate", Some("Installer"));
                }
                if ui
                    .add(enabled_button(
                        !self.update.release_url.is_empty(),
                        "Open Release",
                    ))
                    .clicked()
                {
                    open_url(&self.update.release_url);
                }
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(asset_label("App zip", &self.update.app_zip_size)).color(muted()),
            );
            ui.label(
                RichText::new(asset_label("Installer", &self.update.setup_size)).color(muted()),
            );
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
            self.refresh_health();
        }
        if self.last_health_refresh.elapsed() >= Duration::from_secs(5) {
            self.refresh_health();
        }
        if self.page == Page::Logs && self.last_log_file_refresh.elapsed() >= Duration::from_secs(1)
        {
            self.refresh_file_log();
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
                Page::Health => self.draw_health(ui),
                Page::Logs => self.draw_logs(ui),
                Page::Repair => self.draw_repair(ui),
                Page::Updates => self.draw_updates(ui),
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

fn log_tab(ui: &mut Ui, selected: &mut LogView, target: LogView, text: &str) {
    let is_selected = *selected == target;
    if ui
        .add(
            Button::new(RichText::new(text).strong())
                .fill(if is_selected {
                    Color32::from_rgb(24, 82, 145)
                } else {
                    Color32::from_rgb(34, 45, 60)
                })
                .corner_radius(CornerRadius::same(5)),
        )
        .clicked()
    {
        *selected = target;
    }
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

fn health_row(ui: &mut Ui, label: &str, ok: bool) {
    let color = if ok {
        Color32::from_rgb(32, 196, 130)
    } else {
        Color32::from_rgb(239, 178, 72)
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(if ok { "OK" } else { "CHECK" })
                .strong()
                .color(color),
        );
        ui.label(RichText::new(label).color(Color32::from_rgb(220, 229, 242)));
    });
}

fn progress_total_for_action(action: &str) -> usize {
    match action {
        "InstallStart" => 8,
        "Install" => 7,
        "Start" | "Restart" => 4,
        "RepairDatabase" => 3,
        "ReextractVMaps" | "RebuildMMaps" => 3,
        "ReimportWorld" => 3,
        "ResetSeededCharacters" => 3,
        "CheckUpdates" => 2,
        "DownloadUpdate" => 3,
        _ => 2,
    }
}

fn progress_index_for_phase(action: &str, phase: &str) -> usize {
    let normalized = phase.to_ascii_lowercase();
    let index = if normalized.contains("configuring") {
        1
    } else if normalized.contains("client data")
        || normalized.contains("maps")
        || normalized.contains("vmaps")
        || normalized.contains("mmaps")
    {
        2
    } else if normalized.contains("mariadb") {
        3
    } else if normalized.contains("database")
        || normalized.contains("classicdb")
        || normalized.contains("seeded")
    {
        4
    } else if normalized.contains("checking launcher updates") {
        1
    } else if normalized.contains("downloading launcher update") {
        2
    } else if normalized.contains("starting") {
        6
    } else if normalized.contains("ready") {
        progress_total_for_action(action)
    } else {
        1
    };
    index.min(progress_total_for_action(action).saturating_sub(1))
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

fn short_commit(commit: &str) -> String {
    let trimmed = commit.trim();
    if trimmed.len() >= 8 {
        trimmed[..8].to_string()
    } else {
        trimmed.to_string()
    }
}

fn asset_label(name: &str, size: &str) -> String {
    if size.trim().is_empty() {
        format!("{name}: not checked")
    } else {
        format!("{name}: {size}")
    }
}

fn read_build_id(paths: &LauncherPaths) -> String {
    if let Ok(text) = fs::read_to_string(&paths.build_info) {
        for line in text.lines() {
            if let Some(commit) = line.strip_prefix("Source commit:") {
                let commit = commit.trim();
                if commit.len() >= 8 {
                    return commit[..8].to_string();
                }
            }
        }
    }

    "local".to_string()
}

fn build_health_snapshot(
    paths: &Result<LauncherPaths, String>,
    settings: &LauncherSettings,
    _ports: PortStatus,
) -> HealthSnapshot {
    let build_id = paths
        .as_ref()
        .map(read_build_id)
        .unwrap_or_else(|_| "local".to_string());
    let data_dir = paths
        .as_ref()
        .map(|paths| server_data_dir(paths, settings))
        .unwrap_or_default();
    let client_path = PathBuf::from(settings.client_dir.trim());
    let client_ok = is_wow_client_dir(&client_path);
    let maps_ok =
        data_dir.join("dbc").is_dir() && has_files_with_extension(&data_dir.join("maps"), "map");
    let vmaps_ok = has_files_with_extension(&data_dir.join("vmaps"), "vmtree")
        && has_files_with_extension(&data_dir.join("vmaps"), "vmtile");
    let mmaps_ok = mmap_maps_ok(&data_dir, &settings.mmap_maps);
    let mariadb_data_ok = paths
        .as_ref()
        .map(|paths| paths.app_data.join("mariadb-data").join("mysql").is_dir())
        .unwrap_or(false);
    let realmlist_ok = client_ok && realmlist_points_to_auth(&client_path, settings.auth_port);

    HealthSnapshot {
        client_ok,
        maps_ok,
        vmaps_ok,
        mmaps_ok,
        mariadb_data_ok,
        realmlist_ok,
        build_id,
        data_dir,
        checked_at: "now".to_string(),
    }
}

fn server_data_dir(paths: &LauncherPaths, settings: &LauncherSettings) -> PathBuf {
    if settings.data_dir.trim().is_empty() {
        paths.app_data.join("data")
    } else {
        PathBuf::from(settings.data_dir.trim())
    }
}

fn has_files_with_extension(path: &Path, extension: &str) -> bool {
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(extension))
    })
}

fn mmap_maps_ok(data_dir: &Path, map_ids: &str) -> bool {
    let mmap_dir = data_dir.join("mmaps");
    let ids: Vec<_> = map_ids
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|value| !value.trim().is_empty())
        .collect();
    if ids.is_empty() {
        return has_files_with_extension(&mmap_dir, "mmap")
            && has_files_with_extension(&mmap_dir, "mmtile");
    }

    ids.into_iter().all(|id| {
        let Ok(map_id) = id.parse::<u32>() else {
            return false;
        };
        let prefix = format!("{map_id:03}");
        mmap_dir.join(format!("{prefix}.mmap")).is_file()
            && has_file_with_prefix(&mmap_dir, &prefix, "mmtile")
    })
}

fn has_file_with_prefix(path: &Path, prefix: &str, extension: &str) -> bool {
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        path.file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with(prefix))
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(extension))
    })
}

fn realmlist_points_to_auth(client_path: &Path, auth_port: u16) -> bool {
    realmlist_paths(client_path).into_iter().any(|path| {
        fs::read_to_string(path).is_ok_and(|text| {
            let expected = format!("127.0.0.1:{auth_port}");
            text.to_ascii_lowercase().contains("set realmlist") && text.contains(&expected)
        })
    })
}

fn realmlist_paths(client_path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![client_path.join("realmlist.wtf")];
    let data_root = client_path.join("Data");
    if let Ok(entries) = fs::read_dir(data_root) {
        paths.extend(entries.flatten().filter_map(|entry| {
            let path = entry.path().join("realmlist.wtf");
            path.is_file().then_some(path)
        }));
    }
    paths
}

fn is_wow_client_dir(path: &Path) -> bool {
    path.join("WoW.exe").is_file() && path.join("Data").is_dir()
}

fn discover_wow_client_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for key in [
        "RUSTY_MANGOS_WOW_DIR",
        "WOW_CLIENT_DIR",
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
        "USERPROFILE",
    ] {
        if let Some(value) = std::env::var_os(key) {
            candidates.push(PathBuf::from(value));
        }
    }

    for base in candidates.clone() {
        candidates.extend([
            base.join("World of Warcraft"),
            base.join("World of Warcraft 1.12.1"),
            base.join("WoW"),
            base.join("Vanilla WoW"),
            base.join("Games").join("World of Warcraft"),
            base.join("Downloads").join("World of Warcraft"),
            base.join("Desktop").join("World of Warcraft"),
            base.join("Documents").join("World of Warcraft"),
        ]);
    }

    candidates.extend([
        PathBuf::from(r"C:\Games\World of Warcraft"),
        PathBuf::from(r"C:\Games\WoW"),
        PathBuf::from(r"C:\WoW"),
        PathBuf::from(r"C:\Vanilla WoW"),
        PathBuf::from(r"C:\World of Warcraft"),
    ]);

    for candidate in &candidates {
        if is_wow_client_dir(candidate) {
            return Some(candidate.clone());
        }
    }

    for root in candidates {
        if let Some(found) = find_wow_client_under(&root, 3, &mut 0) {
            return Some(found);
        }
    }

    None
}

fn find_wow_client_under(path: &Path, depth: usize, visited: &mut usize) -> Option<PathBuf> {
    if *visited > 4_000 || depth == 0 || !path.is_dir() {
        return None;
    }
    *visited += 1;

    if is_wow_client_dir(path) {
        return Some(path.to_path_buf());
    }

    let entries = fs::read_dir(path).ok()?;
    for entry in entries.flatten() {
        let child = entry.path();
        if child.is_dir() {
            if let Some(found) = find_wow_client_under(&child, depth - 1, visited) {
                return Some(found);
            }
        }
    }
    None
}

fn read_log_group(title: &str, paths: &[PathBuf]) -> String {
    let mut output = format!("{title} logs\n");
    for path in paths {
        output.push_str("\n== ");
        output.push_str(&path.display().to_string());
        output.push_str(" ==\n");
        let text = read_text_tail(path, 120_000);
        if text.trim().is_empty() {
            output.push_str("(no log output yet)\n");
        } else {
            let cleaned = clean_log_text(&text);
            output.push_str(&cleaned);
            if !cleaned.ends_with('\n') {
                output.push('\n');
            }
        }
    }
    output
}

fn read_text_tail(path: &Path, max_bytes: usize) -> String {
    let Ok(bytes) = fs::read(path) else {
        return String::new();
    };
    let start = bytes.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&bytes[start..]).to_string()
}

fn clean_log_text(text: &str) -> String {
    let mut output = String::new();
    let mut vmap_load_count = 0usize;
    for raw_line in text.lines() {
        let line = strip_ansi(raw_line);
        if line.contains("VMapManager2: loading file") {
            vmap_load_count += 1;
            continue;
        }
        if vmap_load_count > 0 {
            output.push_str(&format!(
                "(collapsed {vmap_load_count} VMap model load lines)\n"
            ));
            vmap_load_count = 0;
        }
        output.push_str(&trim_long_log_line(&line));
        output.push('\n');
    }
    if vmap_load_count > 0 {
        output.push_str(&format!(
            "(collapsed {vmap_load_count} VMap model load lines)\n"
        ));
    }
    output
}

fn trim_long_log_line(line: &str) -> String {
    const MAX_CHARS: usize = 1_200;
    if line.chars().count() <= MAX_CHARS {
        return line.to_string();
    }
    let mut trimmed: String = line.chars().take(MAX_CHARS).collect();
    trimmed.push_str(" ... [line trimmed]");
    trimmed
}

fn strip_ansi(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
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
    let mut command = Command::new("explorer.exe");
    command.arg(path);
    hide_process_window(&mut command);
    let _ = command.spawn();
}

fn open_url(url: &str) {
    let mut command = Command::new("cmd.exe");
    command.args(["/C", "start", "", url]);
    hide_process_window(&mut command);
    let _ = command.spawn();
}

fn hide_process_window(command: &mut Command) {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
}
