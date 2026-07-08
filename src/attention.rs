// SPDX-License-Identifier: GPL-3.0-or-later
//! The attention index: which .md/.html docs point at which, estate-wide.
//! A deterministic scanner (no LLM) walks the estate roots, extracts
//! references (markdown links, [[wikilinks]], href/src, bare ~ paths) and
//! stores the doc graph in SQLite at ~/.local/share/concourse/attention.db.
//! Agents refresh it per docs/ATTENTION-WORKFLOW.md; the GUI's estate map
//! renders it. Archived material is never indexed (Ben's law).

use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── the roots ───────────────────────────────────────────────────────────────
// label · path (~-relative) · agent color-home · granular (children become
// folder boxes) — grey covers opencode and misc/unknown alike.

pub struct Root {
    pub label: &'static str,
    pub rel: &'static str,
    pub agent: &'static str, // "claude" | "nexus" | "opencode"
    pub granular: bool,
}

pub const ROOTS: &[Root] = &[
    Root { label: "workspace", rel: "Dev/ClaudeWorkspace", agent: "claude", granular: true },
    Root { label: "nexus", rel: "Nexus", agent: "nexus", granular: true },
    Root { label: "claude-skills", rel: ".claude/skills", agent: "claude", granular: false },
    Root { label: "claude-md", rel: ".claude/CLAUDE.md", agent: "claude", granular: false },
    Root { label: "hermes-skills", rel: ".hermes/skills", agent: "nexus", granular: false },
    Root { label: "soul", rel: ".hermes/SOUL.md", agent: "nexus", granular: false },
    Root { label: "opencode", rel: ".config/opencode", agent: "opencode", granular: false },
];

const SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", "target", "zig-out", "dist", "build", "__pycache__",
    ".venv", "venv", ".cache", ".next", "vendor",
];
const MAX_FILE_BYTES: u64 = 2_000_000;

fn dir_skipped(name: &str) -> bool {
    let l = name.to_lowercase();
    // never index archived material — Ben's law for the attention map
    l.contains("archive") || SKIP_DIRS.contains(&l.as_str()) || l.starts_with('.')
}

fn is_doc(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("md") | Some("html") | Some("htm")
    )
}

// ── storage ─────────────────────────────────────────────────────────────────

pub fn db_path() -> PathBuf {
    crate::util::data_dir().join("attention.db")
}

