// SPDX-License-Identifier: GPL-3.0-or-later
//! The hall's rooms: top bar, departures board, the bays, the windows
//! (inspector popouts, filing cabinet, skills rack, asks ledger, doctor).
//! House rules live here: sharp corners, hairline frames, mono for data,
//! carved stone only for the few controls that matter, popouts persist
//! until dismissed.

use super::theme::{status_color, Palette};
use super::App;
use crate::probe::{Live, State};
use crate::registry::{Node, NodeKind, ProbeSpec};
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke};
use std::sync::atomic::Ordering;
use std::sync::Arc;

// ── small kit ───────────────────────────────────────────────────────────────

fn mono(s: impl Into<String>, size: f32, c: Color32) -> RichText {
    RichText::new(s.into()).font(FontId::monospace(size)).color(c)
}

fn take(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// A carved stone control — depth for the few actions that matter.
pub(super) fn stone_button(ui: &mut egui::Ui, pal: &Palette, label: &str, size: egui::Vec2, lit: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let pressed = resp.is_pointer_button_down_on();
    let mix = ui.ctx().animate_bool_with_time(resp.id.with("lit"), lit || resp.hovered(), 0.15);
    pal.carve_with_face(ui.painter(), rect, pressed, mix * 0.9);
    let ink = pal.ink;
    ui.painter().text(
        rect.center() + if pressed { egui::vec2(0.5, 0.5) } else { egui::Vec2::ZERO },
        Align2::CENTER_CENTER,
        label,
        FontId::monospace(12.5),
        ink,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn lamp_glyph(state: State) -> &'static str {
    match state {
        State::Ok => "●",
        State::Present => "◆",
        State::Unavailable => "◐",
        State::Error => "✗",
        State::Missing => "▢",
        State::Unprobed => "·",
    }
}

impl App {
    fn live_for(&self, id: &str) -> Option<Live> {
        self.lives.lock().ok().and_then(|m| m.get(id).cloned())
    }

    pub fn probe_one(&mut self, ctx: &egui::Context, node: &Node) {
        let lives = Arc::clone(&self.lives);
        let inflight = Arc::clone(&self.inflight);
        let ctx = ctx.clone();
        let node = node.clone();
        inflight.fetch_add(1, Ordering::SeqCst);
        std::thread::spawn(move || {
            let live = crate::probe::probe(&node);
            if let Ok(mut m) = lives.lock() {
                m.insert(node.id.clone(), live);
            }
            inflight.fetch_sub(1, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }

    fn open_inspector(&mut self, id: &str) {
        if !self.inspectors.iter().any(|x| x == id) {
            self.inspectors.push(id.to_string());
        }
    }
}

// ── top bar ─────────────────────────────────────────────────────────────────

pub fn top_bar(app: &mut App, ctx: &egui::Context) {
    let pal = app.pal;
    egui::TopBottomPanel::top("bar")
        .frame(
            egui::Frame::new()
                .fill(pal.plane)
                .inner_margin(egui::Margin::symmetric(14, 8)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🏛").size(20.0));
                ui.label(
                    RichText::new("CONCOURSE")
                        .font(FontId::monospace(19.0))
                        .color(pal.ink)
                        .strong(),
                );
                ui.label(RichText::new("— the station's shared hall").color(pal.ink_2).size(12.5));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(mono(chrono::Local::now().format("%H:%M:%S").to_string(), 15.0, pal.ink_2));
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                // the two renders of the hall: departures+bays, or the estate map
                for (mode, label, hint) in [
                    (super::ViewMode::Hall, "🏛 hall", "departures board + the bays"),
                    (super::ViewMode::Estate, "🧭 estate", "top-down folders + attention flow"),
                ] {
                    let on = app.view == mode;
                    if ui
                        .selectable_label(on, RichText::new(label).size(13.5))
                        .on_hover_text(hint)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        && !on
                    {
                        app.view = mode;
                        app.save_prefs();
                    }
                }
                ui.separator();
                let toggle = |ui: &mut egui::Ui, label: &str, on: &mut bool| {
                    let r = ui.selectable_label(*on, RichText::new(label).size(13.5));
                    if r.clicked() {
                        *on = !*on;
                    }
                    r.on_hover_cursor(egui::CursorIcon::PointingHand)
                };
                let mut open_cabinet = app.show_cabinet;
                let mut open_skills = app.show_skills;
                let mut open_asks = app.show_asks;
                let mut open_doctor = app.show_doctor;
                let mut open_services = app.show_services;
                toggle(ui, "🗄 cabinet", &mut open_cabinet);
                toggle(ui, "🎓 skills", &mut open_skills);
                toggle(ui, "📒 asks", &mut open_asks);
                toggle(ui, "🩺 doctor", &mut open_doctor);
                toggle(ui, "⚙ services", &mut open_services);
                if open_cabinet && app.cabinet.is_none() {
                    app.load_cabinet();
                }
                if open_skills && app.skills.is_none() {
                    if let Ok(reg) = &app.reg {
                        app.skills = Some(crate::skills::report(&reg.skill_mirrors));
                    }
                }
                if open_asks {
                    app.asks = crate::asks::list().unwrap_or_default();
                }
                if open_doctor && !app.show_doctor {
                    app.run_doctor(ctx);
                }
                if open_services && !app.show_services {
                    app.load_services(ctx);
                    app.jobs_polled_at = None; // fresh ledger on open
                }
                app.show_cabinet = open_cabinet;
                app.show_skills = open_skills;
                app.show_asks = open_asks;
                app.show_doctor = open_doctor;
                app.show_services = open_services;

                if ui
                    .selectable_label(false, RichText::new("🗺 map").size(13.5))
                    .on_hover_text("the SymbolOS map (index.html)")
                    .clicked()
                {
                    if let Ok(reg) = &app.reg {
                        if let Some(n) = reg.node("station") {
                            let _ = crate::verbs::open_node(n);
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let busy = app.inflight.load(Ordering::SeqCst) > 0;
                    let label = if busy { "⟳ PROBING…" } else { "⟳ REFRESH" };
                    if stone_button(ui, &pal, label, egui::vec2(116.0, 30.0), busy).clicked() && !busy {
                        app.refresh(ctx);
                    }
                    ui.add_space(6.0);
                    if stone_button(ui, &pal, pal.label, egui::vec2(116.0, 30.0), false)
                        .on_hover_text("cycle theme")
                        .clicked()
                    {
                        app.pal = super::theme::next_palette(app.pal.id);
                        app.pal.apply(ctx);
                        app.save_theme();
                    }
                });
            });
        });
}

// ── status strip ────────────────────────────────────────────────────────────

pub fn status_strip(app: &mut App, ctx: &egui::Context) {
    let pal = app.pal;
    egui::TopBottomPanel::bottom("strip")
        .frame(
            egui::Frame::new()
                .fill(pal.surface_2)
                .inner_margin(egui::Margin::symmetric(14, 6)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let lives = app.lives.lock().map(|m| m.clone()).unwrap_or_default();
                let ok = lives
                    .values()
                    .filter(|l| matches!(l.state, State::Ok | State::Present))
                    .count();
                let attention = lives
                    .values()
                    .filter(|l| matches!(l.state, State::Error | State::Missing))
                    .count();
                let total = app.reg.as_ref().map(|r| r.nodes.len()).unwrap_or(0);
                ui.label(mono(format!("{total} nodes"), 12.0, pal.ink_2));
                ui.label(mono(format!("{ok} well"), 12.0, status_color(&pal, State::Ok)));
                if attention > 0 {
                    ui.label(mono(
                        format!("{attention} need attention"),
                        12.0,
                        status_color(&pal, State::Error),
                    ));
                }
                let gov = crate::verbs::governance_summary();
                let sealed = gov["readonly"].as_bool().unwrap_or(false);
                ui.label(mono(
                    if gov["present"].as_bool().unwrap_or(false) {
                        if sealed { "cabinet sealed" } else { "cabinet WRITABLE" }
                    } else {
                        "cabinet MISSING"
                    },
                    12.0,
                    if sealed { pal.ink_2 } else { status_color(&pal, State::Unavailable) },
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((msg, at)) = &app.toast {
                        if at.elapsed().as_secs_f32() < 3.0 {
                            ui.label(mono(msg.clone(), 12.0, pal.accent));
                        }
                    }
                    if let Some(t) = app.last_refresh {
                        ui.label(mono(format!("probed {}s ago", t.elapsed().as_secs()), 12.0, pal.muted));
                    }
                });
            });
        });
}

// ── central: the board + the bays ───────────────────────────────────────────

pub fn central(app: &mut App, ctx: &egui::Context) {
    let pal = app.pal;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(pal.plane).inner_margin(egui::Margin::same(14)))
        .show(ctx, |ui| {
            match &app.reg {
                Err((e, fix)) => {
                    ui.label(RichText::new("the registry could not load").color(pal.ink).strong());
                    ui.label(RichText::new(e).color(pal.ink_2));
                    ui.label(mono(format!("fix: {fix}"), 12.5, pal.accent));
                }
                Ok(_) => match app.view {
                    super::ViewMode::Hall => {
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            board(app, ui);
                            ui.add_space(14.0);
                            bays(app, ui);
                            ui.add_space(10.0);
                        });
                    }
                    super::ViewMode::Estate => {
                        super::estate::estate_central(app, ui);
                    }
                },
            }
        });
}

