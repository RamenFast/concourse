// SPDX-License-Identifier: GPL-3.0-or-later
//! The estate map: the bays as a top-down folder structure with attention
//! flow drawn between them — who points where, .md/.html doc by doc.
//! Orange is Claude, pink is Nexus, grey is opencode and the unknown.
//! One-way references are `-->` (weaker line); mutual are `<-->` (stronger);
//! brightness normalizes to the visible window of connection counts.
//! The left control panel collapses; folders hide (ghost or gone); every
//! folder opens in a terminal — the manual agent entry the workflow doc names.

use super::theme::{lerp, Palette};
use super::App;
use crate::attention::{FolderGraph, FolderNode, GraphData};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::sync::atomic::Ordering;
use std::sync::Arc;

// ── the semantic agent colors (stable across the estate) ───────────────────
pub fn agent_color(agent: &str) -> Color32 {
    match agent {
        "claude" => Color32::from_rgb(0xe8, 0x86, 0x3a),  // orange
        "nexus" => Color32::from_rgb(0xec, 0x8f, 0xac),   // pink
        _ => Color32::from_rgb(0x93, 0x93, 0x9c),         // opencode & misc — grey
    }
}

const COL_W: f32 = 232.0;
const COL_PITCH: f32 = 316.0;
const BOX_H: f32 = 30.0;
const SUB_H: f32 = 24.0;
const GAP: f32 = 7.0;

pub const CONCOURSE_DIR: &str = "~/Dev/ClaudeWorkspace/concourse";
pub const WORKFLOW_DOC: &str = "~/Dev/ClaudeWorkspace/concourse/docs/ATTENTION-WORKFLOW.md";

// ── data plumbing ───────────────────────────────────────────────────────────

impl App {
    pub fn ensure_estate(&mut self) {
        if self.estate.is_some() || self.estate_load_tried {
            return;
        }
        self.estate_load_tried = true;
        match crate::attention::load_graph() {
            Ok(g) => self.install_estate(g),
            Err(e) => self.estate_err = Some(e),
        }
    }

    pub fn install_estate(&mut self, g: GraphData) {
        self.estate_folders = Some(crate::attention::folder_graph(&g, self.estate_show_subdirs));
        self.estate = Some(g);
        self.estate_err = None;
    }

    pub fn rebuild_folders(&mut self) {
        if let Some(g) = &self.estate {
            self.estate_folders = Some(crate::attention::folder_graph(g, self.estate_show_subdirs));
        }
    }

