# UI v3 精简选项卡 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 GameSaveEditor 从侧栏导航改为顶部选项卡布局，提高信息密度，简化操作流程。

**Architecture:** 废弃 Sidebar+Quickbar+Dashboard 三层架构，改为 TabBar+CentralPanel+StatusBar 三层。启动页替代仪表盘，5 个选项卡替代 6 个侧栏视图。字段表新增实时值/锁定/操作列。

**Tech Stack:** Rust, egui 0.31, eframe 0.31, game_tool_core, game_tool_rpgmaker

**Spec:** `docs/superpowers/specs/2026-05-23-game-tool-ui-v3-design.md`

---

## File Structure Map

```
crates/gui/src/
├── main.rs              [MODIFY] — Remove startup rfd dialog
├── state.rs             [MODIFY] — TabMode enum, ConfirmDialog, remove AppView/sidebar_collapsed
├── app.rs               [REWRITE] — New layout: startup/tab routing
├── theme.rs             [MODIFY] — Add tab_name(), engine_display_name()
├── lib.rs               [MODIFY] — No changes needed
├── panels/
│   ├── mod.rs           [MODIFY] — Add/remove modules
│   ├── tab_bar.rs       [NEW]    — Tab bar rendering
│   ├── status_bar.rs    [NEW]    — Status bar rendering
│   ├── startup.rs       [NEW]    — Startup page
│   ├── save_editor.rs   [NEW]    — Save editor tab (merge save_panel + dashboard)
│   ├── realtime_editor.rs [NEW]  — Realtime editor tab (from realtime_panel)
│   ├── backup.rs        [REWRITE] — Backup tab with diff comparison
│   ├── toolbox.rs       [REWRITE] — Toolbox tab with collapsible sections
│   ├── settings.rs      [REWRITE] — Settings tab expanded
│   ├── top_bar.rs       [MODIFY] — Simplify
│   ├── sidebar.rs       [REMOVE]
│   ├── quickbar.rs      [REMOVE]
│   ├── dashboard.rs     [REMOVE]
│   ├── save_panel.rs    [REMOVE]
│   └── realtime_panel.rs [REMOVE]
├── widgets/
│   ├── mod.rs           [MODIFY] — Update declarations
│   ├── field_table.rs   [MODIFY] — Add live_value col, lock col, action col
│   ├── category_tree.rs [MODIFY] — No change needed
│   ├── search_bar.rs    [MODIFY] — No change needed
│   └── summary_card.rs  [MODIFY] — No change needed
```

---

## Phase 1: 骨架迁移

### Task 1: 新增 TabMode 枚举和 ConfirmDialog

**Files:**
- Modify: `crates/gui/src/state.rs`

- [ ] **Step 1: 替换 AppView 为 TabMode，新增 ConfirmDialog**

Read `crates/gui/src/state.rs` first. Replace the `AppView` enum with `TabMode`:

```rust
// REMOVE lines 8-16 (AppView enum)
// ADD after ConnectionStatus:
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TabMode {
    SaveEditor,
    RealtimeEditor,
    BackupManager,
    Toolbox,
    Settings,
}
```

Add `selected_indices` to `RtPanelState`:

```rust
// In RtPanelState, add after locked_fields:
pub locked_fields: HashSet<String>,
pub selected_indices: HashSet<usize>,    // NEW: for backup diff selection tracking
```

Add `ConfirmAction` and modify `AppState`:

```rust
// ADD before AppState:
pub enum ConfirmAction {
    DiscardAndSwitch,
    DeleteBackups(Vec<usize>),
    RestoreBackup(usize),
    ClearRecentGames,
    DeleteSingleBackup(usize),
}

pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub on_confirm: ConfirmAction,
}
```

Modify `AppState`:

```rust
pub struct AppState {
    pub game_dir: Option<String>,
    pub game_title: String,
    pub engine: EngineType,
    pub game_config: Option<GameConfig>,
    // REMOVE: pub active_view: AppView,
    // REMOVE: pub sidebar_collapsed: bool,
    pub active_tab: TabMode,                  // NEW
    pub dark_mode: bool,
    pub recent_games: Vec<String>,
    pub backup_paths: Vec<String>,
    pub save_panel: SavePanelState,
    pub rt_panel: RtPanelState,
    pub status_message: String,
    pub show_unsaved_dialog: bool,
    pub show_confirm_dialog: Option<ConfirmDialog>,  // NEW
}
```

Update the test at line 149-155 to test `TabMode` instead of `AppView`:

```rust
#[test]
fn test_tab_mode_variants_distinct() {
    assert_ne!(TabMode::SaveEditor, TabMode::RealtimeEditor);
    assert_ne!(TabMode::RealtimeEditor, TabMode::BackupManager);
    assert_ne!(TabMode::BackupManager, TabMode::Toolbox);
    assert_ne!(TabMode::Toolbox, TabMode::Settings);
    assert_ne!(TabMode::Settings, TabMode::SaveEditor);
}

#[test]
fn test_tab_mode_clone() {
    let t = TabMode::SaveEditor;
    assert_eq!(t, t);
}

#[test]
fn test_confirm_action_variants_constructible() {
    let _a1 = ConfirmAction::DiscardAndSwitch;
    let _a2 = ConfirmAction::DeleteBackups(vec![0, 1]);
    let _a3 = ConfirmAction::RestoreBackup(0);
    let _a4 = ConfirmAction::ClearRecentGames;
    let _a5 = ConfirmAction::DeleteSingleBackup(0);
}
```

Remove the old AppView tests (lines 149-161).

- [ ] **Step 2: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: COMPILE ERRORS — many references to `AppView`, `active_view`, `sidebar_collapsed` remain. Proceed to next task to fix.

---

### Task 2: 更新 theme.rs — 添加公用函数

**Files:**
- Modify: `crates/gui/src/theme.rs`

- [ ] **Step 1: 替换 view_icon/view_name，添加 tab_name/engine_display_name**

Replace the existing `view_icon` and `view_name` functions:

```rust
// REMOVE view_icon() and view_name() (lines 86-105)
// ADD:

pub fn tab_icon(tab: &crate::state::TabMode) -> &'static str {
    match tab {
        crate::state::TabMode::SaveEditor => "\u{1f4c2}",
        crate::state::TabMode::RealtimeEditor => "\u{26a1}",
        crate::state::TabMode::BackupManager => "\u{1f5c4}",
        crate::state::TabMode::Toolbox => "\u{1f9f0}",
        crate::state::TabMode::Settings => "\u{2699}",
    }
}

pub fn tab_name(tab: &crate::state::TabMode) -> &'static str {
    match tab {
        crate::state::TabMode::SaveEditor => "\u{5b58}\u{6863}\u{7f16}\u{8f91}",
        crate::state::TabMode::RealtimeEditor => "\u{5b9e}\u{65f6}\u{4fee}\u{6539}",
        crate::state::TabMode::BackupManager => "\u{5907}\u{4efd}\u{7ba1}\u{7406}",
        crate::state::TabMode::Toolbox => "\u{5de5}\u{5177}\u{7bb1}",
        crate::state::TabMode::Settings => "\u{8bbe}\u{7f6e}",
    }
}

use game_tool_core::detector::EngineType;

pub fn engine_display_name(engine: &EngineType) -> &'static str {
    match engine {
        EngineType::RpgMakerMv => "RPG Maker MV",
        EngineType::RpgMakerMz => "RPG Maker MZ",
        EngineType::NwJs => "NW.js",
        EngineType::RenPy => "Ren'Py",
        EngineType::Unreal => "Unreal",
        EngineType::UnityMono => "Unity (Mono)",
        EngineType::UnityIl2Cpp => "Unity (IL2CPP)",
        EngineType::Godot => "Godot",
        EngineType::Unknown => "\u{672a}\u{77e5}",
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: Still errors from sidebar.rs references to old `AppView` — proceed to fix.

---

### Task 3: 新建 tab_bar.rs

**Files:**
- Create: `crates/gui/src/panels/tab_bar.rs`

- [ ] **Step 1: 写入 tab_bar.rs**

```rust
use crate::state::{AppState, TabMode};
use crate::theme::{self, colors};
use egui::{Frame, Margin, Ui};