fn section_title(ui: &mut egui::Ui, pal: &Palette, title: &str, sub: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(FontId::monospace(14.0)).color(pal.ink).strong());
        ui.label(RichText::new(sub).color(pal.muted).size(11.5));
    });
    ui.add_space(2.0);
}

/// Fixed-width, left-aligned board cell (add_sized centers; boards don't).
fn cell(ui: &mut egui::Ui, w: f32, rt: RichText) {
    ui.allocate_ui_with_layout(
        egui::vec2(w, 18.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(w);
            ui.label(rt);
        },
    );
}

fn board(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Ok(reg) = &app.reg else { return };
    let rows: Vec<Node> = reg
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Tool | NodeKind::Service))
        .cloned()
        .collect();

    section_title(ui, &pal, "DEPARTURES", "— live state of every tool and service · click a row to inspect");
    egui::Frame::new()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.line_strong))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            // header
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                cell(ui, 196.0, mono("NODE", 10.5, pal.muted));
                cell(ui, 76.0, mono("VERSION", 10.5, pal.muted));
                cell(ui, 52.0, mono("MS", 10.5, pal.muted));
                ui.label(mono("REMARK", 10.5, pal.muted));
            });
            let sep = ui.available_width();
            let (r, _) = ui.allocate_exact_size(egui::vec2(sep, 1.0), Sense::hover());
            ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, pal.line));

            for node in &rows {
                board_row(app, ui, node);
            }
        });
}

