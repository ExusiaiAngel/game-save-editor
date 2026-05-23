# GameSaveEditor GUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add egui-based GUI with dual-panel layout for save file editing and real-time memory modification.

**Architecture:** New `crates/gui` crate with eframe/egui frontend. Reuses all existing engine crates and core traits unchanged. TCP bridge moved to background thread with channel-based communication to keep UI responsive. Engine-adaptive panels use `SavePanelMode` enum to switch between RPG Maker tree, Ren'Py meta, Unreal read-only, and Generic flat views.

**Tech Stack:** Rust (edition 2021), eframe 0.31, egui 0.31, rfd 0.15, std::sync::mpsc channels

**Spec:** `docs/superpowers/specs/2026-05-22-game-tool-gui-design.md`

**File Structure:**
```
crates/gui/
├── Cargo.toml
└── src/
    ├── main.rs              # eframe entry point + startup folder dialog
    ├── app.rs               # AppState builder, eframe::App impl, dual-panel layout, orchestration
    ├── state.rs             # AppState, SavePanelState, RtPanelState, enums, channel types
    ├── factory.rs           # create_format(), create_bridge(), is_readonly(), supports_realtime(), game_state_to_fields()
    ├── discovery.rs         # find_save_files()
    ├── connection.rs        # spawn_bridge_thread(), drain_results()
    ├── panels/
    │   ├── mod.rs           # panel module
    │   ├── top_bar.rs       # top bar: game dir, engine, switch button
    │   ├── save_panel.rs    # left panel: save selector, summary, search, category tree, field table, save button
    │   └── realtime_panel.rs # right panel: connection, plugin, compact/full view
    └── widgets/
        ├── mod.rs           # widget module
        ├── search_bar.rs    # inline search text input
        ├── summary_card.rs  # SaveSummary card display
        ├── field_table.rs   # scrollable editable field table
        └── category_tree.rs # RPG Maker category tree
```

**Modified existing files:**
- `Cargo.toml` — add `crates/gui` member, add eframe/egui/rfd workspace deps
- `crates/app/Cargo.toml` — rename binary to `game-tool-cli`
- `build.bat` — build gui crate instead of workspace

---

### Task 1: Rename CLI binary + Add workspace deps

**Files:**
- Modify: `crates/app/Cargo.toml`
- Modify: `Cargo.toml`

- [ ] **Step: Rename CLI binary in app Cargo.toml**

Change line 8 in `crates/app/Cargo.toml`:
```toml
name = "game-tool-cli"
```

- [ ] **Step: Add gui member and deps to workspace Cargo.toml**

Add `"crates/gui"` to the `members` array, add `eframe`, `egui`, `rfd` to `[workspace.dependencies]`:
```toml
[workspace]
resolver = "2"
members = [
    "crates/core",
    "crates/engines/rpgmaker",
    "crates/engines/renpy",
    "crates/engines/unreal",
    "crates/engines/generic",
    "crates/app",
    "crates/gui",
]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
zip = "2"
walkdir = "2"
tempfile = "3"
lz-str = "0.2"
toml = "0.8"
chrono = "0.4"
eframe = "0.31"
egui = "0.31"
rfd = { version = "0.15", default-features = false, features = ["file-handle-inner"] }
```

---

### Task 2: Create gui crate with Cargo.toml and empty module structure

**Files:**
- Create: `crates/gui/Cargo.toml`
- Create: `crates/gui/src/main.rs` (placeholder)
- Create: `crates/gui/src/state.rs` (placeholder)
- Create: `crates/gui/src/panels/mod.rs`
- Create: `crates/gui/src/widgets/mod.rs`

- [ ] **Step: Create crate directories**

Run: `New-Item -ItemType Directory -Force -Path "crates/gui/src/panels", "crates/gui/src/widgets"`

- [ ] **Step: Write `crates/gui/Cargo.toml`**

```toml
[package]
name = "game-tool-gui"
version = "0.1.0"
edition = "2021"
description = "Game Save Editor — GUI"

[[bin]]
name = "GameSaveEditor"
path = "src/main.rs"

[dependencies]
eframe.workspace = true
egui.workspace = true
rfd.workspace = true
serde_json.workspace = true
serde.workspace = true

game-tool-core = { path = "../core" }
game-tool-rpgmaker = { path = "../engines/rpgmaker" }
game-tool-renpy = { path = "../engines/renpy" }
game-tool-unreal = { path = "../engines/unreal" }
game-tool-generic = { path = "../engines/generic" }
```

- [ ] **Step: Write placeholder files**

Write `crates/gui/src/panels/mod.rs`:
```rust
pub mod top_bar;
pub mod save_panel;
pub mod realtime_panel;
```

Write `crates/gui/src/widgets/mod.rs`:
```rust
pub mod search_bar;
pub mod summary_card;
pub mod field_table;
pub mod category_tree;
```

---

### Task 3: state.rs — All state types, enums, and channel message types

**Files:**
- Create: `crates/gui/src/state.rs`

- [ ] **Step: Write the complete file**