pub enum TabAction {
    SwitchTab(TabMode),
    SwitchGame,
}

pub fn render(ui: &mut Ui, state: &AppState) -> Vec<TabAction> {
    let mut actions = Vec::new();
    let has_game = state.game_dir.is_some();

    let tabs = [
        TabMode::SaveEditor,
        TabMode::RealtimeEditor,
        TabMode::BackupManager,
        TabMode::Toolbox,
        TabMode::Settings,
    ];

    Frame::NONE
        .fill(colors::PANEL_DARK)
        .inner_margin(Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for tab in &tabs {
                    let enabled = match tab {
                        TabMode::Toolbox | TabMode::Settings => true,
                        TabMode::RealtimeEditor => {
                            has_game && crate::factory::supports_realtime(&state.engine)
                        }
                        _ => has_game,
                    };
                    let selected = state.active_tab == *tab;

                    let text_color = if enabled {
                        if selected { colors::ACCENT } else { colors::TEXT }
                    } else {
                        colors::TEXT_DISABLED
                    };

                    let label = format!("{} {}", theme::tab_icon(tab), theme::tab_name(tab));
                    let resp = ui.add_enabled_ui(enabled, |ui| {
                        ui.selectable_label(selected, &label)
                    });
                    let resp = resp.inner;

                    if selected {
                        let rect = resp.rect.expand2(egui::vec2(0.0, 2.0));
                        let underline = egui::Rect::from_min_size(
                            egui::pos2(rect.left(), rect.bottom()),
                            egui::vec2(rect.width(), 2.0),
                        );
                        ui.painter().rect_filled(underline, 0.0, colors::ACCENT);
                    }

                    if resp.clicked() && enabled {
                        actions.push(TabAction::SwitchTab(*tab));
                    }

                    if !enabled {
                        let reason = if !has_game {
                            "\u{8bf7}\u{5148}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}"
                        } else {
                            "\u{5f53}\u{524d}\u{5f15}\u{64ce}\u{4e0d}\u{652f}\u{6301}\u{5b9e}\u{65f6}\u{4fee}\u{6539}"
                        };
                        resp.on_hover_text(reason);
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_game && ui.button("\u{1f504} \u{5207}\u{6362}\u{6e38}\u{620f}").clicked() {
                        actions.push(TabAction::SwitchGame);
                    }
                });
            });
        });

    actions
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: Module not yet declared — still errors but new file doesn't break anything.

---

### Task 4: 新建 status_bar.rs

**Files:**
- Create: `crates/gui/src/panels/status_bar.rs`

- [ ] **Step 1: 写入 status_bar.rs**

```rust
use crate::state::{AppState, ConnectionStatus, TabMode};
use crate::theme::colors;
use egui::{Frame, Margin, Ui};

pub fn render(ui: &mut Ui, state: &AppState) {
    Frame::NONE
        .fill(colors::PANEL_DARK)
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let has_game = state.game_dir.is_some();

                if !has_game {
                    ui.colored_label(colors::TEXT_SECONDARY, "\u{672a}\u{52a0}\u{8f7d}\u{6e38}\u{620f}");
                    return;
                }

                match state.active_tab {
                    TabMode::SaveEditor => {
                        let dirty = state.save_panel.dirty_count;
                        if dirty > 0 {
                            ui.colored_label(colors::WARNING, format!("\u{204a}{}\u{5904}\u{4fee}\u{6539}\u{672a}\u{4fdd}\u{5b58}", dirty));
                        } else {
                            ui.label("\u{65e0}\u{672a}\u{4fdd}\u{5b58}\u{4fee}\u{6539}");
                        }
                        ui.separator();
                        let count = state.save_panel.fields.len();
                        ui.label(format!("\u{5171} {}\u{4e2a}\u{5b57}\u{6bb5}", count));
                        if let Some(ref path) = state.save_panel.selected_save {
                            if let Some(name) = std::path::Path::new(path)
                                .file_name().and_then(|n| n.to_str())
                            {
                                ui.colored_label(colors::TEXT_SECONDARY, format!(" | {}", name));
                            }
                        }
                    }
                    TabMode::RealtimeEditor => {
                        let status = state.rt_panel.conn.as_ref()
                            .map(|c| c.status)
                            .unwrap_or(ConnectionStatus::Disconnected);
                        let (icon, icon_color, label) = match status {
                            ConnectionStatus::Connected => ("\u{25cf}", colors::SUCCESS, "\u{5df2}\u{8fde}\u{63a5}"),
                            ConnectionStatus::Connecting => ("\u{25cc}", colors::WARNING, "\u{8fde}\u{63a5}\u{4e2d}..."),
                            ConnectionStatus::Disconnected => ("\u{25cb}", colors::TEXT_DISABLED, "\u{672a}\u{8fde}\u{63a5}"),
                        };
                        ui.colored_label(icon_color, format!("{} {}", icon, label));
                        ui.colored_label(colors::TEXT_SECONDARY, format!(" :{}", state.rt_panel.port));
                        ui.separator();
                        let locked = state.rt_panel.locked_fields.len();
                        if locked > 0 {
                            ui.colored_label(colors::WARNING, format!("\u{1f512} {}\u{4e2a}\u{9501}\u{5b9a}", locked));
                        }
                        ui.separator();
                        let count = state.rt_panel.fields.len();
                        ui.label(format!("\u{5171} {}\u{4e2a}\u{5b57}\u{6bb5}", count));
                    }
                    TabMode::BackupManager => {
                        let count = state.backup_paths.len();
                        ui.label(format!("\u{5171} {}\u{4e2a}\u{5907}\u{4efd}", count));
                        if let Some(ref path) = state.save_panel.selected_save {
                            if let Some(name) = std::path::Path::new(path)
                                .file_name().and_then(|n| n.to_str())
                            {
                                ui.colored_label(colors::TEXT_SECONDARY, format!(" | \u{5f53}\u{524d}: {}", name));
                            }
                        }
                    }
                    TabMode::Toolbox => {
                        ui.colored_label(colors::TEXT_SECONDARY, "\u{5de5}\u{5177}\u{7bb1} \u{2014} \u{72ec}\u{7acb}\u{5de5}\u{5177}\u{ff0c}\u{65e0}\u{9700}\u{52a0}\u{8f7d}\u{6e38}\u{620f}");
                    }
                    TabMode::Settings => {
                        ui.colored_label(colors::TEXT_SECONDARY, "\u{8bbe}\u{7f6e}");
                    }
                }
            });
        });
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: Module not yet declared.

---

### Task 5: 新建 startup.rs

**Files:**
- Create: `crates/gui/src/panels/startup.rs`

- [ ] **Step 1: 写入 startup.rs**

```rust
use crate::state::AppState;
use crate::theme::colors;
use egui::{Frame, Margin, Ui};

pub enum StartupAction {
    OpenGameDir,
    OpenRecentGame(String),
}