fn open_db() -> Result<Connection, String> {
    let path = db_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let conn = Connection::open(&path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS docs (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            root TEXT NOT NULL,
            agent TEXT NOT NULL,
            mtime INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS links (
            src INTEGER NOT NULL REFERENCES docs(id),
            dst INTEGER NOT NULL REFERENCES docs(id),
            n INTEGER NOT NULL,
            PRIMARY KEY (src, dst)
        );
        CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

// ── the scan ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ScanStats {
    pub docs: usize,
    pub link_pairs: usize,
    pub refs_total: u64,
    pub elapsed_ms: u64,
    pub db: String,
    pub scanned_at: String,
}

struct DocMeta {
    path: PathBuf,
    root: &'static str,
    agent: &'static str,
    mtime: i64,
}

pub fn scan() -> Result<ScanStats, String> {
    let started = std::time::Instant::now();
    let home = crate::util::home();

    // pass 1 — enumerate every doc under the roots
    let mut docs: Vec<DocMeta> = Vec::new();
    for root in ROOTS {
        let base = home.join(root.rel);
        if base.is_file() {
            if let Ok(meta) = std::fs::metadata(&base) {
                docs.push(DocMeta {
                    path: base.clone(),
                    root: root.label,
                    agent: root.agent,
                    mtime: mtime_secs(&meta),
                });
            }
            continue;
        }
        if base.is_dir() {
            walk_docs(&base, root, &mut docs);
        }
    }
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    docs.dedup_by(|a, b| a.path == b.path);

    // lookup tables: full path → index, lowercase stem → indices (wikilinks)
    let mut by_path: HashMap<PathBuf, usize> = HashMap::new();
    let mut by_stem: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, d) in docs.iter().enumerate() {
        by_path.insert(d.path.clone(), i);
        if let Some(stem) = d.path.file_stem().and_then(|s| s.to_str()) {
            by_stem.entry(stem.to_lowercase()).or_default().push(i);
        }
    }

    // pass 2 — read each doc, resolve its references
    let mut pairs: HashMap<(usize, usize), u32> = HashMap::new();
    let mut refs_total: u64 = 0;
    for (i, d) in docs.iter().enumerate() {
        let Ok(text) = std::fs::read_to_string(&d.path) else { continue };
        let src_dir = d.path.parent().unwrap_or(Path::new("/")).to_path_buf();
        for cand in extract_candidates(&text) {
            let target = match cand {
                Candidate::Path(s) => resolve_path(&s, &src_dir, &by_path),
                Candidate::Wiki(s) => resolve_wiki(&s, i, &docs, &by_stem),
            };
            if let Some(j) = target {
                if j != i {
                    *pairs.entry((i, j)).or_insert(0) += 1;
                    refs_total += 1;
                }
            }
        }
    }

    // persist — full rebuild in one transaction (the estate is small enough)
    let mut conn = open_db()?;
    let scanned_at = crate::util::now_ts();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch("DELETE FROM links; DELETE FROM docs;").map_err(|e| e.to_string())?;
    {
        let mut ins = tx
            .prepare("INSERT INTO docs (id, path, root, agent, mtime) VALUES (?1, ?2, ?3, ?4, ?5)")
            .map_err(|e| e.to_string())?;
        for (i, d) in docs.iter().enumerate() {
            ins.execute(rusqlite::params![
                i as i64,
                d.path.to_string_lossy(),
                d.root,
                d.agent,
                d.mtime
            ])
            .map_err(|e| e.to_string())?;
        }
        let mut insl = tx
            .prepare("INSERT INTO links (src, dst, n) VALUES (?1, ?2, ?3)")
            .map_err(|e| e.to_string())?;
        for ((a, b), n) in &pairs {
            insl.execute(rusqlite::params![*a as i64, *b as i64, *n]).map_err(|e| e.to_string())?;
        }
    }
    tx.execute(
        "INSERT INTO meta (key, value) VALUES ('scanned_at', ?1)
         ON CONFLICT(key) DO UPDATE SET value = ?1",
        [&scanned_at],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(ScanStats {
        docs: docs.len(),
        link_pairs: pairs.len(),
        refs_total,
        elapsed_ms: started.elapsed().as_millis() as u64,
        db: db_path().to_string_lossy().into_owned(),
        scanned_at,
    })
}

fn mtime_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn walk_docs(dir: &Path, root: &Root, out: &mut Vec<DocMeta>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if !dir_skipped(&name) {
                walk_docs(&p, root, out);
            }
        } else if is_doc(&p) {
            let Ok(meta) = e.metadata() else { continue };
            if meta.len() > MAX_FILE_BYTES {
                continue;
            }
            out.push(DocMeta { path: p, root: root.label, agent: root.agent, mtime: mtime_secs(&meta) });
        }
    }
}

// ── reference extraction ────────────────────────────────────────────────────

enum Candidate {
    Path(String), // something path-shaped (md link target, href, bare ~/…)
    Wiki(String), // [[wikilink]] body
}