fn board_row(app: &mut App, ui: &mut egui::Ui, node: &Node) {
    let pal = app.pal;
    let live = app.live_for(&node.id);
    let (state, version, detail, ms) = match &live {
        Some(l) => (
            l.state,
            l.version.clone().unwrap_or_else(|| "—".into()),
            l.detail.clone(),
            format!("{}", l.latency_ms),
        ),
        None => (State::Unprobed, "—".into(), "…".into(), "".into()),
    };

    let bg_idx = ui.painter().add(egui::Shape::Noop);
    let resp = ui
        .horizontal(|ui| {
            ui.label(
                RichText::new(lamp_glyph(state))
                    .font(FontId::monospace(12.5))
                    .color(status_color(&pal, state)),
            );
            cell(
                ui,
                196.0,
                RichText::new(take(&format!("{} {}", node.glyph, node.name), 26))
                    .color(pal.ink)
                    .size(13.5),
            );
            cell(ui, 76.0, mono(take(&version, 9), 12.0, pal.ink_2));
            cell(ui, 52.0, mono(ms, 11.5, pal.muted));
            ui.label(mono(take(&detail, 64), 12.0, pal.ink_2));
        })
        .response;

    let rect = resp.rect.expand2(egui::vec2(4.0, 1.0));
    let click = ui.interact(rect, egui::Id::new(("row", &node.id)), Sense::click());
    if click.hovered() {
        ui.painter().set(
            bg_idx,
            egui::Shape::rect_filled(rect, 0.0, super::theme::lerp(pal.surface, pal.accent, 0.07)),
        );
    }
    // gentle flash when a state changes (skipped under reduce_motion)
    if !app.reduce_motion {
        if let Some(at) = app.flash.get(&node.id) {
            let t = at.elapsed().as_secs_f32();
            if t < 0.9 {
                let a = ((0.9 - t) / 0.9) * 0.16;
                ui.painter().set(
                    bg_idx,
                    egui::Shape::rect_filled(rect, 0.0, pal.accent.gamma_multiply(a)),
                );
                ui.ctx().request_repaint();
            }
        }
    }
    if click.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        app.open_inspector(&node.id);
    }
}

fn bays(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    let Ok(reg) = &app.reg else { return };
    let districts = reg.districts.clone();
    let nodes = reg.nodes.clone();
    for district in &districts {
        let in_d: Vec<&Node> = nodes.iter().filter(|n| &n.district == district).collect();
        if in_d.is_empty() {
            continue;
        }
        ui.add_space(8.0);
        section_title(ui, &pal, &district.to_uppercase(), &format!("— {} bays", in_d.len()));
        let cols = ((ui.available_width() + 10.0) / 302.0).floor().max(1.0) as usize;
        egui::Grid::new(("bays", district))
            .num_columns(cols)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                for (i, node) in in_d.iter().enumerate() {
                    node_card(app, ui, node);
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    }
}

fn node_card(app: &mut App, ui: &mut egui::Ui, node: &Node) {
    let pal = app.pal;
    let live = app.live_for(&node.id);
    let state = live.as_ref().map(|l| l.state).unwrap_or(State::Unprobed);

    let card = egui::Frame::new()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.line))
        .inner_margin(egui::Margin::same(10));
    ui.allocate_ui_with_layout(
        egui::vec2(292.0, 10.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
        ui.set_width(292.0);
        ui.set_max_width(292.0);
        card.show(ui, |ui| {
            ui.set_width(272.0);
            ui.set_max_width(272.0);
            ui.set_min_height(74.0); // equal-height cards keep bay rows level
            // header row — click to inspect
            let head = ui
                .horizontal(|ui| {
                    ui.label(RichText::new(&node.glyph).size(16.0));
                    ui.label(RichText::new(take(&node.name, 22)).color(pal.ink).strong().size(14.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(lamp_glyph(state))
                                .font(FontId::monospace(12.0))
                                .color(status_color(&pal, state)),
                        );
                        ui.label(mono(node.kind.label(), 10.0, pal.muted));
                    });
                })
                .response;
            let head_click = ui.interact(
                head.rect.expand2(egui::vec2(2.0, 1.0)),
                egui::Id::new(("card", &node.id)),
                Sense::click(),
            );
            if head_click.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                app.open_inspector(&node.id);
            }

            ui.label(RichText::new(take(&node.line, 92)).color(pal.ink_2).size(11.8));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if node.open.is_some() {
                    if ui.button(RichText::new("open").size(12.0)).clicked() {
                        match crate::verbs::open_node(node) {
                            Ok(_) => app.note(format!("opened {}", node.id)),
                            Err(e) => app.note(e),
                        }
                    }
                }
                if let Some(p) = &node.path {
                    if ui.button(RichText::new("folder").size(12.0)).clicked() {
                        let dir = crate::util::expand_home(p);
                        let dir = if dir.is_file() { dir.parent().map(|x| x.to_path_buf()).unwrap_or(dir) } else { dir };
                        let _ = crate::util::xdg_open(&dir);
                    }
                }
                if let Some(s) = &node.signpost {
                    if ui.button(RichText::new("signpost").size(12.0)).clicked() {
                        let _ = crate::util::xdg_open(&crate::util::expand_home(s));
                    }
                }
            });
        });
    });
}

// ── the windows (persist until dismissed — house rule) ─────────────────────