pub fn render(ui: &mut Ui, state: &AppState) -> Vec<StartupAction> {
    let mut actions = Vec::new();

    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.heading("\u{1f3ae} \u{9009}\u{62e9}\u{4e00}\u{4e2a}\u{6e38}\u{620f}\u{76ee}\u{5f55}\u{5f00}\u{59cb}");
        ui.add_space(16.0);
        if ui
            .add_sized(
                [200.0, 48.0],
                egui::Button::new("\u{1f4c2} \u{6253}\u{5f00}\u{6e38}\u{620f}\u{76ee}\u{5f55}..."),
            )
            .clicked()
        {
            actions.push(StartupAction::OpenGameDir);
        }
    });

    ui.add_space(32.0);

    if !state.recent_games.is_empty() {
        ui.separator();
        ui.add_space(12.0);
        ui.strong("\u{2014}\u{2014} \u{6700}\u{8fd1}\u{6e38}\u{620f} \u{2014}\u{2014}");
        ui.add_space(8.0);

        Frame::NONE
            .fill(colors::PANEL_DARK)
            .inner_margin(Margin::same(12))
            .corner_radius(6)
            .show(ui, |ui| {
                for path in &state.recent_games {
                    let engine = game_tool_core::detector::detect_by_filesystem(path);
                    let ename = crate::theme::engine_display_name(&engine);

                    ui.horizontal(|ui| {
                        ui.label(format!("\u{1f4c1} {}", {
                            std::path::Path::new(path)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| path.clone())
                        }));
                        ui.colored_label(colors::TEXT_SECONDARY, ename);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("\u{6253}\u{5f00}").clicked() {
                                actions.push(StartupAction::OpenRecentGame(path.clone()));
                            }
                        });
                    });
                }
            });
    }

    actions
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: Module not yet declared.

---

### Task 6: 更新 panels/mod.rs — 模块声明

**Files:**
- Modify: `crates/gui/src/panels/mod.rs`

- [ ] **Step 1: 更新模块声明**

Replace entire file:

```rust
pub mod backup;
pub mod realtime_editor;
pub mod save_editor;
pub mod settings;
pub mod startup;
pub mod status_bar;
pub mod tab_bar;
pub mod toolbox;
pub mod top_bar;
```

- [ ] **Step 2: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: Still errors — main `app.rs` references missing modules (sidebar, quickbar, dashboard, save_panel, realtime_panel). Proceed to rewrite app.rs.

---

### Task 7: 重写 app.rs — 新主布局

**Files:**
- Rewrite: `crates/gui/src/app.rs`

This is the largest change. We replace the SidePanel navigation with TabBar routing.

- [ ] **Step 1: 重写 AppState::new() — 改用 TabMode**

Read current `crates/gui/src/app.rs` lines 16-114. Modify `AppState::new()`:

Change:
```rust
active_view: AppView::Dashboard,
sidebar_collapsed: false,
```
To:
```rust
active_tab: TabMode::SaveEditor,
```

Remove the `sidebar_collapsed` field initialization.

- [ ] **Step 2: 重写 update() 方法 (lines 456-761)**

Replace the entire `impl eframe::App for AppState` block:

```rust
impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_rt_results();
        crate::theme::Theme::new(self.dark_mode).apply(ctx);

        let has_game = self.game_dir.is_some();

        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            top_bar::render(ui, has_game, &self.game_title, &self.engine, &self.game_dir);
        });

        // Content area
        if has_game {
            // Tab bar
            egui::TopBottomPanel::top("tab_bar")
                .show_separator_line(false)
                .show(ctx, |ui| {
                    let actions = tab_bar::render(ui, self);
                    for action in actions {
                        match action {
                            tab_bar::TabAction::SwitchTab(tab) => {
                                self.active_tab = tab;
                            }
                            tab_bar::TabAction::SwitchGame => {
                                if self.save_panel.dirty_count > 0 {
                                    self.show_unsaved_dialog = true;
                                } else {
                                    self.switch_game();
                                }
                            }
                        }
                    }
                });

            egui::CentralPanel::default().show(ctx, |ui| {
                match self.active_tab {
                    TabMode::SaveEditor => {
                        let actions = save_editor::render(ui, self);
                        for action in actions {
                            match action {
                                save_editor::SaveEditorAction::LoadSave => self.load_save_file(),
                                save_editor::SaveEditorAction::RefreshFiles => self.refresh_save_files(),
                                save_editor::SaveEditorAction::Save => { self.save_current(); }
                                save_editor::SaveEditorAction::UndoDirty => {
                                    if let Some(ref path) = self.save_panel.selected_save {
                                        if let Some(ref format) = self.save_panel.format {
                                            if let Ok(data) = format.load(path) {
                                                let game_dir = self.game_dir.as_deref().unwrap_or("");
                                                let fields = format.scan_fields(&data, game_dir);
                                                self.save_panel.save_data = Some(data);
                                                self.save_panel.fields = fields;
                                                self.save_panel.dirty_count = 0;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    TabMode::RealtimeEditor => {
                        // Connection controls
                        ui.horizontal(|ui| {
                            let is_disconnected = self.rt_panel.conn.is_none()
                                || self.rt_panel.conn.as_ref()
                                    .map(|c| c.status == ConnectionStatus::Disconnected)
                                    .unwrap_or(true);

                            if is_disconnected {
                                if ui.button("\u{25cf} \u{8fde}\u{63a5}").clicked() {
                                    self.rt_connect();
                                }
                            } else {
                                if ui.button("\u{25cb} \u{65ad}\u{5f00}").clicked() {
                                    self.rt_disconnect();
                                }
                            }

                            if !self.rt_panel.plugin_installed {
                                if ui.button("\u{6ce8}\u{5165}\u{63d2}\u{4ef6}").clicked() {
                                    self.inject_plugin();
                                }
                            } else {
                                ui.colored_label(
                                    crate::theme::colors::SUCCESS,
                                    "\u{2713} \u{63d2}\u{4ef6}\u{5df2}\u{6ce8}\u{5165}",
                                );
                            }

                            ui.separator();

                            let auto = self.rt_panel.auto_refresh;
                            let label = if auto { "\u{25b6} \u{81ea}\u{52a8}\u{5237}\u{65b0}" }
                                        else { "\u{23f8} \u{6682}\u{505c}\u{5237}\u{65b0}" };
                            if ui.button(label).clicked() {
                                self.rt_panel.auto_refresh = !auto;
                            }
                            if ui.button("\u{1f4e5} \u{624b}\u{52a8}\u{5237}\u{65b0}").clicked() {
                                self.rt_send_command(BridgeCommand::ReadAll);
                            }
                        });

                        if !self.rt_panel.error_message.is_empty() {
                            ui.colored_label(crate::theme::colors::ERROR, &self.rt_panel.error_message);
                        }
                        if !self.rt_panel.write_feedback.is_empty() {
                            ui.colored_label(crate::theme::colors::SUCCESS, &self.rt_panel.write_feedback);
                        }

                        ui.separator();

                        let actions = realtime_editor::render(ui, &mut self.rt_panel, &self.engine);
                        for action in actions {
                            match action {
                                realtime_editor::RealtimeAction::WriteField(id, val) => {
                                    self.rt_send_command(BridgeCommand::WriteField(id, val));
                                }
                                realtime_editor::RealtimeAction::ToggleLock(fid) => {
                                    if self.rt_panel.locked_fields.contains(&fid) {
                                        self.rt_panel.locked_fields.remove(&fid);
                                    } else {
                                        self.rt_panel.locked_fields.insert(fid);
                                    }
                                }
                                realtime_editor::RealtimeAction::CopyToSave(fid) => {
                                    if let Some(field) = self.rt_panel.fields.iter()
                                        .find(|f| f.field_id == fid)
                                    {
                                        if let Some(sf) = self.save_panel.fields.iter_mut()
                                            .find(|f| f.field_id == fid)
                                        {
                                            sf.save_value = field.live_value.clone();
                                            sf.dirty = true;
                                        }
                                    }
                                    self.save_panel.dirty_count = self.save_panel.fields.iter()
                                        .filter(|f| f.dirty).count();
                                }
                            }
                        }
                    }
                    TabMode::BackupManager => {
                        let actions = backup::render(ui, self);
                        for action in actions {
                            match action {
                                backup::BackupAction::CreateBackup => self.create_backup(),
                                backup::BackupAction::Restore(i) => {
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "\u{6062}\u{590d}\u{5907}\u{4efd}".into(),
                                        message: format!("\u{786e}\u{5b9a}\u{7528}\u{6b64}\u{5907}\u{4efd}\u{8986}\u{76d6}\u{5f53}\u{524d}\u{5b58}\u{6863}\u{ff1f}\u{6b64}\u{64cd}\u{4f5c}\u{4e0d}\u{53ef}\u{64a4}\u{9500}\u{3002}"),
                                        on_confirm: ConfirmAction::RestoreBackup(i),
                                    });
                                }
                                backup::BackupAction::Delete(i) => {
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "\u{5220}\u{9664}\u{5907}\u{4efd}".into(),
                                        message: format!("\u{786e}\u{5b9a}\u{5220}\u{9664}\u{6b64}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{ff1f}"),
                                        on_confirm: ConfirmAction::DeleteSingleBackup(i),
                                    });
                                }
                                backup::BackupAction::BatchDelete(indices) => {
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "\u{6279}\u{91cf}\u{5220}\u{9664}".into(),
                                        message: format!("\u{786e}\u{5b9a}\u{5220}\u{9664}\u{9009}\u{4e2d}\u{7684} {}\u{4e2a}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{ff1f}", indices.len()),
                                        on_confirm: ConfirmAction::DeleteBackups(indices),
                                    });
                                }
                            }
                        }
                    }
                    TabMode::Toolbox => {
                        toolbox::render(ui, self);
                    }
                    TabMode::Settings => {
                        let actions = settings::render(ui, self);
                        for action in actions {
                            match action {
                                settings::SettingsAction::ToggleDarkMode => {
                                    self.dark_mode = !self.dark_mode;
                                    if let Ok(mut cfg) = load_config() {
                                        cfg.dark_mode = self.dark_mode;
                                        let _ = game_tool_core::config::save_config(&cfg);
                                    }
                                }
                                settings::SettingsAction::SetPort(port) => {
                                    self.rt_panel.port = port;
                                    if self.rt_panel.conn.is_some() {
                                        self.status_message = "\u{7aef}\u{53e3}\u{5df2}\u{66f4}\u{6539}\u{ff0c}\u{8bf7}\u{65ad}\u{5f00}\u{540e}\u{91cd}\u{65b0}\u{8fde}\u{63a5}\u{4ee5}\u{751f}\u{6548}\u{3002}".into();
                                    }
                                }
                            }
                        }
                    }
                }
            });
        } else {
            // No game loaded — show startup page
            egui::CentralPanel::default().show(ctx, |ui| {
                let actions = startup::render(ui, self);
                for action in actions {
                    match action {
                        startup::StartupAction::OpenGameDir => {
                            self.switch_game();
                        }
                        startup::StartupAction::OpenRecentGame(path) => {
                            self.game_dir = Some(path);
                            // Re-initialize for the loaded game
                            let dir_str = self.game_dir.as_ref().unwrap().clone();
                            self.engine = detect_by_filesystem(&dir_str);
                            self.game_config = if self.engine != EngineType::Unknown {
                                let gc = game_tool_rpgmaker::scanner::scan_game_directory(&dir_str);
                                if gc.data_loaded {
                                    self.game_title = gc.game_title.clone();
                                    Some(gc)
                                } else {
                                    self.game_title.clear();
                                    None
                                }
                            } else {
                                self.game_title.clear();
                                None
                            };
                            self.save_panel.format = create_format(&self.engine);
                            self.save_panel.panel_mode = factory::engine_to_panel_mode(&self.engine);
                            self.save_panel.readonly = factory::is_readonly(&self.engine);
                            self.save_panel.selected_save = None;
                            self.save_panel.save_data = None;
                            self.save_panel.summary = None;
                            self.save_panel.fields.clear();
                            self.save_panel.dirty_count = 0;
                            self.refresh_save_files();
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
                            self.active_tab = TabMode::SaveEditor;

                            // Update recent games
                            if let Some(ref dir) = self.game_dir {
                                let dir = dir.clone();
                                self.recent_games.retain(|g| g != &dir);
                                self.recent_games.insert(0, dir);
                                self.recent_games.truncate(5);
                            }
                        }
                    }
                }
            });
        }

        // Unaved changes dialog
        if self.show_unsaved_dialog {
            egui::Window::new("\u{672a}\u{4fdd}\u{5b58}\u{7684}\u{4fee}\u{6539}")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("\u{6709} {}\u{5904}\u{672a}\u{4fdd}\u{5b58}\u{7684}\u{4fee}\u{6539}\u{3002}\u{662f}\u{5426}\u{4fdd}\u{5b58}\u{540e}\u{518d}\u{5207}\u{6362}\u{ff1f}", self.save_panel.dirty_count));
                    ui.horizontal(|ui| {
                        if ui.button("\u{4fdd}\u{5b58}\u{5e76}\u{5207}\u{6362}").clicked()
                            && self.save_current()
                        {
                            self.show_unsaved_dialog = false;
                            self.switch_game();
                        }
                        if ui.button("\u{4e22}\u{5f03}\u{4fee}\u{6539}").clicked() {
                            if let Some(ref path) = self.save_panel.selected_save {
                                if let Some(ref format) = self.save_panel.format {
                                    if let Ok(data) = format.load(path) {
                                        let game_dir = self.game_dir.as_deref().unwrap_or("");
                                        let fields = format.scan_fields(&data, game_dir);
                                        self.save_panel.save_data = Some(data);
                                        self.save_panel.fields = fields;
                                    }
                                }
                            }
                            self.save_panel.dirty_count = 0;
                            self.show_unsaved_dialog = false;
                            self.switch_game();
                        }
                        if ui.button("\u{53d6}\u{6d88}").clicked() {
                            self.show_unsaved_dialog = false;
                        }
                    });
                });
        }

        // Confirm dialog
        if let Some(ref dialog) = self.show_confirm_dialog {
            let title = dialog.title.clone();
            let message = dialog.message.clone();
            let action = std::mem::replace(
                &mut self.show_confirm_dialog,
                Some(ConfirmDialog {
                    title: String::new(),
                    message: String::new(),
                    on_confirm: ConfirmAction::DiscardAndSwitch,
                }),
            );
            let mut confirmed = false;

            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(&message);
                    ui.horizontal(|ui| {
                        if ui.button("\u{786e}\u{5b9a}").clicked() {
                            confirmed = true;
                        }
                        if ui.button("\u{53d6}\u{6d88}").clicked() {
                            self.show_confirm_dialog = None;
                        }
                    });
                });

            if confirmed {
                if let Some(d) = action {
                    match d.on_confirm {
                        ConfirmAction::DeleteBackups(indices) => {
                            let mut sorted: Vec<usize> = indices.into_iter().collect();
                            sorted.sort_by(|a, b| b.cmp(a));
                            for i in sorted {
                                self.delete_backup(i);
                            }
                        }
                        ConfirmAction::RestoreBackup(i) => self.restore_backup(i),
                        ConfirmAction::DeleteSingleBackup(i) => self.delete_backup(i),
                        ConfirmAction::ClearRecentGames => {
                            self.recent_games.clear();
                        }
                        ConfirmAction::DiscardAndSwitch => {}
                    }
                }
                self.show_confirm_dialog = None;
            }
        }

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            status_bar::render(ui, self);
        });
    }
}
```

- [ ] **Step 2: 更新 top_bar::render 签名和 import**

The top_bar's render signature changes. Update it to match:

Open `crates/gui/src/panels/top_bar.rs` and replace contents:

```rust
use crate::theme::colors;
use egui::Ui;

pub fn render(ui: &mut Ui, has_game: bool, game_title: &str, engine: &game_tool_core::detector::EngineType, game_dir: &Option<String>) {
    ui.horizontal(|ui| {
        ui.heading("\u{1f3ae} GameSaveEditor");

        if has_game {
            ui.separator();
            if let Some(ref dir) = game_dir {
                let short = std::path::Path::new(dir)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| dir.clone());
                ui.colored_label(colors::TEXT_SECONDARY, format!("\u{6e38}\u{620f}: {}", short))
                    .on_hover_text(dir);
            }
            ui.separator();
            let ename = crate::theme::engine_display_name(engine);
            ui.label(format!("\u{5f15}\u{64ce}: {}", ename));
            if !game_title.is_empty() {
                ui.separator();
                ui.label(format!("\u{6807}\u{9898}: {}", game_title));
            }
        }

        if !has_game {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Toolbox and settings buttons accessible even without a game
                // These would need to be wired via actions, but for now keep the
                // startup page handling them
            });
        }
    });
}
```

