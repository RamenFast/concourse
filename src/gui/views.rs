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
fn stone_button(ui: &mut egui::Ui, pal: &Palette, label: &str, size: egui::Vec2, lit: bool) -> egui::Response {
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
                toggle(ui, "🗄 cabinet", &mut open_cabinet);
                toggle(ui, "🎓 skills", &mut open_skills);
                toggle(ui, "📒 asks", &mut open_asks);
                toggle(ui, "🩺 doctor", &mut open_doctor);
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
                app.show_cabinet = open_cabinet;
                app.show_skills = open_skills;
                app.show_asks = open_asks;
                app.show_doctor = open_doctor;

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
                Ok(_) => {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        board(app, ui);
                        ui.add_space(14.0);
                        bays(app, ui);
                        ui.add_space(10.0);
                    });
                }
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
                ui.label("~/AGENTS.md could not be read — `concourse doctor` will say why.");
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
                    let _ = crate::util::xdg_open(&crate::util::expand_home("~/AGENTS.md"));
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
        .default_width(600.0)
        .show(ctx, |ui| {
            let Some(sk) = app.skills.clone() else {
                ui.label("scanning…");
                return;
            };
            ui.label(
                RichText::new("Claude and Hermes stay separate — mirrors sync only on Ben's ping. Each mirror names its source of record.")
                    .color(pal.ink_2)
                    .size(12.0),
            );
            ui.label(mono(
                format!("{} claude · {} hermes", sk.claude.len(), sk.hermes.len()),
                12.0,
                pal.ink_2,
            ));
            ui.separator();
            ui.label(mono("MIRRORS", 11.0, pal.muted));
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
                    ui.label(mono(take(&m.claude, 36), 12.0, pal.ink));
                    ui.label(mono("↔", 12.0, pal.muted));
                    ui.label(mono(take(&m.hermes, 28), 12.0, pal.ink));
                    if !okp {
                        ui.label(mono(m.state.clone(), 11.0, status_color(&pal, State::Unavailable)));
                    }
                });
            }
            ui.separator();
            egui::CollapsingHeader::new(mono(format!("claude skills ({})", sk.claude.len()), 12.0, pal.ink_2))
                .default_open(false)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(200.0).id_salt("csk").show(ui, |ui| {
                        for s in &sk.claude {
                            ui.horizontal(|ui| {
                                ui.label(mono(take(&s.name, 34), 11.5, pal.ink));
                                ui.label(RichText::new(take(&s.description, 60)).color(pal.muted).size(10.5));
                            });
                        }
                    });
                });
            egui::CollapsingHeader::new(mono(format!("hermes skills ({})", sk.hermes.len()), 12.0, pal.ink_2))
                .default_open(false)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(200.0).id_salt("hsk").show(ui, |ui| {
                        for s in &sk.hermes {
                            ui.horizontal(|ui| {
                                ui.label(mono(take(&s.name, 34), 11.5, pal.ink));
                                ui.label(RichText::new(take(&s.description, 60)).color(pal.muted).size(10.5));
                            });
                        }
                    });
                });
            if ui.button("rescan").clicked() {
                if let Ok(reg) = &app.reg {
                    app.skills = Some(crate::skills::report(&reg.skill_mirrors));
                }
            }
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