```rust
use game_tool_core::{ISaveFormat, ModifiableField, SaveSummary, AppConfig, BridgeCommand};
use game_tool_core::detector::EngineType;
use serde_json::Value;
use std::sync::mpsc::{Sender, Receiver};

#[derive(Clone, Copy, PartialEq)]
pub enum SavePanelMode {
    RpgMaker,
    RenPy,
    Unreal,
    Generic,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RtViewMode {
    Compact,
    Full,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

pub enum BridgeJob {
    Connect { host: String, port: u16 },
    Disconnect,
    Execute(BridgeCommand),
}

pub enum BridgeResult {
    Connected,
    Disconnected,
    CommandResult(Value),
    Error(String),
}

pub struct RealtimeConnection {
    pub cmd_tx: Sender<BridgeJob>,
    pub result_rx: Receiver<BridgeResult>,
    pub status: ConnectionStatus,
}

pub struct SavePanelState {
    pub format: Option<Box<dyn ISaveFormat>>,
    pub save_files: Vec<String>,
    pub selected_save: Option<String>,
    pub save_data: Option<Value>,
    pub summary: Option<SaveSummary>,
    pub fields: Vec<ModifiableField>,
    pub dirty_count: usize,
    pub selected_category: Option<String>,
    pub search_query: String,
    pub panel_mode: SavePanelMode,
    pub readonly: bool,
}

pub struct RtPanelState {
    pub conn: Option<RealtimeConnection>,
    pub fields: Vec<ModifiableField>,
    pub view_mode: RtViewMode,
    pub plugin_installed: bool,
    pub host: String,
    pub port: u16,
    pub pending_connect: bool,
    pub pending_disconnect: bool,
    pub error_message: String,
}

pub struct AppState {
    pub game_dir: Option<String>,
    pub game_title: String,
    pub engine: EngineType,
    pub config: AppConfig,
    pub save_panel: SavePanelState,
    pub rt_panel: RtPanelState,
    pub status_message: String,
    pub show_unsaved_dialog: bool,
    pub pending_game_switch: bool,
}
```

---

### Task 4: factory.rs — Format/Bridge factories + utility functions

**Files:**
- Create: `crates/gui/src/factory.rs`

- [ ] **Step: Write the complete file**

```rust
use game_tool_core::{ISaveFormat, GameBridge, ModifiableField, GameState};
use game_tool_core::detector::EngineType;
use game_tool_rpgmaker::format::RpgMakerFormat;
use game_tool_rpgmaker::tcp::RpgMakerTcpBridge;
use game_tool_renpy::format::RenPyFormat;
use game_tool_renpy::bridge::RenPyBridge;
use game_tool_unreal::format::UnrealGVASFormat;
use game_tool_generic::format::GenericJsonFormat;
use serde_json::Value;
use crate::state::SavePanelMode;

pub fn create_format(engine: &EngineType) -> Option<Box<dyn ISaveFormat>> {
    match engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs =>
            Some(Box::new(RpgMakerFormat::new())),
        EngineType::RenPy =>
            Some(Box::new(RenPyFormat::new())),
        EngineType::Unreal =>
            Some(Box::new(UnrealGVASFormat::new())),
        EngineType::UnityMono | EngineType::UnityIl2Cpp | EngineType::Godot =>
            Some(Box::new(GenericJsonFormat::new())),
        EngineType::Unknown => None,
    }
}

pub fn create_bridge(_engine: &EngineType, host: &str, port: u16) -> Option<Box<dyn GameBridge>> {
    match _engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs =>
            Some(Box::new(RpgMakerTcpBridge::new(host, port))),
        EngineType::RenPy =>
            Some(Box::new(RenPyBridge::new(host, port))),
        _ => None,
    }
}

pub fn is_readonly(engine: &EngineType) -> bool {
    matches!(engine, EngineType::Unreal)
}

pub fn supports_realtime(engine: &EngineType) -> bool {
    matches!(engine,
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs | EngineType::RenPy)
}

pub fn engine_to_panel_mode(engine: &EngineType) -> SavePanelMode {
    match engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs =>
            SavePanelMode::RpgMaker,
        EngineType::RenPy =>
            SavePanelMode::RenPy,
        EngineType::Unreal =>
            SavePanelMode::Unreal,
        _ => SavePanelMode::Generic,
    }
}

pub fn game_state_to_fields(state: &GameState, engine: &EngineType) -> Vec<ModifiableField> {
    match engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs =>
            rpgmaker_state_to_fields(state),
        EngineType::RenPy =>
            renpy_state_to_fields(state),
        _ => vec![],
    }
}

fn rpgmaker_state_to_fields(state: &GameState) -> Vec<ModifiableField> {
    let mut fields = Vec::new();
    let ext = &state.extensions;

    if let Some(gold) = ext.get("gold").and_then(|v| v.as_i64()) {
        fields.push(ModifiableField {
            category: "gold".into(), field_id: "gold".into(),
            display_name: "金币".into(), field_type: "int".into(),
            live_value: Value::Number(gold.into()),
            min_val: 0, max_val: 99_999_999,
            ..Default::default()
        });
    }

    if let Some(switches) = ext.get("switches").and_then(|v| v.as_object()) {
        for (k, val) in switches {
            if let Ok(i) = k.parse::<i32>() {
                fields.push(ModifiableField {
                    category: "switch".into(),
                    field_id: format!("switch_{}", i),
                    display_name: format!("开关 #{}", i),
                    item_id: i, field_type: "bool".into(),
                    live_value: val.clone(),
                    min_val: 0, max_val: 1,
                    ..Default::default()
                });
            }
        }
    }

    if let Some(vars) = ext.get("variables").and_then(|v| v.as_object()) {
        for (k, val) in vars {
            if let Ok(i) = k.parse::<i32>() {
                let v = val.as_i64().unwrap_or(0) as i32;
                fields.push(ModifiableField {
                    category: "variable".into(),
                    field_id: format!("var_{}", i),
                    display_name: format!("变量 #{}", i),
                    item_id: i, field_type: "int".into(),
                    live_value: Value::Number(v.into()),
                    min_val: -9_999_999, max_val: 99_999_999,
                    ..Default::default()
                });
            }
        }
    }

    if let Some(items) = ext.get("items").and_then(|v| v.as_object()) {
        for (k, count) in items {
            if let Ok(i) = k.parse::<i32>() {
                let c = count.as_i64().unwrap_or(0) as i32;
                if c > 0 {
                    fields.push(ModifiableField {
                        category: "item".into(),
                        field_id: format!("item_{}", i),
                        display_name: format!("物品 #{}", i),
                        item_id: i, field_type: "int".into(),
                        live_value: Value::Number(c.into()),
                        min_val: 0, max_val: 999,
                        ..Default::default()
                    });
                }
            }
        }
    }

    if let Some(party) = ext.get("party").and_then(|v| v.as_array()) {
        for actor in party {
            let id = actor.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let hp = actor.get("_hp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let mp = actor.get("_mp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            fields.push(ModifiableField {
                category: "actor".into(), field_id: format!("actor_{}_hp", id),
                display_name: format!("角色 {} HP", id), item_id: id, field_type: "int".into(),
                live_value: Value::Number(hp.into()),
                min_val: 0, max_val: 999_999,
                ..Default::default()
            });
            fields.push(ModifiableField {
                category: "actor".into(), field_id: format!("actor_{}_mp", id),
                display_name: format!("角色 {} MP", id), item_id: id, field_type: "int".into(),
                live_value: Value::Number(mp.into()),
                min_val: 0, max_val: 999_999,
                ..Default::default()
            });
        }
    }

    fields
}

fn renpy_state_to_fields(state: &GameState) -> Vec<ModifiableField> {
    let mut fields = Vec::new();
    if let Some(store) = state.extensions.get("store").and_then(|v| v.as_object()) {
        for (key, val) in store {
            let field_type = match val {
                Value::Bool(_) => "bool",
                Value::Number(_) => "int",
                Value::String(_) => "str",
                _ => "str",
            };
            fields.push(ModifiableField {
                category: "store".into(),
                field_id: format!("var_{}", key),
                display_name: key.clone(),
                field_type: field_type.into(),
                live_value: val.clone(),
                ..Default::default()
            });
        }
    }
    fields
}
```