- [ ] **Step 3: 更新 drain_rt_results — 用实际时间替代帧计数**

Find `self.rt_panel.refresh_timer += 1;` and `if self.rt_panel.refresh_timer >= 180` in drain_rt_results (lines 298-315).

Replace with `std::time::Instant` based timing. Add a field to RtPanelState:

In `state.rs`, add to `RtPanelState`:
```rust
pub last_refresh: Option<std::time::Instant>,   // NEW: replaces refresh_timer
```

In `app.rs`, initialize in `AppState::new()`:
```rust
last_refresh: None,
```

Replace the auto-refresh timer logic (lines 299-315) with:
```rust
if self.rt_panel.auto_refresh
    && self.rt_panel.conn.as_ref()
        .map(|c| c.status == ConnectionStatus::Connected)
        .unwrap_or(false)
{
    let interval = std::time::Duration::from_secs(self.rt_panel.refresh_interval_secs);
    let should_refresh = match self.rt_panel.last_refresh {
        Some(last) => last.elapsed() >= interval,
        None => true,
    };
    if should_refresh {
        self.rt_panel.last_refresh = Some(std::time::Instant::now());
        if let Some(ref conn) = self.rt_panel.conn {
            let _ = conn.cmd_tx.send(BridgeJob::Execute(BridgeCommand::ReadAll));
        }
    }
}
```

Also add to `RtPanelState`:
```rust
pub refresh_interval_secs: u64,   // NEW: configurable refresh interval
```

Initialize in AppState::new():
```rust
refresh_interval_secs: 3,
```

- [ ] **Step 4: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: ERRORS — missing save_editor.rs, realtime_editor.rs. Proceed to Phase 2.

---

### Task 8: 更新 main.rs — 移除启动 rfd 对话框

**Files:**
- Modify: `crates/gui/src/main.rs`

- [ ] **Step 1: 移除启动时文件对话框**

Replace `main()` function:

```rust
fn main() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("GameSaveEditor"),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "GameSaveEditor",
        native_options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();

            if let Some(cjk_data) = load_cjk_font() {
                fonts.font_data.insert(
                    "CJK".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(cjk_data)),
                );
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "CJK".to_owned());
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .push("CJK".to_owned());
            }

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(AppState::new(None)))
        }),
    );
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: Still errors from missing save_editor/realtime_editor modules.

---

## Phase 2: 核心标签

### Task 9: 新建 save_editor.rs

**Files:**
- Create: `crates/gui/src/panels/save_editor.rs`

- [ ] **Step 1: 写入 save_editor.rs**

```rust
use crate::state::AppState;
use crate::theme::colors;
use crate::widgets::{category_tree, field_table, search_bar, summary_card};
use egui::{Frame, Margin, Ui};

pub enum SaveEditorAction {
    LoadSave,
    RefreshFiles,
    Save,
    UndoDirty,
}

pub fn render(ui: &mut Ui, state: &mut AppState) -> Vec<SaveEditorAction> {
    let mut actions = Vec::new();

    egui::ScrollArea::vertical().show(ui, |ui| {
        // Game overview row
        ui.horizontal(|ui| {
            ui.label(format!("\u{1f3ae} {}", state.game_title));
            ui.separator();
            let ename = crate::theme::engine_display_name(&state.engine);
            ui.label(format!("\u{5f15}\u{64ce}: {}", ename));
            ui.separator();

            let status = state.rt_panel.conn.as_ref()
                .map(|c| c.status)
                .unwrap_or(crate::state::ConnectionStatus::Disconnected);
            let (icon, icon_color) = match status {
                crate::state::ConnectionStatus::Connected => ("\u{25cf}", colors::SUCCESS),
                crate::state::ConnectionStatus::Connecting => ("\u{25cc}", colors::WARNING),
                crate::state::ConnectionStatus::Disconnected => ("\u{25cb}", colors::TEXT_DISABLED),
            };
            ui.colored_label(icon_color, icon);

            let currency = state.game_config.as_ref()
                .map(|gc| gc.currency_unit.as_str())
                .unwrap_or("G");
            ui.label(format!("\u{91d1}\u{5e01}\u{5355}\u{4f4d}: {}", currency));
        });

        ui.separator();

        // Save file selector
        ui.horizontal(|ui| {
            ui.label("\u{5b58}\u{6863}\u{6587}\u{4ef6}:");
            let current = state.save_panel.selected_save.as_ref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("\u{672a}\u{9009}\u{62e9}");

            egui::ComboBox::from_id_salt("save_editor_combo")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for sf in &state.save_panel.save_files.clone() {
                        let name = std::path::Path::new(&sf)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&sf);
                        let is_sel = state.save_panel.selected_save.as_deref() == Some(sf.as_str());
                        if ui.selectable_label(is_sel, name).clicked() {
                            state.save_panel.selected_save = Some(sf.clone());
                            actions.push(SaveEditorAction::LoadSave);
                        }
                    }
                });

            if ui.button("\u{1f504} \u{5237}\u{65b0}").clicked() {
                actions.push(SaveEditorAction::RefreshFiles);
            }
            if state.save_panel.dirty_count > 0 {
                if ui.button(format!("\u{1f4be} \u{4fdd}\u{5b58} ({})", state.save_panel.dirty_count)).clicked() {
                    actions.push(SaveEditorAction::Save);
                }
            }
        });

        ui.separator();

        // Filter bar
        ui.horizontal(|ui| {
            if state.save_panel.panel_mode == crate::state::SavePanelMode::RpgMaker {
                category_tree::render_horizontal(ui, &state.save_panel.fields, &mut state.save_panel.selected_category);
                ui.separator();
            }
            search_bar::render(ui, &mut state.save_panel.search_query);
            ui.separator();
            ui.label("\u{8df3}\u{8f6c} ID:");
            ui.text_edit_singleline(&mut state.save_panel.jump_id);
            if !state.save_panel.jump_id.is_empty() && ui.button("\u{21b0}").clicked() {
                // jump is handled in field_table via jump_id being cleared on consumption
            }
            if state.save_panel.dirty_count > 0 {
                if ui.button("\u{21a9} \u{64a4}\u{9500}\u{4fee}\u{6539}").clicked() {
                    actions.push(SaveEditorAction::UndoDirty);
                }
            }
        });

        // Summary card
        if let Some(ref summary) = state.save_panel.summary {
            ui.add_space(4.0);
            let currency = state.game_config.as_ref()
                .map(|gc| gc.currency_unit.as_str())
                .unwrap_or("");
            summary_card::render(ui, summary, currency);
            ui.add_space(4.0);
            ui.separator();
        }

        // Field table
        let is_connected = state.rt_panel.conn.as_ref()
            .map(|c| c.status == crate::state::ConnectionStatus::Connected)
            .unwrap_or(false);

        state.save_panel.dirty_count = field_table::render(
            ui,
            &mut state.save_panel.fields,
            state.save_panel.readonly,
            &state.save_panel.search_query,
            &state.save_panel.selected_category,
            &mut state.save_panel.jump_id,
            if is_connected { Some(&state.rt_panel.fields) } else { None },
        );
    });

    actions
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: Error — field_table::render signature mismatch. Proceed to update field_table.

---

### Task 10: 更新 field_table.rs — 新增实时值/差异/锁定列

**Files:**
- Modify: `crates/gui/src/widgets/field_table.rs`

- [ ] **Step 1: 更新 render 签名**

Change the render function signature to accept an optional live_fields parameter:

```rust
pub fn render(
    ui: &mut Ui,
    fields: &mut [ModifiableField],
    readonly: bool,
    search_query: &str,
    selected_category: &Option<String>,
    jump_id: &mut String,
    live_fields: Option<&[ModifiableField]>,
) -> usize {
```