    pub fn spawn_rescan(&mut self, ctx: &egui::Context) {
        if self.estate_scan_running.load(Ordering::SeqCst) > 0 {
            return;
        }
        let running = Arc::clone(&self.estate_scan_running);
        let slot = Arc::clone(&self.estate_scan_result);
        let ctx = ctx.clone();
        running.fetch_add(1, Ordering::SeqCst);
        std::thread::spawn(move || {
            let res = crate::attention::scan().and_then(|_| crate::attention::load_graph());
            if let Ok(mut s) = slot.lock() {
                *s = Some(res);
            }
            running.fetch_sub(1, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }
}

fn rel_time(pal: &Palette, ts: &str) -> RichText {
    let text = chrono::DateTime::parse_from_rfc3339(ts)
        .map(|t| {
            let mins = (chrono::Local::now().signed_duration_since(t)).num_minutes();
            match mins {
                m if m < 1 => "just now".to_string(),
                m if m < 60 => format!("{m}m ago"),
                m if m < 60 * 24 => format!("{}h ago", m / 60),
                m => format!("{}d ago", m / (60 * 24)),
            }
        })
        .unwrap_or_else(|_| "—".into());
    RichText::new(text).font(FontId::monospace(11.0)).color(pal.muted)
}

/// Degree (edge endpoints) per folder, over the current folder graph.
fn degrees(fg: &FolderGraph) -> Vec<u32> {
    let mut deg = vec![0u32; fg.folders.len()];
    for e in &fg.edges {
        let w = e.n_ab + e.n_ba;
        deg[e.a] += w;
        deg[e.b] += w;
    }
    deg
}

fn column_of(f: &FolderNode) -> usize {
    match f.root.as_str() {
        "workspace" => 1,
        "nexus" => 2,
        _ => 0,
    }
}

// ── the left control panel ──────────────────────────────────────────────────

pub fn control_panel(app: &mut App, ctx: &egui::Context) {
    let pal = app.pal;
    if !app.estate_panel_open {
        egui::SidePanel::left("estate-strip")
            .exact_width(26.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(pal.surface_2))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                if ui
                    .button(RichText::new("▸").size(13.0))
                    .on_hover_text("open the map controls")
                    .clicked()
                {
                    app.estate_panel_open = true;
                    app.save_prefs();
                }
            });
        return;
    }
    let mut dirty = false;
    egui::SidePanel::left("estate-panel")
        .default_width(252.0)
        .resizable(true)
        .frame(egui::Frame::new().fill(pal.surface_2).inner_margin(egui::Margin::same(10)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🧭 THE MAP").font(FontId::monospace(13.0)).color(pal.ink).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("◂").size(13.0)).on_hover_text("collapse").clicked() {
                        app.estate_panel_open = false;
                        dirty = true;
                    }
                });
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // ── the index ──
                ui.label(mono_muted(&pal, "THE INDEX"));
                let scanning = app.estate_scan_running.load(Ordering::SeqCst) > 0;
                match (&app.estate, &app.estate_err) {
                    (Some(g), _) => {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{} docs · {} links", g.docs.len(), g.links.len()))
                                    .font(FontId::monospace(11.5))
                                    .color(pal.ink_2),
                            );
                            ui.label(rel_time(&pal, &g.scanned_at));
                        });
                    }
                    (None, Some(_)) => {
                        ui.label(RichText::new("no index yet — scan builds it").color(pal.ink_2).size(11.5));
                    }
                    _ => {}
                }
                ui.horizontal(|ui| {
                    let label = if scanning { "⟳ SCANNING…" } else { "⟳ REINDEX" };
                    if super::views::stone_button(ui, &pal, label, egui::vec2(110.0, 26.0), scanning).clicked()
                        && !scanning
                    {
                        app.spawn_rescan(ui.ctx());
                    }
                    if scanning {
                        ui.spinner();
                    }
                });
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Agents refresh this by hand — open a terminal below and call claude, hermes, or opencode on the workflow doc.")
                        .color(pal.muted)
                        .size(10.5),
                );
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("🖥 concourse in terminal").size(11.5)).clicked() {
                        match crate::util::open_terminal(&crate::util::expand_home(CONCOURSE_DIR)) {
                            Ok(_) => app.note("terminal opened at the concourse project"),
                            Err(e) => app.note(e),
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("📄 workflow doc").size(11.5)).clicked() {
                        let _ = crate::util::xdg_open(&crate::util::expand_home(WORKFLOW_DOC));
                    }
                });
                ui.separator();

                // ── view options ──
                ui.label(mono_muted(&pal, "VIEW"));
                if ui.checkbox(&mut app.estate_show_subdirs, RichText::new("show subdirs").size(12.0)).changed() {
                    app.rebuild_folders();
                    dirty = true;
                }
                if ui
                    .checkbox(&mut app.estate_only_connected, RichText::new("only connected folders").size(12.0))
                    .on_hover_text("what agents and humans actually use — not noise")
                    .changed()
                {
                    dirty = true;
                }
                if ui
                    .checkbox(&mut app.estate_ghosts, RichText::new("ghost hidden folders").size(12.0))
                    .on_hover_text("hidden folders stay as grey, unnamed silhouettes")
                    .changed()
                {
                    dirty = true;
                }
                ui.separator();

                // ── agents legend ──
                ui.label(mono_muted(&pal, "AGENTS"));
                if let Some(g) = &app.estate {
                    let count = |a: &str| g.docs.iter().filter(|d| d.agent == a).count();
                    for (agent, label) in [
                        ("claude", "claude — orange"),
                        ("nexus", "nexus — pink"),
                        ("opencode", "opencode / misc — grey"),
                    ] {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                            ui.painter().rect_filled(r, 0.0, agent_color(agent));
                            ui.label(RichText::new(label).size(11.5).color(pal.ink_2));
                            ui.label(
                                RichText::new(format!("{}", count(agent)))
                                    .font(FontId::monospace(11.0))
                                    .color(pal.muted),
                            );
                        });
                    }
                }
                ui.separator();

                // ── folders (hide/show) ──
                ui.label(mono_muted(&pal, "FOLDERS"));
                if let Some(fg) = &app.estate_folders {
                    let mut keys: Vec<(String, String, u8)> = fg
                        .folders
                        .iter()
                        .filter(|f| f.depth <= 1)
                        .map(|f| (f.key.clone(), f.label.clone(), f.depth))
                        .collect();
                    keys.sort_by(|a, b| a.0.cmp(&b.0));
                    for (key, label, _) in keys {
                        let mut vis = !app.estate_hidden.contains(&key);
                        let text = RichText::new(label).size(11.5).color(if vis { pal.ink } else { pal.muted });
                        if ui.checkbox(&mut vis, text).changed() {
                            if vis {
                                app.estate_hidden.remove(&key);
                            } else {
                                app.estate_hidden.insert(key.clone());
                            }
                            dirty = true;
                        }
                    }
                }
                ui.separator();

                // ── connections ──
                ui.label(mono_muted(&pal, "CONNECTIONS"));
                if let Some(fg) = &app.estate_folders {
                    for (ei, e) in fg.edges.iter().take(30).enumerate() {
                        let (a, b) = (&fg.folders[e.a], &fg.folders[e.b]);
                        let mutual = e.n_ab > 0 && e.n_ba > 0;
                        let arrow = if mutual {
                            "⟷"
                        } else if e.n_ab > 0 {
                            "→"
                        } else {
                            "←"
                        };
                        let sel = app.estate_edge_sel == Some(ei);
                        let text = format!("{} {} {} · {}", trunc(&a.label, 12), arrow, trunc(&b.label, 12), e.n_ab + e.n_ba);
                        let r = ui.selectable_label(
                            sel,
                            RichText::new(text).font(FontId::monospace(10.8)).color(if sel {
                                pal.on_accent
                            } else if mutual {
                                pal.ink
                            } else {
                                pal.ink_2
                            }),
                        );
                        if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            app.estate_edge_sel = if sel { None } else { Some(ei) };
                        }
                    }
                }
            });
        });
    if dirty {
        app.save_prefs();
    }
}