---

### Task 5: discovery.rs — Save file discovery

**Files:**
- Create: `crates/gui/src/discovery.rs`

- [ ] **Step: Write the complete file**

```rust
use std::fs;
use std::path::Path;
use game_tool_core::ISaveFormat;

pub fn find_save_files(game_dir: &str, format: &dyn ISaveFormat) -> Vec<String> {
    let exts = format.extensions();
    let mut files = Vec::new();

    let mut search_dirs = Vec::new();
    if let Some(d) = format.find_data_dir(game_dir) {
        search_dirs.push(d);
    }

    let base = Path::new(game_dir);
    for sub in &["www/save", "www/Save", "save", "Save", "saves", "game/saves", "Saved/SaveGames"] {
        let d = base.join(sub);
        if d.is_dir() {
            let s = d.to_string_lossy().to_string();
            if !search_dirs.contains(&s) {
                search_dirs.push(s);
            }
        }
    }

    for dir in &search_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.contains(".bak.") || name == "config.rpgsave" || name == "global.rpgsave" {
                        continue;
                    }
                    for ext in &exts {
                        if name.ends_with(ext.as_str()) {
                            files.push(path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    files.sort_by(|a, b| {
        let ma = fs::metadata(a).and_then(|m| m.modified()).ok();
        let mb = fs::metadata(b).and_then(|m| m.modified()).ok();
        mb.cmp(&ma)
    });

    files
}
```

---

### Task 6: connection.rs — Background thread TCP bridge with channels

**Files:**
- Create: `crates/gui/src/connection.rs`

- [ ] **Step: Write the complete file**

```rust
use std::sync::mpsc;
use std::thread;
use game_tool_core::detector::EngineType;
use crate::state::{BridgeJob, BridgeResult, ConnectionStatus, RealtimeConnection};
use crate::factory::create_bridge;

pub fn spawn_bridge_thread(engine_clone: EngineType, host: String, port: u16) -> RealtimeConnection {
    let (cmd_tx, cmd_rx) = mpsc::channel::<BridgeJob>();
    let (result_tx, result_rx) = mpsc::channel::<BridgeResult>();

    let host_clone = host.clone();
    thread::spawn(move || {
        let mut bridge: Option<Box<dyn game_tool_core::GameBridge>> = None;

        loop {
            match cmd_rx.recv() {
                Ok(BridgeJob::Connect { .. }) => {
                    let h = host_clone.clone();
                    if let Some(mut b) = create_bridge(&engine_clone, &h, port) {
                        match b.connect() {
                            Ok(()) => {
                                bridge = Some(b);
                                let _ = result_tx.send(BridgeResult::Connected);
                            }
                            Err(e) => {
                                let _ = result_tx.send(BridgeResult::Error(e.to_string()));
                            }
                        }
                    } else {
                        let _ = result_tx.send(BridgeResult::Error("该引擎不支持实时连接".into()));
                    }
                }
                Ok(BridgeJob::Disconnect) => {
                    if let Some(ref mut b) = bridge {
                        b.disconnect();
                    }
                    bridge = None;
                    let _ = result_tx.send(BridgeResult::Disconnected);
                }
                Ok(BridgeJob::Execute(cmd)) => {
                    match &mut bridge {
                        Some(b) => match b.execute(&cmd) {
                            Ok(val) => { let _ = result_tx.send(BridgeResult::CommandResult(val)); }
                            Err(e) => { let _ = result_tx.send(BridgeResult::Error(e.to_string())); }
                        },
                        None => { let _ = result_tx.send(BridgeResult::Error("未连接".into())); }
                    }
                }
                Err(_) => {
                    if let Some(ref mut b) = bridge { b.disconnect(); }
                    break;
                }
            }
        }
    });

    RealtimeConnection {
        cmd_tx,
        result_rx,
        status: ConnectionStatus::Disconnected,
    }
}

pub fn drain_results(conn: &mut RealtimeConnection) -> Vec<BridgeResult> {
    let mut results = Vec::new();
    while let Ok(r) = conn.result_rx.try_recv() {
        results.push(r);
    }
    results
}
```

---

### Task 7: Widgets

**Files:**
- Create: `crates/gui/src/widgets/search_bar.rs`
- Create: `crates/gui/src/widgets/summary_card.rs`
- Create: `crates/gui/src/widgets/field_table.rs`
- Create: `crates/gui/src/widgets/category_tree.rs`

- [ ] **Step: Write `widgets/search_bar.rs`**