- [ ] **Step 2: 构建 live_values 查找表**

After the dirty_count line, add:

```rust
let live_map: std::collections::HashMap<String, (serde_json::Value, bool)> = live_fields
    .map(|lf| {
        lf.iter()
            .map(|f| (f.field_id.clone(), (f.live_value.clone(), f.locked)))
            .collect()
    })
    .unwrap_or_default();
let show_live_col = !live_map.is_empty();
```

- [ ] **Step 3: 更新表头**

Replace the hardcoded 4-column header (lines 62-67) with:

```rust
ui.strong("\u{5206}\u{7c7b}");
ui.strong("\u{540d}\u{79f0}");
ui.strong("\u{4fdd}\u{5b58}\u{503c}");
if show_live_col {
    ui.strong("\u{5b9e}\u{65f6}\u{503c}");
}
ui.strong("\u{72b6}\u{6001}");
ui.end_row();
```

- [ ] **Step 4: 更新行渲染（在 for &idx 循环内）**

Keep the existing category/name/jump_target rendering (lines 70-83).

Replace the value column rendering (lines 85-95) - keep as is.

Replace the live value column (lines 97-107) with:

```rust
if show_live_col {
    let (live_val, locked) = live_map.get(&fields[idx].field_id)
        .cloned()
        .unwrap_or((serde_json::Value::Null, false));
    let live_display = value_display(&live_val);
    let save_val = &fields[idx].save_value;
    let is_diff = live_val != *save_val;

    if !live_display.is_empty() && live_display != "-" {
        if is_diff {
            ui.colored_label(Color32::from_rgb(210, 153, 34), &live_display);
        } else {
            ui.colored_label(Color32::from_rgb(139, 148, 158), &live_display);
        }
    } else {
        ui.colored_label(Color32::from_rgb(72, 79, 88), "-");
    }
}
```

Replace the status column (lines 109-113) with:

```rust
let mut status_parts = Vec::new();
if dirty {
    status_parts.push("*");
}
if show_live_col {
    let (live_val, _) = live_map.get(&fields[idx].field_id)
        .cloned()
        .unwrap_or((serde_json::Value::Null, false));
    if live_val != fields[idx].save_value {
        status_parts.push("\u{2190}");
    }
}
if status_parts.is_empty() {
    ui.label("");
} else {
    ui.colored_label(Color32::from_rgb(210, 153, 34), status_parts.join(" "));
}
```

- [ ] **Step 5: 编译验证**

Run: `cargo check -p game-tool-gui 2>&1`
Expected: Error — missing realtime_editor.rs. Proceed.

---

### Task 11: 新建 realtime_editor.rs

**Files:**
- Create: `crates/gui/src/panels/realtime_editor.rs`

- [ ] **Step 1: 写入 realtime_editor.rs**

```rust
use crate::state::RtPanelState;
use crate::widgets::{category_tree, field_table, search_bar};
use egui::Ui;
use game_tool_core::detector::EngineType;
use serde_json::Value;
use std::collections::BTreeMap;

pub enum RealtimeAction {
    WriteField(String, Value),
    ToggleLock(String),
    CopyToSave(String),
}

pub fn render(
    ui: &mut Ui,
    state: &mut RtPanelState,
    engine: &EngineType,
) -> Vec<RealtimeAction> {
    let mut actions = Vec::new();

    if !crate::factory::supports_realtime(engine) {
        ui.colored_label(
            egui::Color32::from_rgb(150, 150, 150),
            "\u{8be5}\u{5f15}\u{64ce}\u{6682}\u{4e0d}\u{652f}\u{6301}\u{5b9e}\u{65f6}\u{4fee}\u{6539}",
        );
        return actions;
    }

    let is_conn = state.conn.as_ref()
        .map(|c| c.status == crate::state::ConnectionStatus::Connected)
        .unwrap_or(false);

    // Filter bar
    ui.horizontal(|ui| {
        search_bar::render(ui, &mut state.search_query);
        ui.separator();
        ui.label("\u{8df3}\u{8f6c} ID:");
        ui.text_edit_singleline(&mut state.jump_id);
    });

    ui.separator();

    // Jump scroll target
    let mut jump_scroll_target: Option<usize> = None;
    if !state.jump_id.is_empty() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        let target = state.jump_id.clone();
        state.jump_id.clear();
        for (i, f) in state.fields.iter().enumerate() {
            if f.field_id == target {
                jump_scroll_target = Some(i);
                break;
            }
        }
    }

    let search_lower = state.search_query.to_lowercase();

    let mut cats: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, f) in state.fields.iter().enumerate() {
        let cat = category_tree::category_display_name(&f.category).to_string();
        cats.entry(cat).or_default().push(i);
    }

    let total = state.fields.len();
    ui.label(format!("\u{5171} {} \u{4e2a}\u{5b57}\u{6bb5}", total));

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (cat, indices) in &cats {
            let filtered: Vec<&usize> = indices.iter()
                .filter(|&&i| {
                    if search_lower.is_empty() { return true; }
                    let f = &state.fields[i];
                    f.display_name.to_lowercase().contains(&search_lower)
                        || f.field_id.to_lowercase().contains(&search_lower)
                })
                .collect();

            if filtered.is_empty() && !search_lower.is_empty() { continue; }

            ui.add_space(4.0);
            ui.strong(format!("{} ({})", cat, filtered.len()));

            for &&idx in &filtered {
                let fid = state.fields[idx].field_id.clone();
                let locked = state.locked_fields.contains(&fid);
                let dname = state.fields[idx].display_name.clone();
                let is_jump_target = jump_scroll_target == Some(idx);

                let response = ui.horizontal(|ui| {
                    // Lock toggle
                    let lock_icon = if locked { "\u{1f512}" } else { "\u{1f513}" };
                    if ui.selectable_label(false, lock_icon).clicked() {
                        actions.push(RealtimeAction::ToggleLock(fid.clone()));
                    }

                    let cat_label = category_tree::category_display_name(&state.fields[idx].category);
                    ui.label(cat_label);
                    ui.label(&dname);

                    // Live value editor
                    if let Some(new_val) = field_table::render_field_editor(
                        ui,
                        &state.fields[idx],
                        field_table::FieldSource::Live,
                    ) {
                        state.fields[idx].live_value = new_val.clone();
                        if is_conn {
                            actions.push(RealtimeAction::WriteField(fid.clone(), new_val));
                        }
                    }

                    // Save value (read-only)
                    let save_display = field_table_value_display(&state.fields[idx].save_value);
                    let is_diff = state.fields[idx].live_value != state.fields[idx].save_value;
                    if is_diff {
                        ui.colored_label(egui::Color32::from_rgb(210, 153, 34), &save_display);
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(139, 148, 158), &save_display);
                    }

                    // Copy to save button
                    if is_diff {
                        if ui.button("\u{1f4e4}\u{2192}\u{5b58}\u{6863}").clicked() {
                            actions.push(RealtimeAction::CopyToSave(fid.clone()));
                        }
                    }
                });

                if is_jump_target {
                    response.response.scroll_to_me(Some(egui::Align::Center));
                }
            }
        }
    });

    actions
}

fn field_table_value_display(v: &Value) -> String {
    match v {
        Value::Null => "-".into(),
        Value::Bool(b) => if *b { "ON" } else { "OFF" }.into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo build -p game-tool-gui 2>&1`
Expected: SHOULD COMPILE. This is the first milestone.

---

### Task 12: 编译验证 + 修复编译错误

**Files:**
- Various files as needed

- [ ] **Step 1: 编译**

Run: `cargo build -p game-tool-gui 2>&1`

- [ ] **Step 2: 逐个修复剩余的编译错误**