pub fn windows(app: &mut App, ctx: &egui::Context) {
    inspectors(app, ctx);
    cabinet_window(app, ctx);
    skills_window(app, ctx);
    asks_window(app, ctx);
    doctor_window(app, ctx);
    services_window(app, ctx);
}

fn inspectors(app: &mut App, ctx: &egui::Context) {
    let pal = app.pal;
    let ids = app.inspectors.clone();
    let mut closed: Vec<String> = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let Ok(reg) = &app.reg else { break };
        let Some(node) = reg.node(id).cloned() else {
            closed.push(id.clone());
            continue;
        };
        let live = app.live_for(id);
        let mut open = true;
        egui::Window::new(RichText::new(format!("{} {}", node.glyph, node.name)).strong())
            .id(egui::Id::new(("insp", id)))
            .open(&mut open)
            .resizable(true)
            .default_width(440.0)
            .default_pos(egui::pos2(130.0 + 26.0 * i as f32, 150.0 + 26.0 * i as f32))
            .show(ctx, |ui| {
                let state = live.as_ref().map(|l| l.state).unwrap_or(State::Unprobed);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(lamp_glyph(state))
                            .font(FontId::monospace(13.0))
                            .color(status_color(&pal, state)),
                    );
                    ui.label(mono(state.label(), 13.0, status_color(&pal, state)));
                    if let Some(l) = &live {
                        if let Some(v) = &l.version {
                            ui.label(mono(v.clone(), 12.5, pal.ink_2));
                        }
                        ui.label(mono(format!("{}ms", l.latency_ms), 11.5, pal.muted));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(mono(format!("{} › {}", node.district, node.kind.label()), 11.0, pal.muted));
                    });
                });
                if let Some(l) = &live {
                    ui.label(RichText::new(&l.detail).color(pal.ink_2).size(12.5));
                }
                ui.separator();
                ui.label(RichText::new(&node.line).color(pal.ink).size(13.0));
                if let Some(n) = &node.note {
                    ui.label(RichText::new(n).color(pal.ink_2).italics().size(12.0));
                }
                ui.add_space(4.0);
                if let Some(p) = &node.path {
                    ui.horizontal(|ui| {
                        ui.label(mono("path", 11.0, pal.muted));
                        ui.label(mono(take(p, 46), 11.5, pal.ink_2)).on_hover_text(p);
                    });
                }
                if let Some(b) = &node.bin {
                    ui.horizontal(|ui| {
                        ui.label(mono("bin ", 11.0, pal.muted));
                        ui.label(mono(take(b, 46), 11.5, pal.ink_2)).on_hover_text(b);
                    });
                }
                if !node.verbs.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(mono("verbs", 11.0, pal.muted));
                        ui.label(mono(node.verbs.join(" · "), 11.5, pal.accent));
                    });
                }
                if let Some(raw) = live.as_ref().and_then(|l| l.raw.clone()) {
                    ui.add_space(4.0);
                    egui::CollapsingHeader::new(mono("probe payload", 12.0, pal.ink_2))
                        .id_salt(("raw", id))
                        .default_open(false)
                        .show(ui, |ui| {
                            let pretty = serde_json::to_string_pretty(&raw).unwrap_or_default();
                            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                                ui.label(mono(pretty.clone(), 11.0, pal.ink_2));
                            });
                            if ui.button(RichText::new("copy json").size(11.5)).clicked() {
                                ui.ctx().copy_text(pretty);
                            }
                        });
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if node.probe.is_some() && ui.button("probe again").clicked() {
                        let n = node.clone();
                        app.probe_one(ctx, &n);
                    }
                    if node.open.is_some() && ui.button("open").clicked() {
                        match crate::verbs::open_node(&node) {
                            Ok(_) => app.note(format!("opened {}", node.id)),
                            Err(e) => app.note(e),
                        }
                    }
                    if let Some(s) = &node.signpost {
                        if ui.button("signpost").clicked() {
                            let _ = crate::util::xdg_open(&crate::util::expand_home(s));
                        }
                    }
                    if let Some(ProbeSpec::Cmd { argv, .. }) = &node.probe {
                        if ui.button("copy probe cmd").clicked() {
                            ui.ctx().copy_text(argv.join(" "));
                        }
                    }
                });
            });
        if !open {
            closed.push(id.clone());
        }
    }
    app.inspectors.retain(|x| !closed.contains(x));
}

fn cabinet_window(app: &mut App, ctx: &egui::Context) {
    if !app.show_cabinet {
        return;
    }
    let pal = app.pal;
    let mut open = app.show_cabinet;
    let doc_missing = app.cabinet.is_none();
    egui::Window::new(RichText::new("🗄 the filing cabinet").strong())
        .id(egui::Id::new("cabinet"))
        .open(&mut open)
        .resizable(true)
        .default_width(560.0)
        .default_height(520.0)
        .show(ctx, |ui| {
            if doc_missing {
                ui.label("~/Dev/ClaudeWorkspace/AGENTS.md could not be read — `concourse doctor` will say why.");
                return;
            }
            let (path, readonly, sha12, mtime) = {
                let d = app.cabinet.as_ref().unwrap();
                (d.path.clone(), d.readonly, d.sha12.clone(), d.mtime.clone())
            };
            egui::Frame::new()
                .stroke(Stroke::new(1.0, pal.accent.gamma_multiply(0.55)))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Ben's hand only — agents read and follow; changes are proposed in conversation, never written here.")
                            .color(pal.ink_2)
                            .size(12.0),
                    );
                    ui.label(mono(
                        format!(
                            "{} · {} · sha {} · tended {}",
                            take(&path, 34),
                            if readonly { "sealed 444" } else { "WRITABLE" },
                            sha12,
                            mtime
                        ),
                        10.5,
                        pal.muted,
                    ));
                });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("open in editor").clicked() {
                    let _ = crate::util::xdg_open(&crate::util::cabinet_path());
                }
                if ui.button("copy path").clicked() {
                    ui.ctx().copy_text(path.clone());
                }
                if ui.button("reload").clicked() {
                    app.load_cabinet();
                }
            });
            ui.separator();
            let text = app.cabinet.as_ref().map(|d| d.text.clone()).unwrap_or_default();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                md_lite(ui, &pal, &text);
            });
        });
    app.show_cabinet = open;
}