```rust
use egui::Ui;

pub fn render(ui: &mut Ui, query: &mut String) {
    ui.horizontal(|ui| {
        ui.text_edit_singleline(query);
        if !query.is_empty() && ui.button("✕").clicked() {
            query.clear();
        }
    });
}
```

- [ ] **Step: Write `widgets/summary_card.rs`**

```rust
use egui::Ui;
use game_tool_core::SaveSummary;

pub fn render(ui: &mut Ui, summary: &SaveSummary) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.heading("存档摘要");
        ui.add_space(4.0);

        let time_str = format!(
            "{:02}:{:02}:{:02}",
            summary.play_time / 3600,
            (summary.play_time % 3600) / 60,
            summary.play_time % 60
        );

        ui.label(format!(
            "金币: {}  队伍: {}人  物品: {}种  存档次数: {}  时长: {}",
            summary.gold, summary.party_size, summary.item_count,
            summary.save_count, time_str,
        ));

        if !summary.members.is_empty() {
            let m: Vec<&str> = summary.members.iter()
                .filter(|s| !s.is_empty())
                .map(|s| s.as_str())
                .collect();
            if !m.is_empty() {
                ui.label(format!("队员: {}", m.join(", ")));
            }
        }
    });
}
```

- [ ] **Step: Write `widgets/field_table.rs`**

```rust
use egui::{Ui, ScrollArea, Color32};
use game_tool_core::ModifiableField;
use serde_json::Value;

pub fn render(
    ui: &mut Ui,
    fields: &mut [ModifiableField],
    readonly: bool,
    search_query: &str,
    selected_category: &Option<String>,
) -> usize {
    let mut dirty_count = 0;

    let indices: Vec<usize> = fields.iter().enumerate()
        .filter(|(_, f)| {
            if let Some(cat) = selected_category {
                if f.category != *cat { return false; }
            }
            if !search_query.is_empty() {
                let q = search_query.to_lowercase();
                return f.display_name.to_lowercase().contains(&q)
                    || f.field_id.to_lowercase().contains(&q);
            }
            true
        })
        .map(|(i, _)| i)
        .collect();

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("field_grid").striped(true).min_col_width(40.0).show(ui, |ui| {
            ui.strong("分类");
            ui.strong("名称");
            ui.strong("值");
            ui.end_row();

            for idx in indices {
                let f = &fields[idx];
                let fid = f.field_id.clone();
                let cat = f.category.clone();
                let dname = f.display_name.clone();
                let ftype = f.field_type.clone();
                let min = f.min_val;
                let max = f.max_val;

                ui.label(&cat);
                ui.label(&dname);

                if readonly {
                    let ds = value_display(&f.save_value);
                    ui.label(&ds);
                } else {
                    match ftype.as_str() {
                        "bool" => {
                            let mut v = f.save_value.as_bool().unwrap_or(false);
                            if ui.checkbox(&mut v, "").changed() {
                                fields[idx].save_value = Value::Bool(v);
                                fields[idx].dirty = true;
                            }
                        }
                        "int" => {
                            let mut v = f.save_value.as_i64().unwrap_or(0) as i32;
                            let range = (min.min(max) as f64)..=(max.max(min) as f64);
                            if ui.add(egui::DragValue::new(&mut v).clamp_range(range).speed(1)).changed() {
                                fields[idx].save_value = Value::Number(v.into());
                                fields[idx].dirty = true;
                            }
                        }
                        "float" => {
                            let mut v = f.save_value.as_f64().unwrap_or(0.0);
                            if ui.add(egui::DragValue::new(&mut v).speed(0.1)).changed() {
                                if let Some(n) = serde_json::Number::from_f64(v) {
                                    fields[idx].save_value = Value::Number(n);
                                    fields[idx].dirty = true;
                                }
                            }
                        }
                        _ => {
                            let mut v = f.save_value.as_str().unwrap_or("").to_string();
                            if ui.text_edit_singleline(&mut v).changed() {
                                fields[idx].save_value = Value::String(v);
                                fields[idx].dirty = true;
                            }
                        }
                    }
                }

                if fields[idx].dirty {
                    dirty_count += 1;
                    ui.colored_label(Color32::from_rgb(255, 200, 0), "*");
                }

                ui.end_row();
            }
        });
    });

    dirty_count
}

fn value_display(v: &Value) -> String {
    match v {
        Value::Null => "-".into(),
        Value::Bool(b) => if *b { "ON" } else { "OFF" }.into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}
```

- [ ] **Step: Write `widgets/category_tree.rs`**

```rust
use egui::Ui;
use std::collections::BTreeMap;
use game_tool_core::ModifiableField;

pub fn render(ui: &mut Ui, fields: &[ModifiableField], selected: &mut Option<String>) {
    let mut cats: BTreeMap<String, usize> = BTreeMap::new();
    for f in fields {
        *cats.entry(f.category.clone()).or_default() += 1;
    }

    ui.strong("分类");
    ui.add_space(4.0);

    if ui.selectable_label(selected.is_none(), format!("全部 ({})", fields.len())).clicked() {
        *selected = None;
    }

    let labels: Vec<(&str, &str)> = vec![
        ("gold", "金币"), ("switch", "开关"), ("variable", "变量"),
        ("actor", "角色"), ("item", "物品"), ("weapon", "武器"),
        ("armor", "防具"), ("self_switch", "自开关"), ("meta", "元数据"),
        ("gvas", "GVAS"), ("general", "通用"), ("store", "Store"),
    ];

    for (key, label) in &labels {
        if let Some(count) = cats.get(*key) {
            let text = format!("{} ({})", label, count);
            let is_sel = selected.as_deref() == Some(key);
            if ui.selectable_label(is_sel, text).clicked() {
                *selected = Some(key.to_string());
            }
        }
    }

    // Unknown categories
    for (cat, count) in &cats {
        if !labels.iter().any(|(k, _)| k == cat) {
            let text = format!("{} ({})", cat, count);
            let is_sel = selected.as_deref() == Some(cat.as_str());
            if ui.selectable_label(is_sel, text).clicked() {
                *selected = Some(cat.clone());
            }
        }
    }
}
```

