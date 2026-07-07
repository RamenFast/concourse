// SPDX-License-Identifier: GPL-3.0-or-later
//! The hall itself. One window where the whole estate is visible and
//! reachable: the departures board (live nodes), the bays (districts of
//! apps with buttons), the filing cabinet, the skills rack, the asks
//! ledger, the doctor. Same core as the CLI — this is just the human render.

pub mod theme;
mod views;

use crate::probe::{self, Live, State};
use crate::registry::{self, Registry};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 840.0])
            .with_min_inner_size([780.0, 540.0])
            .with_app_id("concourse")
            .with_title("Concourse — the station's shared hall"),
        ..Default::default()
    };
    eframe::run_native(
        "concourse",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .map_err(|e| e.to_string())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct UiConfig {
    theme: String,
    reduce_motion: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        UiConfig { theme: "blossom_dark".into(), reduce_motion: false }
    }
}

fn ui_config_path() -> std::path::PathBuf {
    registry::config_dir().join("ui.json")
}

fn load_ui_config() -> UiConfig {
    std::fs::read_to_string(ui_config_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_ui_config(c: &UiConfig) {
    if let Some(dir) = ui_config_path().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(t) = serde_json::to_string_pretty(c) {
        let _ = std::fs::write(ui_config_path(), t);
    }
}

pub struct CabinetDoc {
    pub path: String,
    pub readonly: bool,
    pub sha12: String,
    pub mtime: String,
    pub text: String,
}

pub struct App {
    pub reg: Result<Registry, (String, String)>,
    pub pal: theme::Palette,
    pub reduce_motion: bool,

    pub lives: Arc<Mutex<HashMap<String, Live>>>,
    pub inflight: Arc<AtomicUsize>,
    pub last_refresh: Option<Instant>,
    pub prev_states: HashMap<String, State>,
    pub flash: HashMap<String, Instant>,

    pub inspectors: Vec<String>, // node-id stack (Esc closes the newest)
    pub show_cabinet: bool,
    pub show_skills: bool,
    pub show_asks: bool,
    pub show_doctor: bool,

    pub cabinet: Option<CabinetDoc>,
    pub skills: Option<crate::skills::SkillsReport>,
    pub asks: Vec<crate::asks::Ask>,
    pub ask_input: String,
    pub ask_by: String,
    pub doctor: Arc<Mutex<Option<crate::doctor::Report>>>,
    pub doctor_running: Arc<AtomicUsize>,
    pub toast: Option<(String, Instant)>,
}

/// Widen glyph coverage from the system's own fonts (loaded from disk, not
/// embedded — the binary stays lean and absence degrades gracefully to the
/// built-in subset).
fn install_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        // monochrome NotoEmoji (google/fonts, installed by concourse's founding
        // session) — ab_glyph can rasterize it, unlike the CBDT color font
        ("noto-emoji", "/usr/local/share/fonts/NotoEmoji-Regular.ttf"),
        ("noto-symbols2", "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf"),
    ];
    let mut added: Vec<String> = Vec::new();
    for (name, path) in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_string(), egui::FontData::from_owned(bytes).into());
            added.push(name.to_string());
        }
    }
    if added.is_empty() {
        return;
    }
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            for name in &added {
                list.push(name.clone());
            }
        }
    }
    ctx.set_fonts(fonts);
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let cfg = load_ui_config();
        install_system_fonts(&cc.egui_ctx);
        let pal = theme::palette(&cfg.theme);
        pal.apply(&cc.egui_ctx);
        let mut app = App {
            reg: registry::load(),
            pal,
            reduce_motion: cfg.reduce_motion,
            lives: Arc::new(Mutex::new(HashMap::new())),
            inflight: Arc::new(AtomicUsize::new(0)),
            last_refresh: None,
            prev_states: HashMap::new(),
            flash: HashMap::new(),
            inspectors: Vec::new(),
            show_cabinet: false,
            show_skills: false,
            show_asks: false,
            show_doctor: false,
            cabinet: None,
            skills: None,
            asks: Vec::new(),
            ask_input: String::new(),
            ask_by: "ben".into(),
            doctor: Arc::new(Mutex::new(None)),
            doctor_running: Arc::new(AtomicUsize::new(0)),
            toast: None,
        };
        app.refresh(&cc.egui_ctx);
        app
    }

    pub fn save_theme(&self) {
        save_ui_config(&UiConfig { theme: self.pal.id.to_string(), reduce_motion: self.reduce_motion });
    }

    /// Probe every node on background threads; the board breathes as
    /// results land (no barrier, no frozen UI).
    pub fn refresh(&mut self, ctx: &egui::Context) {
        let Ok(reg) = &self.reg else { return };
        self.last_refresh = Some(Instant::now());
        for node in reg.nodes.iter().cloned() {
            let lives = Arc::clone(&self.lives);
            let inflight = Arc::clone(&self.inflight);
            let ctx = ctx.clone();
            inflight.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let live = probe::probe(&node);
                if let Ok(mut map) = lives.lock() {
                    map.insert(node.id.clone(), live);
                }
                inflight.fetch_sub(1, Ordering::SeqCst);
                ctx.request_repaint();
            });
        }
    }

    pub fn run_doctor(&mut self, ctx: &egui::Context) {
        let Ok(reg) = &self.reg else { return };
        if self.doctor_running.load(Ordering::SeqCst) > 0 {
            return;
        }
        let reg = reg.clone();
        let slot = Arc::clone(&self.doctor);
        let running = Arc::clone(&self.doctor_running);
        let ctx = ctx.clone();
        running.fetch_add(1, Ordering::SeqCst);
        std::thread::spawn(move || {
            let report = crate::doctor::run(&reg);
            if let Ok(mut s) = slot.lock() {
                *s = Some(report);
            }
            running.fetch_sub(1, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }

    pub fn load_cabinet(&mut self) {
        let path = crate::util::home().join("AGENTS.md");
        let bytes = std::fs::read(&path).unwrap_or_default();
        use sha2::{Digest, Sha256};
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&path).ok();
        self.cabinet = Some(CabinetDoc {
            path: path.to_string_lossy().into_owned(),
            readonly: meta
                .as_ref()
                .map(|m| m.permissions().mode() & 0o222 == 0)
                .unwrap_or(false),
            sha12: format!("{:x}", Sha256::digest(&bytes))[..12].to_string(),
            mtime: meta
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    chrono::DateTime::<chrono::Local>::from(t)
                        .format("%Y-%m-%d %H:%M")
                        .to_string()
                })
                .unwrap_or_default(),
            text: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }

    pub fn note(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    /// Track state changes so rows can flash gently when something moves.
    fn update_flash(&mut self) {
        let lives = self.lives.lock().map(|m| m.clone()).unwrap_or_default();
        for (id, live) in lives.iter() {
            match self.prev_states.get(id) {
                Some(prev) if *prev != live.state => {
                    self.flash.insert(id.clone(), Instant::now());
                }
                None => {} // first sighting isn't a change
                _ => {}
            }
            self.prev_states.insert(id.clone(), live.state);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_flash();

        // the clock ticks, the board breathes
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        // auto re-probe (stale is graceful, but fresh is better)
        if let Some(t) = self.last_refresh {
            if t.elapsed().as_secs() > 120 && self.inflight.load(Ordering::SeqCst) == 0 {
                self.refresh(ctx);
            }
        }

        // Esc: close the newest thing first (the Escape cascade)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if let Some(_) = self.inspectors.pop() {
            } else if self.show_doctor {
                self.show_doctor = false;
            } else if self.show_asks {
                self.show_asks = false;
            } else if self.show_skills {
                self.show_skills = false;
            } else if self.show_cabinet {
                self.show_cabinet = false;
            }
        }

        views::top_bar(self, ctx);
        views::status_strip(self, ctx);
        views::central(self, ctx);
        views::windows(self, ctx);
    }
}