/// One rack (claude/hermes): its root dir and nested categories, counted.
struct RackView {
    rack: &'static str,
    root: std::path::PathBuf,
    total: usize,
    cats: Vec<(String, usize)>, // nested category name, skill count
}

fn rack_view(rack: &'static str, root: std::path::PathBuf, skills: &[crate::skills::SkillInfo]) -> RackView {
    let mut cats: Vec<(String, usize)> = Vec::new();
    for s in skills {
        if let Some((cat, _)) = s.name.split_once('/') {
            match cats.iter_mut().find(|(c, _)| c == cat) {
                Some((_, n)) => *n += 1,
                None => cats.push((cat.to_string(), 1)),
            }
        }
    }
    cats.sort_by(|a, b| a.0.cmp(&b.0));
    RackView { rack, root, total: skills.len(), cats }
}

/// A 📁 button + selectable label — the folder opens, the name filters.
fn cat_row(
    ui: &mut egui::Ui,
    pal: &Palette,
    sel: &mut String,
    key: &str,
    label: &str,
    count: usize,
    dir: &std::path::Path,
    indent: f32,
) {
    ui.horizontal(|ui| {
        ui.add_space(indent);
        if ui
            .button(RichText::new("📁").size(11.5))
            .on_hover_text(format!("open {}", dir.display()))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            let _ = crate::util::xdg_open(&dir.to_path_buf());
        }
        let active = sel == key;
        // selected rows sit on the accent fill — ink must flip to on_accent
        let r = ui.selectable_label(
            active,
            mono(format!("{label} ({count})"), 12.0, if active { pal.on_accent } else { pal.ink }),
        );
        if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            *sel = if active { "all".into() } else { key.to_string() };
        }
    });
}

fn skills_window(app: &mut App, ctx: &egui::Context) {
    if !app.show_skills {
        return;
    }
    let pal = app.pal;
    let mut open = app.show_skills;
    egui::Window::new(RichText::new("🎓 the skills rack").strong())
        .id(egui::Id::new("skills"))
        .open(&mut open)
        .resizable(true)
        .default_width(760.0)
        .default_height(520.0)
        .min_width(520.0)
        .min_height(300.0)
        .show(ctx, |ui| {
            let Some(sk) = app.skills.clone() else {
                ui.label("scanning…");
                return;
            };
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Claude and Hermes stay separate — mirrors sync only on Ben's ping.")
                        .color(pal.ink_2)
                        .size(12.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("rescan").size(12.0)).clicked() {
                        if let Ok(reg) = &app.reg {
                            app.skills = Some(crate::skills::report(&reg.skill_mirrors));
                        }
                    }
                    ui.label(mono(
                        format!("{} claude · {} hermes", sk.claude.len(), sk.hermes.len()),
                        12.0,
                        pal.ink_2,
                    ));
                });
            });
            ui.separator();

            let racks = [
                rack_view("claude", crate::skills::claude_root(), &sk.claude),
                rack_view("hermes", crate::skills::hermes_root(), &sk.hermes),
            ];
            let mut sel = app.skill_sel.clone();

            // two panes fill the window — content claims all space so the
            // window resizes freely on both axes (no hug-the-content fights)
            let body_h = ui.available_height();
            ui.horizontal_top(|ui| {
                ui.set_min_height(body_h);
                // ── left: the racks — folder icons open, names filter ──
                ui.allocate_ui_with_layout(
                    egui::vec2(190.0, body_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(190.0);
                        egui::ScrollArea::vertical()
                            .id_salt("skcats")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.label(mono("RACKS", 10.5, pal.muted));
                                ui.add_space(2.0);
                                let all = sel == "all";
                                if ui
                                    .selectable_label(all, mono("everything", 12.0, if all { pal.on_accent } else { pal.ink }))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    sel = "all".into();
                                }
                                for rv in &racks {
                                    ui.add_space(4.0);
                                    cat_row(ui, &pal, &mut sel, rv.rack, rv.rack, rv.total, &rv.root, 0.0);
                                    for (cat, n) in &rv.cats {
                                        let key = format!("{}/{}", rv.rack, cat);
                                        cat_row(ui, &pal, &mut sel, &key, cat, *n, &rv.root.join(cat), 14.0);
                                    }
                                }
                            });
                    },
                );
                ui.separator();
                // ── right: mirrors (unfiltered view) + the skill list ──
                ui.vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("sklist")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if sel == "all" {
                                ui.label(mono("MIRRORS", 10.5, pal.muted));
                                for m in &sk.mirrors {
                                    let okp = m.state == "paired";
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(if okp { "✓" } else { "⚠" })
                                                .font(FontId::monospace(12.0))
                                                .color(if okp {
                                                    status_color(&pal, State::Ok)
                                                } else {
                                                    status_color(&pal, State::Unavailable)
                                                }),
                                        );
                                        ui.label(mono(take(&m.claude, 34), 12.0, pal.ink));
                                        ui.label(mono("↔", 12.0, pal.muted));
                                        ui.label(mono(take(&m.hermes, 26), 12.0, pal.ink));
                                        if !okp {
                                            ui.label(mono(m.state.clone(), 11.0, status_color(&pal, State::Unavailable)));
                                        }
                                    });
                                }
                                ui.separator();
                            }
                            for rv in &racks {
                                let list = if rv.rack == "claude" { &sk.claude } else { &sk.hermes };
                                let shown: Vec<_> = list
                                    .iter()
                                    .filter(|s| match sel.as_str() {
                                        "all" => true,
                                        k if k == rv.rack => true,
                                        k => k
                                            .strip_prefix(&format!("{}/", rv.rack))
                                            .is_some_and(|cat| s.name.starts_with(&format!("{cat}/"))),
                                    })
                                    .collect();
                                if shown.is_empty() {
                                    continue;
                                }
                                ui.add_space(4.0);
                                ui.label(mono(format!("{} ({})", rv.rack.to_uppercase(), shown.len()), 10.5, pal.muted));
                                for s in shown {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .button(RichText::new("📁").size(10.5))
                                            .on_hover_text("open this skill's folder")
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            if let Some(dir) = std::path::Path::new(&s.path).parent() {
                                                let _ = crate::util::xdg_open(&dir.to_path_buf());
                                            }
                                        }
                                        ui.label(mono(take(&s.name, 34), 11.5, pal.ink));
                                        ui.label(RichText::new(take(&s.description, 72)).color(pal.muted).size(10.5))
                                            .on_hover_text(&s.description);
                                    });
                                }
                            }
                        });
                });
            });
            app.skill_sel = sel;
        });
    app.show_skills = open;
}