---

### Task 8: Panels

**Files:**
- Create: `crates/gui/src/panels/top_bar.rs`
- Create: `crates/gui/src/panels/save_panel.rs`
- Create: `crates/gui/src/panels/realtime_panel.rs`

- [ ] **Step: Write `panels/top_bar.rs`**

```rust
use egui::Ui;
use game_tool_core::detector::EngineType;

pub fn render(ui: &mut Ui, game_dir: &Option<String>, game_title: &str, engine: &EngineType) -> bool {
    let mut switch = false;
    ui.horizontal(|ui| {
        let dir = game_dir.as_deref().unwrap_or("未选择游戏目录");
        ui.label(format!("游戏: {}", dir));

        let ename = match engine {
            EngineType::RpgMakerMv => "RPG Maker MV",
            EngineType::RpgMakerMz => "RPG Maker MZ",
            EngineType::NwJs => "NW.js",
            EngineType::RenPy => "Ren'Py",
            EngineType::Unreal => "Unreal",
            EngineType::UnityMono => "Unity (Mono)",
            EngineType::UnityIl2Cpp => "Unity (IL2CPP)",
            EngineType::Godot => "Godot",
            EngineType::Unknown => "未知",
        };
        ui.label(format!("引擎: {}", ename));

        if !game_title.is_empty() {
            ui.label(format!("标题: {}", game_title));
        }

        ui.separator();
        if ui.button("切换游戏...").clicked() {
            switch = true;
        }
    });
    switch
}
```

- [ ] **Step: Write `panels/save_panel.rs`**

```rust
use egui::Ui;
use crate::state::SavePanelState;
use crate::widgets::{summary_card, search_bar, field_table, category_tree};

pub enum SaveAction {
    LoadSave,
    RefreshFiles,
    Save,
}

pub fn render(
    ui: &mut Ui,
    state: &mut SavePanelState,
) -> Vec<SaveAction> {
    let mut actions = Vec::new();

    ui.horizontal(|ui| {
        ui.label("存档文件:");
        let current = state.selected_save.as_ref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("未选择");

        egui::ComboBox::from_id_salt("save_combo")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for sf in &state.save_files {
                    let name = std::path::Path::new(sf)
                        .file_name().and_then(|n| n.to_str()).unwrap_or(sf);
                    let is_sel = state.selected_save.as_deref() == Some(sf.as_str());
                    if ui.selectable_label(is_sel, name).clicked() {
                        state.selected_save = Some(sf.clone());
                        actions.push(SaveAction::LoadSave);
                    }
                }
            });

        if ui.button("刷新").clicked() {
            actions.push(SaveAction::RefreshFiles);
        }
    });

    ui.separator();

    if let Some(ref summary) = state.summary {
        summary_card::render(ui, summary);
        ui.separator();
    }

    search_bar::render(ui, &mut state.search_query);

    match state.panel_mode {
        crate::state::SavePanelMode::RpgMaker => {
            ui.horizontal(|ui| {
                egui::ScrollArea::vertical().max_width(160.0).show(ui, |ui| {
                    category_tree::render(ui, &state.fields, &mut state.selected_category);
                });
                ui.separator();
                ui.vertical(|ui| {
                    state.dirty_count = field_table::render(
                        ui, &mut state.fields, state.readonly,
                        &state.search_query, &state.selected_category,
                    );
                });
            });
        }
        _ => {
            state.dirty_count = field_table::render(
                ui, &mut state.fields, state.readonly,
                &state.search_query, &state.selected_category,
            );
        }
    }

    ui.separator();

    ui.horizontal(|ui| {
        if state.readonly {
            ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "当前引擎仅供只读预览");
        } else {
            let text = if state.dirty_count > 0 {
                format!("保存更改 ({} 处)", state.dirty_count)
            } else {
                "保存更改".into()
            };
            if ui.add_enabled(state.dirty_count > 0 && state.save_data.is_some(),
                egui::Button::new(text)).clicked() {
                actions.push(SaveAction::Save);
            }

            if state.dirty_count > 0 {
                ui.colored_label(egui::Color32::from_rgb(255, 200, 0),
                    format!("{} 处未保存", state.dirty_count));
            }
        }
    });

    actions
}
```

- [ ] **Step: Write `panels/realtime_panel.rs`**