Common expected errors to fix:
1. `sidebar.rs`, `quickbar.rs`, `dashboard.rs`, `save_panel.rs`, `realtime_panel.rs` still exist in the module tree — remove their `pub mod` declarations from `panels/mod.rs`
2. Any remaining references to `AppView` in other files — replace with `TabMode`
3. Any remaining references to `sidebar_collapsed` — remove
4. `BackupAction::BatchDelete` and `Compare` — update `backup.rs` next
5. `view_icon` / `view_name` references — update to `tab_icon` / `tab_name`

- [ ] **Step 3: 删除旧文件**

After all references are removed:

```bash
Remove-Item -LiteralPath "crates/gui/src/panels/sidebar.rs"
Remove-Item -LiteralPath "crates/gui/src/panels/quickbar.rs"
Remove-Item -LiteralPath "crates/gui/src/panels/dashboard.rs"
Remove-Item -LiteralPath "crates/gui/src/panels/save_panel.rs"
Remove-Item -LiteralPath "crates/gui/src/panels/realtime_panel.rs"
```

- [ ] **Step 4: 最终编译**

Run: `cargo build -p game-tool-gui 2>&1`
Expected: COMPILE SUCCESS.

---

## Phase 3: 辅助标签

### Task 13: 重写 backup.rs

**Files:**
- Rewrite: `crates/gui/src/panels/backup.rs`

- [ ] **Step 1: 写入新 backup.rs**

```rust
use crate::state::AppState;
use crate::theme::colors;
use egui::Ui;

pub enum BackupAction {
    CreateBackup,
    Restore(usize),
    Delete(usize),
    BatchDelete(Vec<usize>),
}

pub fn render(ui: &mut Ui, state: &AppState) -> Vec<BackupAction> {
    let mut actions = Vec::new();

    if state.game_dir.is_none() {
        ui.colored_label(colors::TEXT_SECONDARY, "\u{8bf7}\u{5148}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}\u{3002}");
        return actions;
    }

    // File selector
    ui.horizontal(|ui| {
        let current = state.save_panel.selected_save.as_ref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("\u{672a}\u{9009}\u{62e9}\u{5b58}\u{6863}");
        ui.label(format!("\u{5f53}\u{524d}\u{5b58}\u{6863}: {}", current));
    });

    ui.add_space(4.0);

    if state.save_panel.selected_save.is_some() && state.save_panel.save_data.is_some() {
        if ui.button("\u{1f4be} \u{521b}\u{5efa}\u{5907}\u{4efd}").clicked() {
            actions.push(BackupAction::CreateBackup);
        }
    } else {
        ui.colored_label(colors::TEXT_DISABLED, "\u{8bf7}\u{5148}\u{5728}\u{5b58}\u{6863}\u{7f16}\u{8f91}\u{4e2d}\u{52a0}\u{8f7d}\u{5b58}\u{6863}\u{3002}");
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    if state.backup_paths.is_empty() {
        ui.colored_label(colors::TEXT_SECONDARY, "\u{6682}\u{65e0}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{3002}");
    } else {
        let mut selected: Vec<usize> = Vec::new();

        egui::Grid::new("backup_grid")
            .striped(true)
            .show(ui, |ui| {
                ui.strong("\u{6587}\u{4ef6}\u{540d}");
                ui.strong("\u{5927}\u{5c0f}");
                ui.strong("\u{64cd}\u{4f5c}");
                ui.end_row();

                for (i, bp) in state.backup_paths.iter().enumerate() {
                    let name = std::path::Path::new(bp)
                        .file_name().and_then(|n| n.to_str()).unwrap_or(bp);
                    let size = std::fs::metadata(bp).map(|m| m.len()).unwrap_or(0);
                    let size_str = if size > 1024 {
                        format!("{} KB", size / 1024)
                    } else {
                        format!("{} B", size)
                    };

                    let mut checked = selected.contains(&i);
                    ui.checkbox(&mut checked, "");
                    if checked { selected.push(i); }

                    ui.label(name);
                    ui.label(&size_str);

                    ui.horizontal(|ui| {
                        if ui.button("\u{267b} \u{6062}\u{590d}").clicked() {
                            actions.push(BackupAction::Restore(i));
                        }
                        if ui.button("\u{1f5d1} \u{5220}\u{9664}").clicked() {
                            actions.push(BackupAction::Delete(i));
                        }
                    });
                    ui.end_row();
                }

                if !selected.is_empty() {
                    ui.label(format!("\u{5df2}\u{9009} {} \u{9879}", selected.len()));
                    ui.label("");
                    ui.horizontal(|ui| {
                        if ui.button("\u{1f5d1} \u{6279}\u{91cf}\u{5220}\u{9664}").clicked() {
                            actions.push(BackupAction::BatchDelete(selected));
                        }
                    });
                    ui.end_row();
                }
            });
    }

    actions
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo build -p game-tool-gui 2>&1`
Expected: COMPILE SUCCESS.

---

### Task 14: 重写 toolbox.rs

**Files:**
- Rewrite: `crates/gui/src/panels/toolbox.rs`

- [ ] **Step 1: 写入新 toolbox.rs**

```rust
use crate::state::AppState;
use egui::Ui;

pub fn render(ui: &mut Ui, _state: &AppState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        // LZString tool
        egui::CollapsingHeader::new("\u{1f4e6} LZString \u{538b}\u{7f29}/\u{89e3}\u{538b}")
            .default_open(true)
            .show(ui, |ui| {
                lzstring_tool(ui);
            });

        ui.add_space(8.0);

        // Base64 tool
        egui::CollapsingHeader::new("\u{1f511} Base64 \u{7f16}\u{89e3}\u{7801}")
            .default_open(false)
            .show(ui, |ui| {
                base64_tool(ui);
            });

        ui.add_space(8.0);

        // Integrity check placeholder
        egui::CollapsingHeader::new("\u{1f50d} \u{5b58}\u{6863}\u{5b8c}\u{6574}\u{6027}\u{68c0}\u{67e5}")
            .default_open(false)
            .show(ui, |ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(139, 148, 158),
                    "\u{5f85}\u{5b9e}\u{73b0}...",
                );
            });

        ui.add_space(8.0);

        // Game directory scanner placeholder
        egui::CollapsingHeader::new("\u{1f4e1} \u{6e38}\u{620f}\u{76ee}\u{5f55}\u{626b}\u{63cf}")
            .default_open(false)
            .show(ui, |ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(139, 148, 158),
                    "\u{5f85}\u{5b9e}\u{73b0}...",
                );
            });
    });
}

fn lzstring_tool(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label("\u{6a21}\u{5f0f}:");
        ui.selectable_label(true, "\u{25cf} \u{538b}\u{7f29}");
    });

    ui.add_space(4.0);
    ui.label("\u{8f93}\u{5165}:");
    let input_id = ui.next_auto_id();
    let mut input = ui.data_mut(|d| d.get_temp::<String>(input_id).unwrap_or_default());
    ui.add_sized(
        [ui.available_width(), 120.0],
        egui::TextEdit::multiline(&mut input).hint_text("JSON \u{6216}\u{538b}\u{7f29}\u{6587}\u{672c}..."),
    );
    ui.data_mut(|d| d.insert_temp(input_id, input.clone()));

    ui.horizontal(|ui| {
        if ui.button("\u{6267}\u{884c}\u{538b}\u{7f29}").clicked() {
            let result = game_tool_core::lzstring::compress_to_base64(&input);
            let output_id = ui.next_auto_id();
            ui.data_mut(|d| d.insert_temp(output_id, result));
        }
        if ui.button("\u{6267}\u{884c}\u{89e3}\u{538b}").clicked() {
            let result = game_tool_core::lzstring::decompress_from_base64(&input);
            let output_id = ui.next_auto_id();
            ui.data_mut(|d| d.insert_temp(output_id, result.unwrap_or_default()));
        }
    });

    ui.add_space(4.0);
    ui.label("\u{7ed3}\u{679c}:");
    let output_id = ui.next_auto_id();
    let output = ui.data_mut(|d| d.get_temp::<String>(output_id).unwrap_or_default());
    ui.add_sized(
        [ui.available_width(), 80.0],
        egui::TextEdit::multiline(&mut output.clone()).interactive(false),
    );
}

fn base64_tool(ui: &mut Ui) {
    ui.label("\u{8f93}\u{5165}:");
    let input_id = ui.next_auto_id();
    let mut input = ui.data_mut(|d| d.get_temp::<String>(input_id).unwrap_or_default());
    ui.add_sized(
        [ui.available_width(), 80.0],
        egui::TextEdit::multiline(&mut input).hint_text("\u{6587}\u{672c}\u{6216} Base64..."),
    );
    ui.data_mut(|d| d.insert_temp(input_id, input.clone()));

    ui.horizontal(|ui| {
        if ui.button("\u{7f16}\u{7801}").clicked() {
            let result = game_tool_core::base64::encode(&input);
            let output_id = ui.next_auto_id();
            ui.data_mut(|d| d.insert_temp(output_id, result));
        }
        if ui.button("\u{89e3}\u{7801}").clicked() {
            let result = game_tool_core::base64::decode(&input);
            let output_id = ui.next_auto_id();
            ui.data_mut(|d| d.insert_temp(output_id, result.unwrap_or_default()));
        }
    });

    ui.add_space(4.0);
    ui.label("\u{7ed3}\u{679c}:");
    let output_id = ui.next_auto_id();
    let output = ui.data_mut(|d| d.get_temp::<String>(output_id).unwrap_or_default());
    ui.add_sized(
        [ui.available_width(), 60.0],
        egui::TextEdit::multiline(&mut output.clone()).interactive(false),
    );
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo build -p game-tool-gui 2>&1`
Expected: May have errors with `game_tool_core::lzstring` or `game_tool_core::base64` references. Check existing crate APIs. Fix import paths if needed.

