//! `mug-gui` — a minimal native launcher for the `mug` static site generator.
//!
//! It "opens" a project directory (read-only — it never edits `config.yaml`) and
//! runs the same `mug::{build, serve, clean, new}` operations the CLI does,
//! streaming their progress into a log pane. The intended audience is people who
//! would rather not touch a terminal.
//!
//! Long-running work runs on worker threads; each thread reports
//! [`Progress`](mug::report::Progress) through a channel-backed [`ChannelReporter`]
//! that wakes the UI via `egui::Context::request_repaint`.

use eframe::egui;
use mug::report::{Progress, Reporter, ServeHandle};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

const PORT: u16 = 3000;

/// [`Reporter`] that forwards events to the UI thread and requests a repaint.
struct ChannelReporter {
    tx: Sender<Progress>,
    ctx: egui::Context,
}

impl Reporter for ChannelReporter {
    fn report(&self, progress: Progress) {
        // If the UI has gone away the send fails; nothing useful to do but drop it.
        let _ = self.tx.send(progress);
        self.ctx.request_repaint();
    }
}

#[derive(PartialEq, Eq)]
enum Mode {
    Idle,
    Building,
    Serving,
}

struct App {
    ctx: egui::Context,
    tx: Sender<Progress>,
    rx: Receiver<Progress>,
    project: Option<PathBuf>,
    mode: Mode,
    log: String,
    serve_handle: Option<ServeHandle>,
    serve_addr: Option<SocketAddr>,
    // "New site" form state.
    show_new: bool,
    new_location: Option<PathBuf>,
    new_name: String,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = channel();
        Self {
            ctx: cc.egui_ctx.clone(),
            tx,
            rx,
            project: None,
            mode: Mode::Idle,
            log: String::new(),
            serve_handle: None,
            serve_addr: None,
            show_new: false,
            new_location: None,
            new_name: String::new(),
        }
    }

    fn reporter(&self) -> Arc<ChannelReporter> {
        Arc::new(ChannelReporter {
            tx: self.tx.clone(),
            ctx: self.ctx.clone(),
        })
    }

    fn push(&mut self, line: impl AsRef<str>) {
        self.log.push_str(line.as_ref());
        self.log.push('\n');
    }

    /// Apply a progress event from a worker thread to UI state.
    fn handle_progress(&mut self, progress: Progress) {
        match progress {
            Progress::Info(msg) => self.push(msg),
            Progress::BuildStarted => self.push("Building…"),
            Progress::BuildOk { pages, elapsed } => {
                self.push(format!("✓ Built {pages} pages in {elapsed:?}"));
                // Only a one-shot Build returns us to Idle; a serve-driven rebuild
                // must keep the Serving state.
                if self.mode == Mode::Building {
                    self.mode = Mode::Idle;
                }
            }
            Progress::BuildErr(msg) => {
                self.push(format!("✗ Build failed: {msg}"));
                if self.mode == Mode::Building {
                    self.mode = Mode::Idle;
                }
            }
            Progress::ServeReady(addr) => {
                self.serve_addr = Some(addr);
                let url = format!("http://{addr}");
                self.push(format!("Serving at {url}"));
                if let Err(e) = opener::open_browser(&url) {
                    self.push(format!("(couldn't open browser: {e})"));
                }
            }
            Progress::ServeStopped => {
                self.mode = Mode::Idle;
                self.serve_addr = None;
                self.serve_handle = None;
                self.push("Server stopped");
            }
        }
    }

    fn start_build(&mut self) {
        let Some(root) = self.project.clone() else {
            return;
        };
        let reporter = self.reporter();
        self.mode = Mode::Building;
        self.push("Building…");
        std::thread::spawn(move || {
            let start = Instant::now();
            match mug::build(&root, false) {
                Ok(report) => reporter.report(Progress::BuildOk {
                    pages: report.pages,
                    elapsed: start.elapsed(),
                }),
                Err(e) => reporter.report(Progress::BuildErr(format!("{e:#}"))),
            }
        });
    }

    fn start_serve(&mut self) {
        let Some(root) = self.project.clone() else {
            return;
        };
        let reporter = self.reporter();
        let handle = ServeHandle::new();
        self.serve_handle = Some(handle.clone());
        self.mode = Mode::Serving;
        self.push("Starting server…");
        let host = IpAddr::V4(Ipv4Addr::LOCALHOST);
        std::thread::spawn(move || {
            if let Err(e) = mug::serve(&root, host, PORT, reporter.clone(), handle) {
                reporter.report(Progress::BuildErr(format!("serve failed: {e:#}")));
            }
            // Whether serve exited cleanly (via stop) or errored, tell the UI so it
            // resets out of the Serving state.
            reporter.report(Progress::ServeStopped);
        });
    }

    fn stop_serve(&mut self) {
        if let Some(handle) = &self.serve_handle {
            handle.stop();
            self.push("Stopping server…");
        }
        // State resets when the worker reports `ServeStopped`.
    }

    fn start_clean(&mut self) {
        let Some(root) = self.project.clone() else {
            return;
        };
        let reporter = self.reporter();
        std::thread::spawn(move || match mug::clean(&root) {
            Ok(Some(dir)) => reporter.report(Progress::Info(format!("Cleaned {}", dir.display()))),
            Ok(None) => reporter.report(Progress::Info("Nothing to clean".into())),
            Err(e) => reporter.report(Progress::Info(format!("✗ Clean failed: {e:#}"))),
        });
    }

    fn create_new_site(&mut self) {
        let Some(location) = self.new_location.clone() else {
            return;
        };
        let name = self.new_name.trim();
        if name.is_empty() {
            return;
        }
        let target = location.join(name);
        match mug::new(&target) {
            Ok(()) => {
                self.push(format!("Created new site at {}", target.display()));
                self.project = Some(target);
                self.show_new = false;
                self.new_name.clear();
            }
            Err(e) => self.push(format!("✗ Couldn't create site: {e:#}")),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Ok(progress) = self.rx.try_recv() {
            self.handle_progress(progress);
        }

        let idle = self.mode == Mode::Idle;
        let serving = self.mode == Mode::Serving;
        let has_project = self.project.is_some();

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("mug");
            ui.label(match &self.project {
                Some(p) => format!("Project: {}", p.display()),
                None => "No project open.".to_string(),
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(idle, egui::Button::new("Open…"))
                    .clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder()
                {
                    self.push(format!("Opened {}", dir.display()));
                    self.project = Some(dir);
                }
                if ui
                    .add_enabled(idle, egui::Button::new("New site…"))
                    .clicked()
                {
                    self.show_new = !self.show_new;
                }
            });

            if self.show_new {
                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.label("Create a new starter site");
                    ui.horizontal(|ui| {
                        if ui.button("Choose location…").clicked()
                            && let Some(dir) = rfd::FileDialog::new().pick_folder()
                        {
                            self.new_location = Some(dir);
                        }
                        ui.label(match &self.new_location {
                            Some(p) => p.display().to_string(),
                            None => "(no location chosen)".to_string(),
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.new_name);
                    });
                    let can_create =
                        self.new_location.is_some() && !self.new_name.trim().is_empty();
                    ui.horizontal(|ui| {
                        if ui.add_enabled(can_create, egui::Button::new("Create")).clicked() {
                            self.create_new_site();
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_new = false;
                        }
                    });
                });
            }

            ui.separator();

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(has_project && idle, egui::Button::new("Build"))
                    .clicked()
                {
                    self.start_build();
                }
                if serving {
                    if ui.button("Stop").clicked() {
                        self.stop_serve();
                    }
                } else if ui
                    .add_enabled(has_project && idle, egui::Button::new("Serve"))
                    .clicked()
                {
                    self.start_serve();
                }
                if ui
                    .add_enabled(has_project && idle, egui::Button::new("Clean"))
                    .clicked()
                {
                    self.start_clean();
                }
            });

            if let Some(addr) = self.serve_addr {
                ui.add_space(2.0);
                ui.hyperlink(format!("http://{addr}"));
            }

            ui.separator();
            ui.label("Log");
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.monospace(&self.log);
                });
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([560.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "mug",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