```rust
use egui::Ui;
use game_tool_core::detector::EngineType;
use game_tool_core::{BridgeCommand, ModifiableField};
use serde_json::Value;
use crate::state::{RtPanelState, RtViewMode, ConnectionStatus, BridgeJob};
use crate::factory;

pub enum RtAction {
    Connect,
    Disconnect,
    InjectPlugin,
    WriteField(String, Value),
    ReadAll,
}

pub fn render(
    ui: &mut Ui,
    state: &mut RtPanelState,
    engine: &EngineType,
    game_dir: &Option<String>,
) -> Vec<RtAction> {
    let mut actions = Vec::new();

    if !factory::supports_realtime(engine) {
        ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "该引擎暂不支持实时修改");
        return actions;
    }

    // Connection bar
    ui.horizontal(|ui| {
        let (text, color) = match state.conn.as_ref().map(|c| c.status) {
            Some(ConnectionStatus::Connected) => ("● 已连接", egui::Color32::GREEN),
            Some(ConnectionStatus::Connecting) => ("◌ 连接中...", egui::Color32::YELLOW),
            _ => ("○ 未连接", egui::Color32::RED),
        };
        ui.colored_label(color, text);

        ui.add(egui::DragValue::new(&mut state.port).clamp_range(1024..=65535).prefix("端口: "));

        let connected = state.conn.as_ref().map(|c| c.status == ConnectionStatus::Connected).unwrap_or(false);
        let connecting = state.conn.as_ref().map(|c| c.status == ConnectionStatus::Connecting).unwrap_or(false);

        if connected {
            if ui.button("断开").clicked() {
                actions.push(RtAction::Disconnect);
            }
        } else if connecting {
            ui.spinner();
        } else {
            if ui.add_enabled(state.plugin_installed, egui::Button::new("连接")).clicked() {
                actions.push(RtAction::Connect);
            }
        }
    });

    if !state.error_message.is_empty() {
        ui.colored_label(egui::Color32::RED, &state.error_message);
    }

    // Plugin
    if let Some(ref gd) = game_dir {
        ui.horizontal(|ui| {
            if state.plugin_installed {
                ui.colored_label(egui::Color32::GREEN, "✓ 插件已就绪");
            } else {
                ui.colored_label(egui::Color32::RED, "⚠ 插件未安装");
                if ui.button("注入插件").clicked() {
                    actions.push(RtAction::InjectPlugin);
                }
            }
        });
    } else {
        ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "请先选择游戏目录");
    }

    ui.separator();

    // View mode
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.view_mode, RtViewMode::Compact, "精简");
        ui.selectable_value(&mut state.view_mode, RtViewMode::Full, "完整");
        if ui.button("刷新数据").clicked() {
            actions.push(RtAction::ReadAll);
        }
    });

    ui.separator();

    // Fields
    let is_conn = state.conn.as_ref().map(|c| c.status == ConnectionStatus::Connected).unwrap_or(false);

    match state.view_mode {
        RtViewMode::Compact => {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for f in state.fields.iter_mut().take(20) {
                    ui.horizontal(|ui| {
                        ui.label(&f.display_name);
                        match f.field_type.as_str() {
                            "bool" => {
                                let mut v = f.live_value.as_bool().unwrap_or(false);
                                let resp = ui.checkbox(&mut v, "");
                                if resp.changed() && is_conn {
                                    let nv = Value::Bool(v);
                                    actions.push(RtAction::WriteField(f.field_id.clone(), nv));
                                }
                            }
                            "int" => {
                                let mut v = f.live_value.as_i64().unwrap_or(0) as i32;
                                let range = (f.min_val.min(f.max_val) as f64)..=(f.max_val.max(f.min_val) as f64);
                                let resp = ui.add(egui::DragValue::new(&mut v).clamp_range(range).speed(1));
                                if resp.changed() && is_conn {
                                    let nv = Value::Number(v.into());
                                    actions.push(RtAction::WriteField(f.field_id.clone(), nv));
                                }
                            }
                            _ => {
                                let ds = match &f.live_value {
                                    Value::Null => "-".into(),
                                    Value::Bool(b) => if *b { "ON" } else { "OFF" }.into(),
                                    Value::Number(n) => n.to_string(),
                                    Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                };
                                ui.label(&ds);
                            }
                        }
                    });
                }
            });
        }
        RtViewMode::Full => {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("rt_grid").striped(true).show(ui, |ui| {
                    ui.strong("分类");
                    ui.strong("名称");
                    ui.strong("当前值");
                    ui.end_row();
                    for f in &mut state.fields {
                        ui.label(&f.category);
                        ui.label(&f.display_name);
                        let ds = match &f.live_value {
                            Value::Null => "-".into(),
                            Value::Bool(b) => if *b { "ON" } else { "OFF" }.into(),
                            Value::Number(n) => n.to_string(),
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        ui.label(&ds);
                        ui.end_row();
                    }
                });
            });
        }
    }

    actions
}
```

---

### Task 9: app.rs — Main AppState initialization, eframe::App impl, panel orchestration

**Files:**
- Create: `crates/gui/src/app.rs`

- [ ] **Step: Write the complete file**