/// Pull every reference-shaped string out of a doc. Structured patterns are
/// masked so the bare-path sweep doesn't double-count the same reference.
fn extract_candidates(text: &str) -> Vec<Candidate> {
    let bytes = text.as_bytes();
    let mut masked = vec![false; bytes.len()];
    let mut out = Vec::new();
    let mask = |from: usize, to: usize, m: &mut Vec<bool>| {
        for slot in m.iter_mut().take(to.min(bytes.len())).skip(from) {
            *slot = true;
        }
    };

    // [[wikilinks]]
    let mut i = 0;
    while let Some(rel) = text[i..].find("[[") {
        let start = i + rel + 2;
        match text[start..].find("]]") {
            Some(endrel) => {
                let end = start + endrel;
                let body = &text[start..end];
                let name = body.split(['|', '#']).next().unwrap_or("").trim();
                if !name.is_empty() && name.len() < 200 {
                    out.push(Candidate::Wiki(name.to_string()));
                }
                mask(i + rel, end + 2, &mut masked);
                i = end + 2;
            }
            None => break,
        }
    }

    // markdown links: ](target)
    let mut i = 0;
    while let Some(rel) = text[i..].find("](") {
        let start = i + rel + 2;
        match text[start..].find(')') {
            Some(endrel) => {
                let end = start + endrel;
                let target = text[start..end].split_whitespace().next().unwrap_or("");
                if !target.is_empty() && target.len() < 500 {
                    out.push(Candidate::Path(target.to_string()));
                }
                mask(i + rel, end + 1, &mut masked);
                i = end + 1;
            }
            None => break,
        }
    }

    // href="…" / src="…" (html)
    for attr in ["href=", "src="] {
        let mut i = 0;
        while let Some(rel) = text[i..].find(attr) {
            let qpos = i + rel + attr.len();
            let Some(q) = text.as_bytes().get(qpos).copied() else { break };
            if q == b'"' || q == b'\'' {
                let start = qpos + 1;
                if let Some(endrel) = text[start..].find(q as char) {
                    let end = start + endrel;
                    let target = &text[start..end];
                    if !target.is_empty() && target.len() < 500 {
                        out.push(Candidate::Path(target.to_string()));
                    }
                    mask(qpos, end + 1, &mut masked);
                    i = end + 1;
                    continue;
                }
            }
            i = qpos;
        }
    }

    // bare home paths: ~/… or /home/… mentioned in prose/code spans
    for needle in ["~/", "/home/"] {
        let mut i = 0;
        while let Some(rel) = text[i..].find(needle) {
            let start = i + rel;
            if masked[start] {
                i = start + needle.len();
                continue;
            }
            let stop = " \t\n\r\"'`<>|)],;*".as_bytes();
            let mut end = start;
            for (off, ch) in text[start..].char_indices() {
                if ch.is_ascii() && stop.contains(&(ch as u8)) {
                    break;
                }
                end = start + off + ch.len_utf8();
            }
            let raw = text[start..end].trim_end_matches(['.', ':', '!', '?']);
            if raw.len() > needle.len() && raw.len() < 500 {
                out.push(Candidate::Path(raw.to_string()));
            }
            i = end.max(start + needle.len());
        }
    }
    out
}

/// tiny %XX decode — html hrefs to files with spaces/emoji arrive encoded
fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn resolve_path(cand: &str, src_dir: &Path, by_path: &HashMap<PathBuf, usize>) -> Option<usize> {
    let mut s = cand.trim().trim_matches(['<', '>']).to_string();
    if let Some(rest) = s.strip_prefix("file://") {
        s = rest.to_string();
    }
    // outbound schemes and anchors aren't estate references
    if s.contains("://") || s.starts_with("mailto:") || s.starts_with("data:") || s.starts_with('#') {
        return None;
    }
    let s = s.split(['#', '?']).next().unwrap_or("").trim();
    if s.is_empty() {
        return None;
    }
    let s = percent_decode(s);
    let p = if let Some(rest) = s.strip_prefix("~/") {
        crate::util::home().join(rest)
    } else if s.starts_with('/') {
        PathBuf::from(&s)
    } else {
        src_dir.join(&s)
    };
    if !is_doc(&p) {
        return None;
    }
    let canon = p.canonicalize().ok()?;
    by_path.get(&canon).copied()
}