fn asks_window(app: &mut App, ctx: &egui::Context) {
    if !app.show_asks {
        return;
    }
    let pal = app.pal;
    let mut open = app.show_asks;
    egui::Window::new(RichText::new("📒 the asks ledger").strong())
        .id(egui::Id::new("asks"))
        .open(&mut open)
        .resizable(true)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Every unique ask, remembered — how bugs get rediscovered and intent survives sessions.")
                    .color(pal.ink_2)
                    .size(12.0),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("ask-by")
                    .selected_text(mono(app.ask_by.clone(), 12.0, pal.ink))
                    .width(86.0)
                    .show_ui(ui, |ui| {
                        for who in ["ben", "claude", "nexus"] {
                            ui.selectable_value(&mut app.ask_by, who.to_string(), who);
                        }
                    });
                let edit = egui::TextEdit::singleline(&mut app.ask_input)
                    .hint_text("what was asked…")
                    .desired_width(ui.available_width() - 64.0);
                let r = ui.add(edit);
                let submit = ui.button("add").clicked()
                    || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                if submit && !app.ask_input.trim().is_empty() {
                    match crate::asks::add(&app.ask_by, &app.ask_input) {
                        Ok(_) => {
                            app.ask_input.clear();
                            app.asks = crate::asks::list().unwrap_or_default();
                            app.note("ask noted");
                        }
                        Err(e) => app.note(e),
                    }
                }
            });
            ui.separator();
            if app.asks.is_empty() {
                ui.label(RichText::new("the ledger is quiet — the first ask starts it").color(pal.muted).size(12.0));
            }
            egui::ScrollArea::vertical().auto_shrink([false, true]).max_height(380.0).show(ui, |ui| {
                for a in app.asks.iter().rev() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(mono(a.ts.chars().take(16).collect::<String>(), 10.5, pal.muted));
                        ui.label(mono(format!("[{}]", a.by), 10.5, pal.accent));
                        ui.label(RichText::new(&a.text).color(pal.ink).size(12.5));
                    });
                }
            });
        });
    app.show_asks = open;
}