```rust
use game_tool_core::{ISaveFormat, AppConfig, BridgeCommand};
use game_tool_core::detector::{detect_by_filesystem, EngineType};
use game_tool_core::config::load_config;

use crate::state::{
    AppState, SavePanelState, RtPanelState, SavePanelMode,
    RtViewMode, ConnectionStatus, BridgeJob, BridgeResult,
};
use crate::factory::{self, create_format};
use crate::discovery;
use crate::connection;
use crate::panels::{top_bar, save_panel, realtime_panel};

impl AppState {
    pub fn new(game_dir: Option<String>) -> Self {
        let config = load_config().unwrap_or_default();
        let port = config.tcp_port;

        let (engine, game_title) = if let Some(ref dir) = game_dir {
            let eng = detect_by_filesystem(dir);
            let title = String::new(); // title populated after format loads
            (eng, title)
        } else {
            (EngineType::Unknown, String::new())
        };

        let panel_mode = factory::engine_to_panel_mode(&engine);
        let readonly = factory::is_readonly(&engine);
        let format = create_format(&engine);
        let save_files = if let (Some(ref dir), Some(ref fmt)) = (&game_dir, &format) {
            discovery::find_save_files(dir, &**fmt)
        } else {
            Vec::new()
        };

        Self {
            game_dir,
            game_title,
            engine,
            config,
            save_panel: SavePanelState {
                format,
                save_files,
                selected_save: None,
                save_data: None,
                summary: None,
                fields: Vec::new(),
                dirty_count: 0,
                selected_category: None,
                search_query: String::new(),
                panel_mode,
                readonly,
            },
            rt_panel: RtPanelState {
                conn: None,
                fields: Vec::new(),
                view_mode: RtViewMode::Compact,
                plugin_installed: false,
                host: "127.0.0.1".into(),
                port,
                pending_connect: false,
                pending_disconnect: false,
                error_message: String::new(),
            },
            status_message: String::new(),
            show_unsaved_dialog: false,
            pending_game_switch: false,
        }
    }

    fn load_save_file(&mut self) {
        let path = match &self.save_panel.selected_save {
            Some(p) => p.clone(),
            None => return,
        };
        let format = match &self.save_panel.format {
            Some(ref f) => f,
            None => return,
        };

        match format.load(&path) {
            Ok(data) => {
                let summary = format.get_summary(&data);
                let game_dir = self.game_dir.as_deref().unwrap_or("");
                let fields = format.scan_fields(&data, game_dir);
                self.save_panel.summary = Some(summary);
                self.save_panel.fields = fields;
                self.save_panel.save_data = Some(data);
                self.save_panel.dirty_count = 0;
            }
            Err(e) => {
                self.status_message = format!("加载存档失败: {}", e);
            }
        }
    }

    fn save_current(&mut self) {
        let path = match &self.save_panel.selected_save {
            Some(p) => p.clone(),
            None => { self.status_message = "未选择存档文件".into(); return; }
        };
        let save_data = match &mut self.save_panel.save_data {
            Some(d) => d,
            None => { self.status_message = "存档数据为空".into(); return; }
        };
        let format = match &self.save_panel.format {
            Some(ref f) => f,
            None => return,
        };

        // Apply all dirty fields
        let dirty: Vec<_> = self.save_panel.fields.iter()
            .filter(|f| f.dirty)
            .cloned()
            .collect();

        for field in &dirty {
            if let Err(e) = format.apply_field(save_data, field) {
                self.status_message = format!("写入字段 {} 失败: {}", field.display_name, e);
                return;
            }
        }

        match format.save(&path, save_data) {
            Ok(()) => {
                // Clear dirty flags
                for f in &mut self.save_panel.fields {
                    f.dirty = false;
                }
                self.save_panel.dirty_count = 0;
                self.status_message = "存档已保存".into();
            }
            Err(e) => {
                self.status_message = format!("保存失败: {}", e);
            }
        }
    }

    fn refresh_save_files(&mut self) {
        if let (Some(ref dir), Some(ref fmt)) = (&self.game_dir, &self.save_panel.format) {
            self.save_panel.save_files = discovery::find_save_files(dir, &**fmt);
        }
    }

    fn switch_game(&mut self) {
        if let Some(new_dir) = rfd::FileDialog::new()
            .set_title("选择游戏目录")
            .pick_folder()
        {
            let dir_str = new_dir.to_string_lossy().to_string();
            self.game_dir = Some(dir_str.clone());
            self.engine = detect_by_filesystem(&dir_str);
            self.game_title = String::new();
            self.save_panel.format = create_format(&self.engine);
            self.save_panel.panel_mode = factory::engine_to_panel_mode(&self.engine);
            self.save_panel.readonly = factory::is_readonly(&self.engine);
            self.save_panel.selected_save = None;
            self.save_panel.save_data = None;
            self.save_panel.summary = None;
            self.save_panel.fields.clear();
            self.save_panel.dirty_count = 0;
            self.save_panel.selected_category = None;
            self.save_panel.search_query.clear();
            self.rt_panel.plugin_installed = false;
            self.refresh_save_files();

            // Check plugin status
            if factory::supports_realtime(&self.engine) {
                match self.engine {
                    EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
                        self.rt_panel.plugin_installed = game_tool_rpgmaker::tcp::is_plugin_installed(&dir_str);
                    }
                    EngineType::RenPy => {
                        self.rt_panel.plugin_installed = game_tool_renpy::bridge::is_plugin_installed(&dir_str);
                    }
                    _ => {}
                }
            }
        }
    }

    fn inject_plugin(&mut self) {
        let dir = match &self.game_dir {
            Some(d) => d.clone(),
            None => { self.status_message = "未选择游戏目录".into(); return; }
        };
        let result = match self.engine {
            EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
                game_tool_rpgmaker::tcp::inject_plugin(&dir).map_err(|e| e)
            }
            EngineType::RenPy => {
                game_tool_renpy::bridge::inject_plugin(&dir).map_err(|e| e)
            }
            _ => Err("不支持".into()),
        };
        match result {
            Ok(()) => { self.rt_panel.plugin_installed = true; }
            Err(e) => { self.status_message = format!("注入失败: {}", e); }
        }
    }

    fn rt_connect(&mut self) {
        self.rt_panel.error_message.clear();
        let port = self.rt_panel.port;
        let host = self.rt_panel.host.clone();
        let engine = self.engine.clone();

        let mut conn = connection::spawn_bridge_thread(engine, host.clone(), port);
        conn.status = ConnectionStatus::Connecting;
        let _ = conn.cmd_tx.send(BridgeJob::Connect { host, port });
        self.rt_panel.conn = Some(conn);
    }

    fn rt_disconnect(&mut self) {
        if let Some(ref conn) = self.rt_panel.conn {
            let _ = conn.cmd_tx.send(BridgeJob::Disconnect);
        }
    }

    fn drain_rt_results(&mut self) {
        if let Some(ref mut conn) = self.rt_panel.conn {
            let results = connection::drain_results(conn);
            for result in results {
                match result {
                    BridgeResult::Connected => {
                        conn.status = ConnectionStatus::Connected;
                        // Auto ReadAll after connect
                        let _ = conn.cmd_tx.send(BridgeJob::Execute(BridgeCommand::ReadAll));
                    }
                    BridgeResult::Disconnected => {
                        conn.status = ConnectionStatus::Disconnected;
                        self.rt_panel.fields.clear();
                    }
                    BridgeResult::CommandResult(val) => {
                        if let Ok(gs) = serde_json::from_value::<game_tool_core::GameState>(val.clone()) {
                            self.rt_panel.fields = factory::game_state_to_fields(&gs, &self.engine);
                        }
                        // Sync single field reads back into rt_fields
                        if let (Some(obj), Some(_fid)) = (val.as_object(), Option::<String>::None) {
                            for (key, v) in obj {
                                if let Some(f) = self.rt_panel.fields.iter_mut()
                                    .find(|f| f.field_id == *key) {
                                    f.live_value = v.clone();
                                }
                            }
                        }
                    }
                    BridgeResult::Error(e) => {
                        self.rt_panel.error_message = e;
                        conn.status = ConnectionStatus::Disconnected;
                    }
                }
            }
        }
    }

    fn rt_send_command(&self, cmd: BridgeCommand) {
        if let Some(ref conn) = self.rt_panel.conn {
            let _ = conn.cmd_tx.send(BridgeJob::Execute(cmd));
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain realtime results first
        self.drain_rt_results();

        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            if top_bar::render(ui, &self.game_dir, &self.game_title, &self.engine) {
                if self.save_panel.dirty_count > 0 {
                    self.show_unsaved_dialog = true;
                    self.pending_game_switch = true;
                } else {
                    self.switch_game();
                }
            }
        });

        // Unsaved changes dialog
        if self.show_unsaved_dialog {
            egui::Window::new("未保存的修改")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("有 {} 处未保存的修改。是否保存后再切换？", self.save_panel.dirty_count));
                    ui.horizontal(|ui| {
                        if ui.button("保存并切换").clicked() {
                            self.save_current();
                            self.show_unsaved_dialog = false;
                            self.pending_game_switch = false;
                            self.switch_game();
                        }
                        if ui.button("丢弃修改").clicked() {
                            for f in &mut self.save_panel.fields { f.dirty = false; }
                            self.save_panel.dirty_count = 0;
                            self.show_unsaved_dialog = false;
                            self.pending_game_switch = false;
                            self.switch_game();
                        }
                        if ui.button("取消").clicked() {
                            self.show_unsaved_dialog = false;
                            self.pending_game_switch = false;
                        }
                    });
                });
        }

        // Main dual-panel layout
        egui::SidePanel::left("save_panel")
            .resizable(true)
            .default_width(500.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("存档编辑");

                    if self.game_dir.is_none() {
                        ui.colored_label(egui::Color32::from_rgb(150, 150, 150),
                            "请选择一个游戏目录开始");
                        return;
                    }

                    let actions = save_panel::render(ui, &mut self.save_panel);
                    for action in actions {
                        match action {
                            save_panel::SaveAction::LoadSave => self.load_save_file(),
                            save_panel::SaveAction::RefreshFiles => self.refresh_save_files(),
                            save_panel::SaveAction::Save => self.save_current(),
                        }
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("实时修改");

                if self.game_dir.is_none() {
                    ui.colored_label(egui::Color32::from_rgb(150, 150, 150),
                        "请先选择游戏目录");
                    return;
                }

                let actions = realtime_panel::render(
                    ui, &mut self.rt_panel, &self.engine, &self.game_dir,
                );
                for action in actions {
                    match action {
                        realtime_panel::RtAction::Connect => self.rt_connect(),
                        realtime_panel::RtAction::Disconnect => self.rt_disconnect(),
                        realtime_panel::RtAction::InjectPlugin => self.inject_plugin(),
                        realtime_panel::RtAction::ReadAll => {
                            self.rt_send_command(BridgeCommand::ReadAll);
                        }
                        realtime_panel::RtAction::WriteField(id, val) => {
                            self.rt_send_command(BridgeCommand::WriteField(id, val));
                        }
                    }
                }
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if !self.status_message.is_empty() {
                    let is_error = self.status_message.contains("失败");
                    if is_error {
                        ui.colored_label(egui::Color32::RED, &self.status_message);
                    } else {
                        ui.label(&self.status_message);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let field_count = self.save_panel.fields.len();
                    ui.label(format!("共 {} 个字段 | 已选游戏: {}",
                        field_count,
                        self.game_dir.as_deref().unwrap_or("无")));
                });
            });
        });
    }
}
```