fn mono_muted(pal: &Palette, s: &str) -> RichText {
    RichText::new(s).font(FontId::monospace(10.5)).color(pal.muted)
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

// ── the map itself ──────────────────────────────────────────────────────────

struct DrawBox {
    fi: usize, // folder index
    rect: Rect,
    ghost: bool,
}

pub fn estate_central(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal;
    app.ensure_estate();

    // pick up a finished background rescan
    let fresh = app.estate_scan_result.lock().ok().and_then(|mut s| s.take());
    if let Some(res) = fresh {
        match res {
            Ok(g) => {
                app.install_estate(g);
                app.note("attention index rebuilt");
            }
            Err(e) => app.note(e),
        }
    }

    let scanning = app.estate_scan_running.load(Ordering::SeqCst) > 0;
    if app.estate.is_none() {
        // gentle empty state — the first scan is a button, not a chore
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("🧭").size(34.0));
            ui.label(RichText::new("the estate map hasn't been drawn yet").color(pal.ink).size(15.0));
            ui.label(
                RichText::new("one scan walks every .md and .html doc and records who points where")
                    .color(pal.ink_2)
                    .size(12.0),
            );
            ui.add_space(10.0);
            let label = if scanning { "⟳ SCANNING…" } else { "SCAN THE ESTATE" };
            if super::views::stone_button(ui, &pal, label, egui::vec2(170.0, 32.0), scanning).clicked() && !scanning {
                app.spawn_rescan(ui.ctx());
            }
            if scanning {
                ui.add_space(6.0);
                ui.spinner();
            }
        });
        return;
    }
    let Some(fg) = app.estate_folders.clone() else { return };
    let deg = degrees(&fg);

    // visible set — hiding, ghosting, only-connected
    let mut boxes: Vec<DrawBox> = Vec::new();
    let mut visible = vec![false; fg.folders.len()];
    let mut order: Vec<usize> = (0..fg.folders.len()).collect();
    // column, then bay-group, then depth, then label — subdirs ride under their bay
    order.sort_by(|&x, &y| {
        let (a, b) = (&fg.folders[x], &fg.folders[y]);
        column_of(a)
            .cmp(&column_of(b))
            .then_with(|| a.depth.min(1).cmp(&b.depth.min(1))) // roots first in agent col
            .then_with(|| {
                let ga = a.above.clone().unwrap_or_else(|| a.label.clone());
                let gb = b.above.clone().unwrap_or_else(|| b.label.clone());
                ga.cmp(&gb)
            })
            .then_with(|| a.depth.cmp(&b.depth))
            .then_with(|| a.label.cmp(&b.label))
    });

    let mut col_y = [0.0f32; 3];
    let header_h = 26.0;
    for &fi in &order {
        let f = &fg.folders[fi];
        let hidden = app.estate_hidden.contains(&f.key)
            || f.above.as_ref().is_some_and(|_| {
                // a subdir under a hidden bay hides with it
                let bay_key = f.key.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default();
                app.estate_hidden.contains(&bay_key)
            });
        if hidden && !app.estate_ghosts {
            continue;
        }
        if !hidden && app.estate_only_connected && deg[fi] == 0 {
            continue;
        }
        let col = column_of(f);
        let (h, indent) = if f.depth == 2 { (SUB_H, 18.0) } else { (BOX_H, 0.0) };
        let x = 16.0 + col as f32 * COL_PITCH + indent;
        let y = header_h + 10.0 + col_y[col];
        col_y[col] += h + GAP;
        boxes.push(DrawBox {
            fi,
            rect: Rect::from_min_size(Pos2::new(x, y), Vec2::new(COL_W - indent, h)),
            ghost: hidden,
        });
        visible[fi] = !hidden;
    }

    let canvas_h = (col_y.iter().fold(0.0f32, |a, &b| a.max(b)) + header_h + 40.0).max(ui.available_height());
    let canvas_w = (16.0 + 3.0 * COL_PITCH + 60.0).max(ui.available_width());

    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        let (resp, painter) = ui.allocate_painter(Vec2::new(canvas_w, canvas_h), Sense::hover());
        let origin = resp.rect.min.to_vec2();

        // column headers
        for (col, title) in [
            (0usize, "AGENT HOMES — where they read from"),
            (1, "CLAUDEWORKSPACE — the bays"),
            (2, "NEXUS — the dwelling"),
        ] {
            painter.text(
                Pos2::new(16.0 + col as f32 * COL_PITCH, 8.0) + origin,
                Align2::LEFT_TOP,
                title,
                FontId::monospace(11.0),
                pal.muted,
            );
        }

        // hover state (from last frame's rects — fine at this scale)
        let hover_fi = app.estate_hover;

        // ── edges under boxes ──
        let vis_edges: Vec<(usize, &crate::attention::FolderEdge)> = fg
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| visible[e.a] && visible[e.b])
            .collect();
        let max_n = vis_edges.iter().map(|(_, e)| e.n_ab + e.n_ba).max().unwrap_or(1).max(1);
        let rect_of = |fi: usize| boxes.iter().find(|b| b.fi == fi).map(|b| b.rect.translate(origin));
        let mut same_col_k = 0usize;
        for (ei, e) in &vis_edges {
            if rect_of(e.a).is_none() || rect_of(e.b).is_none() {
                continue;
            }
            let total = e.n_ab + e.n_ba;
            let mutual = e.n_ab > 0 && e.n_ba > 0;
            // the overton window of brightness — normalized to what's visible
            let t = ((1.0 + total as f32).ln() / (1.0 + max_n as f32).ln()).clamp(0.0, 1.0);
            let mut alpha = 0.20 + 0.72 * t;
            let selected = app.estate_edge_sel == Some(*ei);
            let touched = hover_fi.is_some_and(|h| h == e.a || h == e.b);
            if let Some(h) = hover_fi {
                if h != e.a && h != e.b {
                    alpha *= 0.18;
                }
            }
            if touched {
                alpha = alpha.max(0.9);
            }
            if app.estate_edge_sel.is_some() && !selected {
                alpha *= 0.25;
            }
            // direction: draw from the heavier source
            let (src, dst, fwd_n) = if e.n_ab >= e.n_ba { (e.a, e.b, e.n_ab) } else { (e.b, e.a, e.n_ba) };
            let _ = fwd_n;
            let src_agent = &fg.folders[src].agent;
            let color = agent_color(src_agent).gamma_multiply(alpha);
            let width = if mutual { 1.7 } else { 1.0 }; // one-way is the weaker line
            let (rs, rd) = (rect_of(src).unwrap(), rect_of(dst).unwrap());
            let (p0, p3, c1, c2) = if (rs.center().x - rd.center().x).abs() < COL_PITCH * 0.5 {
                // same column — arc out the right side
                same_col_k += 1;
                let bulge = 30.0 + (same_col_k % 6) as f32 * 10.0;
                let a = Pos2::new(rs.right(), rs.center().y);
                let b = Pos2::new(rd.right(), rd.center().y);
                (a, b, a + Vec2::new(bulge, 0.0), b + Vec2::new(bulge, 0.0))
            } else if rs.center().x < rd.center().x {
                let a = Pos2::new(rs.right(), rs.center().y);
                let b = Pos2::new(rd.left(), rd.center().y);
                let dx = (b.x - a.x) * 0.38;
                (a, b, a + Vec2::new(dx, 0.0), b - Vec2::new(dx, 0.0))
            } else {
                let a = Pos2::new(rs.left(), rs.center().y);
                let b = Pos2::new(rd.right(), rd.center().y);
                let dx = (a.x - b.x) * 0.38;
                (a, b, a - Vec2::new(dx, 0.0), b + Vec2::new(dx, 0.0))
            };
            if selected {
                // a quiet glow under the chosen thread
                painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    [p0, c1, c2, p3],
                    false,
                    Color32::TRANSPARENT,
                    Stroke::new(width + 3.0, pal.accent.gamma_multiply(0.25)),
                ));
            }
            painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [p0, c1, c2, p3],
                false,
                Color32::TRANSPARENT,
                Stroke::new(width, color),
            ));
            arrow_head(&painter, p3, (p3 - c2).normalized(), color);
            if mutual {
                arrow_head(&painter, p0, (p0 - c1).normalized(), color);
            }
        }

        // ── the boxes ──
        let mut hover_now: Option<usize> = None;
        for b in &boxes {
            let f = &fg.folders[b.fi];
            let rect = b.rect.translate(origin);
            if b.ghost {
                // a silhouette without a name — hidden, remembered
                painter.rect_filled(rect, 0.0, pal.surface.gamma_multiply(0.55));
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, pal.line), egui::StrokeKind::Inside);
                continue;
            }
            let id = egui::Id::new(("estate-box", &f.key));
            let r = ui.interact(rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if r.hovered() {
                hover_now = Some(b.fi);
            }
            let open = app.estate_popouts.contains(&f.key);
            let acol = agent_color(&f.agent);
            let fill = if r.hovered() { lerp(pal.surface, acol, 0.10) } else { pal.surface };
            painter.rect_filled(rect, 0.0, fill);
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(
                    1.0,
                    if open { pal.accent } else { lerp(pal.line_strong.to_opaque(), acol, 0.55) },
                ),
                egui::StrokeKind::Inside,
            );
            let fsz = if f.depth == 2 { 11.0 } else { 12.5 };
            let name = if f.depth == 2 {
                format!("▸ {}", trunc(&f.label, 20))
            } else {
                format!("📁 {}", trunc(&f.label, 22))
            };
            painter.text(
                Pos2::new(rect.left() + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                name,
                FontId::monospace(fsz),
                pal.ink,
            );
            if let Some(above) = &f.above {
                painter.text(
                    Pos2::new(rect.left() + 8.0 + 130.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    format!("· {}", trunc(above, 10)),
                    FontId::monospace(9.5),
                    pal.muted,
                );
            }
            painter.text(
                Pos2::new(rect.right() - 8.0, rect.center().y),
                Align2::RIGHT_CENTER,
                format!("{}", f.docs),
                FontId::monospace(10.0),
                pal.muted,
            );
            if r.clicked() {
                if open {
                    app.estate_popouts.retain(|k| k != &f.key);
                } else {
                    app.estate_popouts.push(f.key.clone());
                }
            }
            r.context_menu(|ui| {
                if ui.button("🖥 open in terminal").clicked() {
                    match crate::util::open_terminal(&std::path::PathBuf::from(&f.path)) {
                        Ok(_) => app.note(format!("terminal at {}", f.label)),
                        Err(e) => app.note(e),
                    }
                    ui.close();
                }
                if ui.button("📂 open folder").clicked() {
                    let _ = crate::util::xdg_open(&std::path::PathBuf::from(&f.path));
                    ui.close();
                }
                if ui.button("🚫 hide from the map").clicked() {
                    app.estate_hidden.insert(f.key.clone());
                    app.save_prefs();
                    ui.close();
                }
            });
        }
        app.estate_hover = hover_now;
    });
}