fn doctor_window(app: &mut App, ctx: &egui::Context) {
    if !app.show_doctor {
        return;
    }
    let pal = app.pal;
    let mut open = app.show_doctor;
    egui::Window::new(RichText::new("🩺 the doctor").strong())
        .id(egui::Id::new("doctor"))
        .open(&mut open)
        .resizable(true)
        .default_width(620.0)
        .default_height(480.0)
        .show(ctx, |ui| {
            let running = app.doctor_running.load(Ordering::SeqCst) > 0;
            let report = app.doctor.lock().ok().and_then(|r| r.as_ref().map(|x| {
                (x.pass, x.warn, x.fail, x.findings.clone())
            }));
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("The estate, checked against the standard — probes, envelopes, branch law, governance, mirrors.")
                        .color(pal.ink_2)
                        .size(12.0),
                );
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(!running, egui::Button::new("run again")).clicked() {
                    app.run_doctor(ctx);
                }
                if running {
                    ui.spinner();
                    ui.label(mono("checking…", 11.5, pal.muted));
                }
                if let Some((p, w, f, _)) = &report {
                    ui.label(mono(format!("{p} pass"), 12.0, status_color(&pal, State::Ok)));
                    ui.label(mono(format!("{w} warn"), 12.0, status_color(&pal, State::Unavailable)));
                    ui.label(mono(format!("{f} fail"), 12.0, status_color(&pal, State::Error)));
                }
            });
            ui.separator();
            if let Some((_, _, _, findings)) = report {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let mut last = String::new();
                    for fdg in &findings {
                        if fdg.subject != last {
                            ui.add_space(4.0);
                            ui.label(mono(fdg.subject.to_uppercase(), 11.0, pal.ink));
                            last = fdg.subject.clone();
                        }
                        let (mark, col) = match fdg.level {
                            crate::doctor::Level::Pass => ("✓", status_color(&pal, State::Ok)),
                            crate::doctor::Level::Warn => ("⚠", status_color(&pal, State::Unavailable)),
                            crate::doctor::Level::Fail => ("✗", status_color(&pal, State::Error)),
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(mark).font(FontId::monospace(11.5)).color(col));
                            ui.label(mono(format!("{:<12}", take(&fdg.check, 12)), 11.0, pal.ink_2));
                            ui.label(RichText::new(take(&fdg.note, 84)).color(pal.muted).size(11.0))
                                .on_hover_text(&fdg.note);
                        });
                    }
                });
            } else if !running {
                ui.label(RichText::new("no report yet — run it").color(pal.muted));
            }
        });
    app.show_doctor = open;
}

// ── systemd services: see everything, dispatch an agent to change it ───────

fn services_window(app: &mut App, ctx: &egui::Context) {
    if !app.show_services {
        return;
    }
    app.poll_jobs(); // agents report via `concourse job report` — the ledger is the wire
    let pal = app.pal;
    let mut open = app.show_services;
    egui::Window::new(RichText::new("⚙ services — systemd through the hall").strong())
        .id(egui::Id::new("services"))
        .open(&mut open)
        .resizable(true)
        .default_width(780.0)
        .default_height(560.0)
        .min_width(560.0)
        .min_height(320.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Enable/disable doesn't run systemctl blindly — it dispatches an agent that does the work with judgement and reports back. Green: managed the way you want. Red: could not continue — the agent's log says why.")
                    .color(pal.ink_2)
                    .size(12.0),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let loading = app.services_loading.load(Ordering::SeqCst) > 0;
                for (label, user) in [("user", true), ("system", false)] {
                    let on = app.services_user_scope == user;
                    if ui
                        .selectable_label(on, RichText::new(label).size(12.5))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        && !on
                    {
                        app.services_user_scope = user;
                        if let Ok(mut s) = app.services.lock() {
                            *s = None;
                        }
                        app.load_services(ctx);
                    }
                }
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut app.services_filter)
                        .hint_text("filter units…")
                        .desired_width(180.0),
                );
                if ui.add_enabled(!loading, egui::Button::new(RichText::new("⟳ refresh").size(12.0))).clicked() {
                    app.load_services(ctx);
                }
                if loading {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let before = app.default_harness.clone();
                    egui::ComboBox::from_id_salt("dispatch-harness")
                        .selected_text(mono(app.default_harness.clone(), 12.0, pal.accent))
                        .width(104.0)
                        .show_ui(ui, |ui| {
                            for h in crate::services::HARNESSES {
                                ui.selectable_value(&mut app.default_harness, h.to_string(), h);
                            }
                        });
                    if app.default_harness != before {
                        app.save_prefs(); // the default is remembered
                    }
                    ui.label(mono("dispatch via", 11.0, pal.muted));
                });
            });
            ui.separator();

            let units = app.services.lock().ok().and_then(|s| s.clone());
            let body_h = (ui.available_height() - 130.0).max(120.0);
            match units {
                None => {
                    ui.label(RichText::new("reading systemd…").color(pal.muted));
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(&e).color(status_color(&pal, State::Error)).size(12.0));
                    ui.label(mono("fix: is systemd running? systemctl --version", 11.5, pal.accent));
                }
                Some(Ok(units)) => {
                    let filter = app.services_filter.to_lowercase();
                    let shown: Vec<&crate::services::Unit> = units
                        .iter()
                        .filter(|u| {
                            filter.is_empty()
                                || u.name.to_lowercase().contains(&filter)
                                || u.description.to_lowercase().contains(&filter)
                        })
                        .collect();
                    ui.label(mono(
                        format!("{} units ({} scope) · {} shown", units.len(), if app.services_user_scope { "user" } else { "system" }, shown.len()),
                        11.0,
                        pal.muted,
                    ));
                    egui::ScrollArea::vertical()
                        .id_salt("svc-list")
                        .max_height(body_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for u in shown {
                                service_row(app, ui, u);
                            }
                        });
                }
            }
            ui.separator();
            // ── recent jobs: the dispatch ledger, newest first ──
            egui::CollapsingHeader::new(mono(format!("jobs ({})", app.jobs.len()), 12.0, pal.ink_2))
                .default_open(!app.jobs.is_empty())
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("job-list").max_height(120.0).show(ui, |ui| {
                        for j in app.jobs.iter().rev().take(30) {
                            let (mark, col) = job_mark(&pal, &j.status);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(mark).font(FontId::monospace(12.0)).color(col));
                                ui.label(mono(j.ts.chars().take(16).collect::<String>(), 10.5, pal.muted));
                                ui.label(mono(format!("{} {}", j.action, take(&j.unit, 30)), 11.5, pal.ink));
                                ui.label(mono(format!("[{}]", j.harness), 10.5, pal.accent));
                                if !j.note.is_empty() {
                                    ui.label(RichText::new(take(&j.note, 40)).color(pal.ink_2).size(10.5))
                                        .on_hover_text(&j.note);
                                }
                                if ui.link(mono("log", 10.5, pal.muted)).clicked() {
                                    let _ = crate::util::xdg_open(&std::path::PathBuf::from(&j.log));
                                }
                            });
                        }
                    });
                });
        });
    app.show_services = open;
}