fn resolve_wiki(
    name: &str,
    src_idx: usize,
    docs: &[DocMeta],
    by_stem: &HashMap<String, Vec<usize>>,
) -> Option<usize> {
    let hits = by_stem.get(&name.to_lowercase())?;
    if hits.is_empty() {
        return None;
    }
    // prefer a target under the same root; then the shortest path (deterministic)
    let src_root = docs[src_idx].root;
    let mut best: Option<usize> = None;
    for &h in hits {
        if h == src_idx {
            continue;
        }
        best = Some(match best {
            None => h,
            Some(b) => {
                let (hb, bb) = (docs[h].root == src_root, docs[b].root == src_root);
                if hb != bb {
                    if hb { h } else { b }
                } else if docs[h].path.as_os_str().len() < docs[b].path.as_os_str().len() {
                    h
                } else {
                    b
                }
            }
        });
    }
    best
}

// ── reading the graph back ──────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct DocRow {
    pub id: i64,
    pub path: String,
    pub root: String,
    pub agent: String,
}

#[derive(Serialize, Clone)]
pub struct LinkRow {
    pub src: i64,
    pub dst: i64,
    pub n: u32,
}

#[derive(Serialize, Clone)]
pub struct GraphData {
    pub scanned_at: String,
    pub docs: Vec<DocRow>,
    pub links: Vec<LinkRow>,
}

pub fn load_graph() -> Result<GraphData, String> {
    if !db_path().is_file() {
        return Err(format!(
            "no attention index at {} — run `concourse attention scan`",
            db_path().display()
        ));
    }
    let conn = open_db()?;
    let scanned_at: String = conn
        .query_row("SELECT value FROM meta WHERE key = 'scanned_at'", [], |r| r.get(0))
        .unwrap_or_default();
    let mut docs = Vec::new();
    let mut st = conn.prepare("SELECT id, path, root, agent FROM docs").map_err(|e| e.to_string())?;
    let rows = st
        .query_map([], |r| {
            Ok(DocRow { id: r.get(0)?, path: r.get(1)?, root: r.get(2)?, agent: r.get(3)? })
        })
        .map_err(|e| e.to_string())?;
    for r in rows.flatten() {
        docs.push(r);
    }
    let mut links = Vec::new();
    let mut st = conn.prepare("SELECT src, dst, n FROM links").map_err(|e| e.to_string())?;
    let rows = st
        .query_map([], |r| Ok(LinkRow { src: r.get(0)?, dst: r.get(1)?, n: r.get(2)? }))
        .map_err(|e| e.to_string())?;
    for r in rows.flatten() {
        links.push(r);
    }
    Ok(GraphData { scanned_at, docs, links })
}

// ── folder-level aggregation (shared by the CLI and the estate map) ────────

#[derive(Serialize, Clone)]
pub struct FolderNode {
    pub key: String,   // "workspace/concourse", "nexus", "claude-skills", …
    pub label: String, // what the box says
    pub above: Option<String>, // parent folder's label, for subdir boxes
    pub path: String,  // absolute dir (terminal/open buttons)
    pub root: String,
    pub agent: String,
    pub depth: u8, // 0 root · 1 bay · 2 subdir
    pub docs: usize,
}

#[derive(Serialize, Clone)]
pub struct FolderEdge {
    pub a: usize, // index into the folder list
    pub b: usize,
    pub n_ab: u32, // references a → b
    pub n_ba: u32, // references b → a (mutual when both > 0)
}

#[derive(Clone)]
pub struct FolderGraph {
    pub folders: Vec<FolderNode>,
    pub edges: Vec<FolderEdge>,
    pub doc_folder: Vec<usize>, // doc id (dense) → folder index
}