fn arrow_head(painter: &egui::Painter, tip: Pos2, dir: Vec2, color: Color32) {
    if !dir.is_finite() || dir == Vec2::ZERO {
        return;
    }
    let d = dir.normalized();
    let perp = Vec2::new(-d.y, d.x);
    let p1 = tip - d * 9.0 + perp * 4.0;
    let p2 = tip - d * 9.0 - perp * 4.0;
    painter.add(egui::Shape::convex_polygon(vec![tip, p1, p2], color, Stroke::NONE));
}

// ── popouts: folder detail + edge inspector (persist until dismissed) ──────

pub fn estate_windows(app: &mut App, ctx: &egui::Context) {
    folder_popouts(app, ctx);
    edge_inspector(app, ctx);
}

fn folder_popouts(app: &mut App, ctx: &egui::Context) {
    let pal = app.pal;
    let keys = app.estate_popouts.clone();
    let (Some(g), Some(fg)) = (app.estate.clone(), app.estate_folders.clone()) else { return };
    let mut closed: Vec<String> = Vec::new();
    for (i, key) in keys.iter().enumerate() {
        let Some(fi) = fg.folders.iter().position(|f| &f.key == key) else {
            closed.push(key.clone());
            continue;
        };
        let f = &fg.folders[fi];
        let mut open = true;
        egui::Window::new(RichText::new(format!("📁 {}", f.label)).strong())
            .id(egui::Id::new(("estate-pop", key)))
            .open(&mut open)
            .resizable(true)
            .default_width(400.0)
            .default_height(360.0)
            .default_pos(egui::pos2(340.0 + 24.0 * i as f32, 120.0 + 24.0 * i as f32))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                    ui.painter().rect_filled(r, 0.0, agent_color(&f.agent));
                    ui.label(RichText::new(&f.agent).font(FontId::monospace(11.5)).color(pal.ink_2));
                    ui.label(RichText::new(format!("{} docs", f.docs)).font(FontId::monospace(11.5)).color(pal.muted));
                });
                ui.label(RichText::new(&f.path).font(FontId::monospace(10.5)).color(pal.muted));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if super::views::stone_button(ui, &pal, "🖥 TERMINAL", egui::vec2(104.0, 26.0), false)
                        .on_hover_text("open a terminal here — then call claude / hermes / opencode")
                        .clicked()
                    {
                        match crate::util::open_terminal(&std::path::PathBuf::from(&f.path)) {
                            Ok(_) => app.note(format!("terminal at {}", f.label)),
                            Err(e) => app.note(e),
                        }
                    }
                    if ui.button("📂 folder").clicked() {
                        let _ = crate::util::xdg_open(&std::path::PathBuf::from(&f.path));
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    // where this folder's docs point, and who points here
                    let outs: Vec<_> = fg
                        .edges
                        .iter()
                        .filter_map(|e| {
                            if e.a == fi && e.n_ab > 0 {
                                Some((e.b, e.n_ab))
                            } else if e.b == fi && e.n_ba > 0 {
                                Some((e.a, e.n_ba))
                            } else {
                                None
                            }
                        })
                        .collect();
                    let ins: Vec<_> = fg
                        .edges
                        .iter()
                        .filter_map(|e| {
                            if e.b == fi && e.n_ab > 0 {
                                Some((e.a, e.n_ab))
                            } else if e.a == fi && e.n_ba > 0 {
                                Some((e.b, e.n_ba))
                            } else {
                                None
                            }
                        })
                        .collect();
                    if !outs.is_empty() {
                        ui.label(mono_muted(&pal, "POINTS AT"));
                        for (o, n) in &outs {
                            ui.label(
                                RichText::new(format!("→ {} · {}", fg.folders[*o].label, n))
                                    .font(FontId::monospace(11.5))
                                    .color(pal.ink_2),
                            );
                        }
                        ui.add_space(4.0);
                    }
                    if !ins.is_empty() {
                        ui.label(mono_muted(&pal, "POINTED AT BY"));
                        for (o, n) in &ins {
                            ui.label(
                                RichText::new(format!("← {} · {}", fg.folders[*o].label, n))
                                    .font(FontId::monospace(11.5))
                                    .color(pal.ink_2),
                            );
                        }
                        ui.add_space(4.0);
                    }
                    // the folder's most-woven docs
                    let id_to_dense: std::collections::HashMap<i64, usize> =
                        g.docs.iter().enumerate().map(|(di, d)| (d.id, di)).collect();
                    let mut doc_deg: Vec<(usize, u32)> = g
                        .docs
                        .iter()
                        .enumerate()
                        .filter(|(di, _)| fg.doc_folder.get(*di) == Some(&fi))
                        .map(|(di, d)| {
                            let n: u32 = g
                                .links
                                .iter()
                                .filter(|l| {
                                    id_to_dense.get(&l.src) == Some(&di) || id_to_dense.get(&l.dst) == Some(&di)
                                })
                                .map(|l| l.n)
                                .sum();
                            let _ = d;
                            (di, n)
                        })
                        .collect();
                    doc_deg.sort_by(|a, b| b.1.cmp(&a.1));
                    ui.label(mono_muted(&pal, "DOCS (most connected first)"));
                    for (di, n) in doc_deg.iter().take(24) {
                        let d = &g.docs[*di];
                        let name = std::path::Path::new(&d.path)
                            .file_name()
                            .map(|x| x.to_string_lossy().into_owned())
                            .unwrap_or_else(|| d.path.clone());
                        ui.horizontal(|ui| {
                            let r = ui.link(RichText::new(trunc(&name, 40)).font(FontId::monospace(11.0)));
                            if r.clicked() {
                                let _ = crate::util::xdg_open(&std::path::PathBuf::from(&d.path));
                            }
                            if *n > 0 {
                                ui.label(RichText::new(format!("{n}")).font(FontId::monospace(10.0)).color(pal.muted));
                            }
                        });
                    }
                    if doc_deg.len() > 24 {
                        ui.label(RichText::new(format!("… and {} more", doc_deg.len() - 24)).color(pal.muted).size(10.5));
                    }
                });
            });
        if !open {
            closed.push(key.clone());
        }
    }
    app.estate_popouts.retain(|k| !closed.contains(k));
}