fn job_mark(pal: &Palette, status: &str) -> (&'static str, Color32) {
    match status {
        "ok" => ("●", status_color(pal, State::Ok)),
        "fail" => ("✗", status_color(pal, State::Error)),
        _ => ("⟳", status_color(pal, State::Unavailable)),
    }
}

fn service_row(app: &mut App, ui: &mut egui::Ui, u: &crate::services::Unit) {
    let pal = app.pal;
    let lamp = match (u.active.as_str(), u.sub.as_str()) {
        ("failed", _) | (_, "failed") => ("✗", status_color(&pal, State::Error)),
        ("active", _) => ("●", status_color(&pal, State::Ok)),
        _ => ("·", pal.muted),
    };
    let last = crate::services::latest_for(&app.jobs, &u.name, &u.scope);
    ui.horizontal(|ui| {
        ui.label(RichText::new(lamp.0).font(FontId::monospace(12.0)).color(lamp.1));
        ui.label(mono(take(&u.name, 38), 12.0, pal.ink)).on_hover_text(&u.name);
        cell(ui, 64.0, mono(
            u.enabled.clone(),
            10.5,
            match u.enabled.as_str() {
                "enabled" | "enabled-runtime" => status_color(&pal, State::Ok),
                "masked" => status_color(&pal, State::Error),
                _ => pal.muted,
            },
        ));
        cell(ui, 92.0, mono(format!("{}/{}", u.active, u.sub), 10.5, pal.ink_2));

        // the dispatch buttons — an agent goes and does it, then reports
        let action = match u.enabled.as_str() {
            "enabled" | "enabled-runtime" => Some("disable"),
            "disabled" => Some("enable"),
            _ => None, // static/masked/generated units aren't enable-toggled
        };
        if let Some(action) = action {
            let dispatched = last.as_ref().is_some_and(|j| j.status == "dispatched");
            if ui
                .add_enabled(!dispatched, egui::Button::new(RichText::new(action).size(11.0)))
                .on_hover_text(format!("dispatch {} to {action} this unit", app.default_harness))
                .clicked()
            {
                let scope = if app.services_user_scope { "user" } else { "system" };
                match crate::services::dispatch(&u.name, scope, action, &app.default_harness) {
                    Ok(job) => {
                        app.note(format!("dispatched {} to {action} {}", job.harness, u.name));
                        app.jobs_polled_at = None; // show the new job immediately
                    }
                    Err(e) => app.note(e),
                }
            }
        } else {
            cell(ui, 52.0, mono("—", 10.5, pal.muted));
        }
        // the report lamp: green = managed as asked · red = see the agent's log
        if let Some(j) = &last {
            let (mark, col) = job_mark(&pal, &j.status);
            let hover = match j.status.as_str() {
                "ok" => format!("{} — {}", j.note.trim(), "managed the way you asked"),
                "fail" => format!("could not continue: {} — click for the agent's log", j.note),
                _ => format!("{} is on it — click for the live log", j.harness),
            };
            if ui
                .link(RichText::new(mark).font(FontId::monospace(13.0)).color(col))
                .on_hover_text(hover)
                .clicked()
            {
                let _ = crate::util::xdg_open(&std::path::PathBuf::from(&j.log));
            }
        }
        ui.label(RichText::new(take(&u.description, 44)).color(pal.muted).size(10.5))
            .on_hover_text(&u.description);
    });
}

// ── md-lite: enough rendering for the cabinet to read with dignity ─────────

fn strip_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                } // ** → drop
            }
            '`' => {}
            _ => out.push(c),
        }
    }
    out
}

fn md_lite(ui: &mut egui::Ui, pal: &Palette, text: &str) {
    for line in text.lines() {
        let t = line.trim_end();
        if t.is_empty() {
            ui.add_space(5.0);
        } else if let Some(h) = t.strip_prefix("# ") {
            ui.label(RichText::new(strip_md(h)).color(pal.ink).strong().size(17.0));
        } else if let Some(h) = t.strip_prefix("## ") {
            ui.add_space(4.0);
            ui.label(RichText::new(strip_md(h)).color(pal.accent).strong().size(14.5));
        } else if let Some(h) = t.strip_prefix("### ") {
            ui.label(RichText::new(strip_md(h)).color(pal.ink).strong().size(13.0));
        } else if t.starts_with('|') {
            ui.label(mono(strip_md(t), 10.8, pal.ink_2));
        } else if let Some(b) = t.trim_start().strip_prefix("- ") {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("·").color(pal.accent).strong());
                ui.label(RichText::new(strip_md(b)).color(pal.ink).size(12.8));
            });
        } else if let Some(q) = t.strip_prefix("> ") {
            ui.label(RichText::new(strip_md(q)).color(pal.ink_2).italics().size(12.5));
        } else if t == "---" || t == "***" {
            ui.separator();
        } else {
            ui.label(RichText::new(strip_md(t)).color(pal.ink).size(12.8));
        }
    }
}