/// Map each doc to a display folder. Granular roots (workspace, nexus) split
/// into their child dirs — the bays; with `subdirs` on, one level deeper.
pub fn folder_graph(g: &GraphData, subdirs: bool) -> FolderGraph {
    let home = crate::util::home();
    let mut folders: Vec<FolderNode> = Vec::new();
    let mut folder_of: HashMap<String, usize> = HashMap::new();
    let mut doc_folder: Vec<usize> = vec![usize::MAX; g.docs.len()];

    let root_by_label: HashMap<&str, &Root> = ROOTS.iter().map(|r| (r.label, r)).collect();

    for (di, doc) in g.docs.iter().enumerate() {
        let Some(root) = root_by_label.get(doc.root.as_str()) else { continue };
        let base = home.join(root.rel);
        let path = PathBuf::from(&doc.path);
        let rel = path.strip_prefix(&base).unwrap_or(&path);
        let comps: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();

        // choose the folder this doc displays under
        let (key, label, above, dir, depth) = if !root.granular || comps.len() <= 1 {
            (
                doc.root.clone(),
                root_display(root),
                None,
                if base.is_file() { base.parent().unwrap_or(&base).to_path_buf() } else { base.clone() },
                0u8,
            )
        } else if subdirs && comps.len() > 2 {
            let bay = &comps[0];
            let sub = &comps[1];
            (
                format!("{}/{}/{}", doc.root, bay, sub),
                sub.clone(),
                Some(bay.clone()),
                base.join(bay).join(sub),
                2,
            )
        } else {
            let bay = &comps[0];
            (format!("{}/{}", doc.root, bay), bay.clone(), None, base.join(bay), 1)
        };

        let idx = *folder_of.entry(key.clone()).or_insert_with(|| {
            folders.push(FolderNode {
                key,
                label,
                above,
                path: dir.to_string_lossy().into_owned(),
                root: doc.root.clone(),
                agent: doc.agent.clone(),
                depth,
                docs: 0,
            });
            folders.len() - 1
        });
        folders[idx].docs += 1;
        doc_folder[di] = idx;
    }

    // aggregate doc links to folder pairs (drop intra-folder chatter)
    let id_to_dense: HashMap<i64, usize> = g.docs.iter().enumerate().map(|(i, d)| (d.id, i)).collect();
    let mut acc: HashMap<(usize, usize), u32> = HashMap::new();
    for l in &g.links {
        let (Some(&si), Some(&ti)) = (id_to_dense.get(&l.src), id_to_dense.get(&l.dst)) else {
            continue;
        };
        let (fa, fb) = (doc_folder[si], doc_folder[ti]);
        if fa == usize::MAX || fb == usize::MAX || fa == fb {
            continue;
        }
        *acc.entry((fa, fb)).or_insert(0) += l.n;
    }
    let mut edges: Vec<FolderEdge> = Vec::new();
    let mut seen: HashMap<(usize, usize), usize> = HashMap::new();
    for ((a, b), n) in acc {
        let (lo, hi, fwd) = if a < b { (a, b, true) } else { (b, a, false) };
        match seen.get(&(lo, hi)) {
            Some(&ei) => {
                if fwd {
                    edges[ei].n_ab += n;
                } else {
                    edges[ei].n_ba += n;
                }
            }
            None => {
                seen.insert((lo, hi), edges.len());
                edges.push(FolderEdge {
                    a: lo,
                    b: hi,
                    n_ab: if fwd { n } else { 0 },
                    n_ba: if fwd { 0 } else { n },
                });
            }
        }
    }
    edges.sort_by(|x, y| (y.n_ab + y.n_ba).cmp(&(x.n_ab + x.n_ba)));
    FolderGraph { folders, edges, doc_folder }
}

fn root_display(r: &Root) -> String {
    match r.label {
        "workspace" => "ClaudeWorkspace".into(),
        "nexus" => "Nexus".into(),
        "claude-skills" => "claude · skills".into(),
        "claude-md" => "CLAUDE.md".into(),
        "hermes-skills" => "hermes · skills".into(),
        "soul" => "SOUL.md".into(),
        "opencode" => "opencode".into(),
        other => other.into(),
    }
}