fn edge_inspector(app: &mut App, ctx: &egui::Context) {
    let pal = app.pal;
    let Some(ei) = app.estate_edge_sel else { return };
    let (Some(g), Some(fg)) = (app.estate.clone(), app.estate_folders.clone()) else { return };
    let Some(e) = fg.edges.get(ei) else {
        app.estate_edge_sel = None;
        return;
    };
    let (fa, fb) = (&fg.folders[e.a], &fg.folders[e.b]);
    let mutual = e.n_ab > 0 && e.n_ba > 0;
    let title = format!("{} {} {}", fa.label, if mutual { "⟷" } else { "→" }, fb.label);
    let mut open = true;
    egui::Window::new(RichText::new(title).strong())
        .id(egui::Id::new("estate-edge"))
        .open(&mut open)
        .resizable(true)
        .default_width(460.0)
        .default_height(320.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!(
                    "{} → {}: {} refs   ·   {} → {}: {} refs",
                    fa.label, fb.label, e.n_ab, fb.label, fa.label, e.n_ba
                ))
                .font(FontId::monospace(11.5))
                .color(pal.ink_2),
            );
            ui.separator();
            let id_to_dense: std::collections::HashMap<i64, usize> =
                g.docs.iter().enumerate().map(|(di, d)| (d.id, di)).collect();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let mut rows: Vec<(String, String, u32, String)> = Vec::new();
                for l in &g.links {
                    let (Some(&si), Some(&ti)) = (id_to_dense.get(&l.src), id_to_dense.get(&l.dst)) else {
                        continue;
                    };
                    let (pa, pb) = (fg.doc_folder[si], fg.doc_folder[ti]);
                    if (pa == e.a && pb == e.b) || (pa == e.b && pb == e.a) {
                        let name = |p: &str| {
                            std::path::Path::new(p)
                                .file_name()
                                .map(|x| x.to_string_lossy().into_owned())
                                .unwrap_or_else(|| p.to_string())
                        };
                        rows.push((name(&g.docs[si].path), name(&g.docs[ti].path), l.n, g.docs[si].path.clone()));
                    }
                }
                rows.sort_by(|a, b| b.2.cmp(&a.2));
                for (from, to, n, src_path) in rows.iter().take(60) {
                    ui.horizontal(|ui| {
                        let r = ui.link(RichText::new(trunc(from, 30)).font(FontId::monospace(11.0)));
                        if r.on_hover_text(src_path).clicked() {
                            let _ = crate::util::xdg_open(&std::path::PathBuf::from(src_path));
                        }
                        ui.label(RichText::new("→").color(pal.muted).size(11.0));
                        ui.label(RichText::new(trunc(to, 30)).font(FontId::monospace(11.0)).color(pal.ink_2));
                        ui.label(RichText::new(format!("{n}")).font(FontId::monospace(10.0)).color(pal.muted));
                    });
                }
            });
        });
    if !open {
        app.estate_edge_sel = None;
    }
}