---

### Task 15: 重写 settings.rs

**Files:**
- Rewrite: `crates/gui/src/panels/settings.rs`

- [ ] **Step 1: 写入新 settings.rs**

```rust
use crate::state::AppState;
use crate::theme::colors;
use egui::Ui;

pub enum SettingsAction {
    ToggleDarkMode,
    SetPort(u16),
}

pub fn render(ui: &mut Ui, state: &AppState) -> Vec<SettingsAction> {
    let mut actions = Vec::new();

    egui::ScrollArea::vertical().show(ui, |ui| {
        // Appearance
        ui.collapsing("\u{1f3a8} \u{5916}\u{89c2}", |ui| {
            ui.horizontal(|ui| {
                ui.label("\u{4e3b}\u{9898}:");
                let label = if state.dark_mode {
                    "\u{1f319} \u{6697}\u{8272}\u{6a21}\u{5f0f}"
                } else {
                    "\u{2600} \u{4eae}\u{8272}\u{6a21}\u{5f0f}"
                };
                if ui.button(label).clicked() {
                    actions.push(SettingsAction::ToggleDarkMode);
                }
            });
        });

        ui.add_space(8.0);

        // Connection
        ui.collapsing("\u{1f517} \u{8fde}\u{63a5}", |ui| {
            ui.horizontal(|ui| {
                ui.label("\u{4e3b}\u{673a}:");
                ui.label(&state.rt_panel.host);
            });
            ui.horizontal(|ui| {
                ui.label("\u{7aef}\u{53e3}:");
                let mut port = state.rt_panel.port;
                if ui.add(egui::DragValue::new(&mut port).range(1024..=65535)).changed() {
                    actions.push(SettingsAction::SetPort(port));
                }
                ui.label(format!("\u{8303}\u{56f4}: 1024-65535"));
            });
        });

        ui.add_space(8.0);

        // Recent games
        ui.collapsing("\u{1f4c1} \u{6700}\u{8fd1}\u{6e38}\u{620f}", |ui| {
            if state.recent_games.is_empty() {
                ui.colored_label(colors::TEXT_SECONDARY, "\u{6682}\u{65e0}\u{8bb0}\u{5f55}");
            } else {
                for path in &state.recent_games {
                    ui.horizontal(|ui| {
                        ui.label(path);
                        if ui.button("\u{2715} \u{79fb}\u{9664}").clicked() {
                            // Note: this requires mutability; we'll wire this later
                        }
                    });
                }
                ui.add_space(4.0);
                if ui.button("\u{1f5d1} \u{6e05}\u{9664}\u{5168}\u{90e8}\u{8bb0}\u{5f55}").clicked() {
                    // Note: ConfirmDialog for this will be wired later
                }
            }
        });

        ui.add_space(8.0);

        // Config
        ui.collapsing("\u{2699} \u{914d}\u{7f6e}", |ui| {
            if let Some(config_dir) = dirs_next::config_dir() {
                let path = config_dir.join("GameSaveEditor");
                ui.label(format!("\u{914d}\u{7f6e}\u{76ee}\u{5f55}: {}", path.display()));
                if ui.button("\u{1f4c2} \u{6253}\u{5f00}\u{76ee}\u{5f55}").clicked() {
                    let _ = open::that(&path);
                }
            }
        });

        ui.add_space(8.0);

        // About
        ui.collapsing("\u{2139} \u{5173}\u{4e8e}", |ui| {
            ui.label("GameSaveEditor v0.1.0");
            ui.colored_label(colors::TEXT_SECONDARY, "\u{8de8}\u{5f15}\u{64ce}\u{6e38}\u{620f}\u{5b58}\u{6863}\u{7f16}\u{8f91}\u{5668}");
            ui.add_space(4.0);
            ui.label("\u{652f}\u{6301}\u{5f15}\u{64ce}:");
            ui.label("  RPG Maker MV / MZ (NW.js)");
            ui.label("  Ren'Py");
            ui.label("  Unreal Engine (GVAS \u{53ea}\u{8bfb})");
            ui.label("  Unity / Godot (\u{901a}\u{7528} JSON)");
        });
    });

    actions
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo build -p game-tool-gui 2>&1`
Expected: May need `dirs_next` and `open` crates. Check if they're already in Cargo.toml; if not, add them.

---

## Phase 4: 打磨

### Task 16: 清理残留 + 全面编译测试

**Files:**
- Various

- [ ] **Step 1: 检查所有残留引用**

Run: `cd D:\AiCode\game_tool && rg "AppView|active_view|sidebar_collapsed|view_icon|view_name" crates/gui/src/ 2>&1`

Expected: NO RESULTS. If any remain, fix them.

- [ ] **Step 2: 检查 engine_display_name 引用**

Run: `cd D:\AiCode\game_tool && rg "fn engine_display" crates/gui/src/ 2>&1`

Expected: Only in `theme.rs`. If duplicated in sidebar.rs or dashboard.rs, those files should be deleted.

- [ ] **Step 3: 编译全局**

Run: `cargo build -p game-tool-gui 2>&1`
Expected: COMPILE SUCCESS with no warnings.

- [ ] **Step 4: 运行测试**

Run: `cargo test -p game-tool-gui 2>&1`
Expected: ALL TESTS PASS (may need to update test references to removed files).

- [ ] **Step 5: 运行完整 workspace 测试**

Run: `cargo test 2>&1`
Expected: ALL TESTS PASS.

---

### Task 17: 移除 dead_code 引用和清理

**Files:**
- Modify: `crates/gui/src/panels/mod.rs`

- [ ] **Step 1: 确认 mod.rs 只包含需要的模块**

Final state of `panels/mod.rs`:
```rust
pub mod backup;
pub mod realtime_editor;
pub mod save_editor;
pub mod settings;
pub mod startup;
pub mod status_bar;
pub mod tab_bar;
pub mod toolbox;
pub mod top_bar;
```

- [ ] **Step 2: 最终编译验证**

Run: `cargo build -p game-tool-gui 2>&1`
Expected: SUCCESS.

- [ ] **Step 3: 最终测试验证**

Run: `cargo test -p game-tool-gui 2>&1`
Expected: PASS.

---