---

### Task 10: main.rs — Entry point with startup folder dialog

**Files:**
- Create: `crates/gui/src/main.rs`

- [ ] **Step: Write the complete file**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod connection;
mod discovery;
mod factory;
mod state;
mod panels;
mod widgets;

fn main() {
    let game_dir = rfd::FileDialog::new()
        .set_title("选择游戏目录 (GameSaveEditor)")
        .pick_folder()
        .map(|p| p.to_string_lossy().to_string());

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("GameSaveEditor"),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "GameSaveEditor",
        native_options,
        Box::new(|_cc| Ok(Box::new(app::AppState::new(game_dir)))),
    );
}
```

---

### Task 11: build.bat update

**Files:**
- Modify: `build.bat`

- [ ] **Step: Update build.bat to build gui crate**

```bat
@echo off
echo === GameSaveEditor Build ===
echo.

echo Cleaning old output...
if exist "dist\" rd /s /q "dist"
mkdir dist

echo Building release...
cargo build --release -p game-tool-gui
if %errorlevel% neq 0 (
    echo BUILD FAILED
    pause
    exit /b 1
)

echo Copying binary...
copy /Y "target\release\GameSaveEditor.exe" "dist\GameSaveEditor.exe"
copy /Y "target\release\GameSaveEditor.pdb" "dist\GameSaveEditor.pdb" 2>nul

echo ===========================================
echo Build Complete!
echo Output: dist\GameSaveEditor.exe
echo Size:
dir "dist\GameSaveEditor.exe" | findstr "GameSaveEditor"
echo ===========================================
pause
```

---

### Task 12: Compile and fix errors

- [ ] **Step: Run cargo check**

```bash
cargo check -p game-tool-gui 2>&1
```

Expected: May have compilation errors. For each error:

Common potential issues and fixes:
- If `egui::ComboBox::from_id_salt` not found → use `from_id_source`
- If `egui::ViewportBuilder` not found → use `egui::viewport::ViewportBuilder` (eframe 0.31)
- If `ui.selectable_label` API differs → check egui 0.31 API
- Missing `pub mod` declarations → add to appropriate mod.rs files
- Import path issues → adjust `use` statements

- [ ] **Step: Fix errors iteratively until cargo check passes**

- [ ] **Step: Run cargo build --release -p game-tool-gui**

```bash
cargo build --release -p game-tool-gui 2>&1
```

- [ ] **Step: Verify binary exists**

```bash
Test-Path "target\release\GameSaveEditor.exe"
```

Expected: `True`
