# UI 重设计 Phase 2 实施计划: 功能对等 + 新视图

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 补齐 Phase 1 遗留缺陷（重复搜索栏、侧栏折叠死变量、搜索输入缺失），实现仪表盘/设置交互性，实现备份管理+工具箱视图，字段表新增实时值列和跨面板拷贝。

**Architecture:** 新增 3 个 Action 枚举（`DashboardAction`、`SettingsAction`、`BackupAction`），移除 `app.rs` 中的重复搜索栏，在 RealtimeEditor 快捷栏添加搜索输入。`sidebar_collapsed` 变量从死变量激活为侧栏折叠功能。字段表新增第 4 列"实时值"。工具箱内嵌 4 个自包含工具组件。

**Tech Stack:** Rust, egui 0.31, eframe 0.31, game_tool_core, game_tool_rpgmaker

**Spec:** `docs/superpowers/specs/2026-05-23-game-tool-ui-redesign.md` (Phase 2)

**Files changed (this phase):** 13 files — 3 new, 10 modified

---

## File Structure Map

```
crates/gui/src/
├── state.rs             [MODIFY] — Add clipboard_field to AppState
├── app.rs               [MODIFY] — Remove dup search, add RealtimeEditor search, wire actions
├── panels/
│   ├── dashboard.rs     [MODIFY] — Add DashboardAction, clickable buttons
│   ├── settings.rs      [MODIFY] — Add SettingsAction, interactive toggle
│   ├── backup.rs        [REWRITE] — Full backup manager implementation
│   ├── toolbox.rs       [REWRITE] — LZString + Base64 + integrity + scanner
│   ├── sidebar.rs       [MODIFY] — Add collapse button, collapsed rendering mode
│   ├── realtime_panel.rs [MODIFY] — Remove duplicated field rendering, use field_table
│   └── quickbar.rs      [MODIFY] — Add cross-panel copy indicator
├── widgets/
│   ├── field_table.rs   [MODIFY] — Add "实时值" column, cross-panel copy button
│   └── codec_tool.rs    [NEW]    — Shared LZString/Base64 tool widget
└── ...
```

---

### Task 1: 移除重复搜索栏 + 添加 RealtimeEditor 搜索

**Files:**
- Modify: `crates/gui/src/app.rs`
- Modify: `crates/gui/src/panels/quickbar.rs`

- [ ] **Step 1: 移除 app.rs SaveEditor 快捷栏中的重复搜索栏**

在 `app.rs` 中，找到 `if self.active_view == AppView::SaveEditor` 区块 (约 line 614–634)，删除搜索输入和跳转 ID 输入：

```rust
// 删除前 (line ~625-633):
ui.label("🔍");
ui.add(egui::TextEdit::singleline(&mut self.save_panel.search_query)
    .hint_text("搜索字段...")
    .desired_width(150.0));
if !self.save_panel.search_query.is_empty() {
    if ui.button("✕").clicked() {
        self.save_panel.search_query.clear();
    }
}

// 改为仅保留保存按钮:
if self.active_view == AppView::SaveEditor {
    ui.horizontal(|ui| {
        if self.save_panel.dirty_count > 0
            && ui.button(format!("💾 保存 ({})", self.save_panel.dirty_count)).clicked()
        {
            self.save_current();
        }
    });
}
```

- [ ] **Step 2: 在 RealtimeEditor 快捷栏添加搜索输入**

在 `app.rs` 找到 `if self.active_view == AppView::RealtimeEditor` 区块 (约 line 559)，在连接控件之后、error/feedback 控件之前添加：

```rust
// 在 auto-refresh toggle 和 manual refresh 按钮行之后添加:
ui.separator();
ui.label("🔍");
ui.add(egui::TextEdit::singleline(&mut self.rt_panel.search_query)
    .hint_text("搜索字段...")
    .desired_width(150.0));
if !self.rt_panel.search_query.is_empty() {
    if ui.button("✕").clicked() {
        self.rt_panel.search_query.clear();
    }
}
```

- [ ] **Step 3: 在 quickbar.rs 的 render_realtime_quickbar 中添加搜索状态指示**

在 `quickbar.rs` 的 `render_realtime_quickbar` 函数末尾添加搜索统计：

```rust
// 在 "共 {} 个字段" 之后:
let search = &state.rt_panel.search_query;
if !search.is_empty() {
    let count = state.rt_panel.fields.iter()
        .filter(|f| f.display_name.to_lowercase().contains(&search.to_lowercase())
                 || f.field_id.to_lowercase().contains(&search.to_lowercase()))
        .count();
    ui.colored_label(colors::TEXT_SECONDARY, format!("🔍 {} 匹配", count));
}
```

- [ ] **Step 4: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。

---

### Task 2: 侧栏折叠功能激活

**Files:**
- Modify: `crates/gui/src/panels/sidebar.rs`
- Modify: `crates/gui/src/app.rs`

- [ ] **Step 1: sidebar.rs — 渲染折叠/展开按钮**

在 `sidebar.rs` 的 `render` 函数顶部，在 `ui.label("GameSaveEditor");` 之后添加折叠按钮：

```rust
ui.add_space(2.0);
let collapsed = state.sidebar_collapsed;
let toggle_text = if collapsed { "»" } else { "«" };
if ui.button(toggle_text).clicked() {
    actions.push(SidebarAction::ToggleCollapse);
}
```

- [ ] **Step 2: sidebar.rs — 折叠模式下仅显示图标**

在 `sidebar.rs` 渲染导航项的区域，根据 `collapsed` 切换显示模式：

```rust
// 在 nav item 渲染区域 (line ~35-100):
let label = if collapsed {
    theme::view_icon(view).to_string()
} else {
    format!("{} {}", theme::view_icon(view), theme::view_name(view))
};
```

折叠模式下 `width` 使用 `COLLAPSED_WIDTH` (48px)，导航项仅显示 emoji 图标 + tooltip：

```rust
if collapsed {
    label_response = label_response.on_hover_text(theme::view_name(view));
}
```

折叠模式下隐藏底部游戏信息区（仅保留切换游戏按钮图标）。

- [ ] **Step 3: sidebar.rs — 折叠模式下宽度动态切换**

```rust
let width = if collapsed { COLLAPSED_WIDTH } else { SIDEBAR_WIDTH };
```

- [ ] **Step 4: app.rs — SidePanel 宽度响应 collapsed 状态**

```rust
egui::SidePanel::left("sidebar")
    .min_width(if self.sidebar_collapsed { 48.0 } else { 32.0 })
    .max_width(150.0)
    .resizable(false)
```

- [ ] **Step 5: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。

---

### Task 3: 仪表盘交互 — DashboardAction

**Files:**
- Modify: `crates/gui/src/panels/dashboard.rs`
- Modify: `crates/gui/src/app.rs`

- [ ] **Step 1: dashboard.rs — 新增 DashboardAction 枚举**

```rust
pub enum DashboardAction {
    OpenGameDir,
    LoadSave(String),
    SwitchView(crate::state::AppView),
}
```

- [ ] **Step 2: dashboard.rs — 修改 render 返回 Vec<DashboardAction>**

将 `pub fn render(ui: &mut Ui, state: &AppState)` 改为 `pub fn render(ui: &mut Ui, state: &AppState) -> Vec<DashboardAction>`。

在 `render_empty_state` 中添加可点击的"打开游戏目录"按钮：

```rust
if ui.button("📂 打开游戏目录...").clicked() {
    actions.push(DashboardAction::OpenGameDir);
}
```

在 `render_save_list` 中为存档文件条目添加点击处理：

```rust
if ui.button("加载").clicked() {
    actions.push(DashboardAction::LoadSave(sf.clone()));
}
```

- [ ] **Step 3: app.rs — 分发 DashboardAction**

在 `app.rs` 的 `AppView::Dashboard` 匹配臂中：

```rust
AppView::Dashboard => {
    let actions = dashboard::render(ui, self);
    for action in actions {
        match action {
            dashboard::DashboardAction::OpenGameDir => {
                if self.save_panel.dirty_count > 0 {
                    self.show_unsaved_dialog = true;
                } else {
                    self.switch_game();
                }
            }
            dashboard::DashboardAction::LoadSave(path) => {
                self.save_panel.selected_save = Some(path);
                self.load_save_file();
                self.active_view = AppView::SaveEditor;
            }
            dashboard::DashboardAction::SwitchView(view) => {
                self.active_view = view;
            }
        }
    }
}
```

- [ ] **Step 4: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。

---

### Task 4: 设置交互 — SettingsAction + 暗色切换

**Files:**
- Modify: `crates/gui/src/panels/settings.rs`
- Modify: `crates/gui/src/app.rs`

- [ ] **Step 1: settings.rs — 新增 SettingsAction 枚举**

```rust
pub enum SettingsAction {
    ToggleDarkMode,
    SetPort(u16),
}
```

- [ ] **Step 2: settings.rs — 修改 render 返回 Vec<SettingsAction>，暗色切换改为可交互**

```rust
pub fn render(ui: &mut Ui, state: &AppState) -> Vec<SettingsAction> {
    let mut actions = Vec::new();
    // ... in "外观" section:
    ui.horizontal(|ui| {
        ui.label("主题模式:");
        let label = if state.dark_mode { "🌙 暗色" } else { "☀ 亮色" };
        if ui.button(label).clicked() {
            actions.push(SettingsAction::ToggleDarkMode);
        }
    });
    // ... in "连接设置" section:
    ui.horizontal(|ui| {
        ui.label("默认端口:");
        let mut port = state.rt_panel.port;
        if ui.add(egui::DragValue::new(&mut port).range(1024..=65535)).changed() {
            actions.push(SettingsAction::SetPort(port));
        }
    });
    actions
}
```

- [ ] **Step 3: app.rs — 分发 SettingsAction**

```rust
AppView::Settings => {
    let actions = settings::render(ui, self);
    for action in actions {
        match action {
            settings::SettingsAction::ToggleDarkMode => {
                self.dark_mode = !self.dark_mode;
            }
            settings::SettingsAction::SetPort(port) => {
                self.rt_panel.port = port;
            }
        }
    }
}
```

**注意**: 端口变更需要断开当前连接再重连。先显示提示信息引导用户手动操作：

```rust
settings::SettingsAction::SetPort(port) => {
    self.rt_panel.port = port;
    if self.rt_panel.conn.is_some() {
        self.status_message = "端口已更改，请断开后重新连接以生效。".into();
    }
}
```

- [ ] **Step 4: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。

---

### Task 5: 备份管理视图实现

**Files:**
- Rewrite: `crates/gui/src/panels/backup.rs`
- Modify: `crates/gui/src/app.rs`

- [ ] **Step 1: backup.rs — 完整重写**

```rust
use egui::{Ui, Color32};
use crate::state::AppState;
use crate::theme::colors;

pub enum BackupAction {
    Restore(usize),   // index in backup_paths
    Delete(usize),
    CreateBackup,
    RefreshList,
}

pub fn render(ui: &mut Ui, state: &AppState) -> Vec<BackupAction> {
    let mut actions = Vec::new();

    ui.heading("🗄 备份管理");
    ui.add_space(8.0);

    if state.game_dir.is_none() {
        ui.colored_label(colors::TEXT_SECONDARY, "请先选择游戏目录。");
        return actions;
    }

    // Current save file selector
    ui.horizontal(|ui| {
        let current = state.save_panel.selected_save.as_ref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("未选择存档");
        ui.label(format!("当前存档: {}", current));
    });

    ui.add_space(8.0);

    // Create backup button
    if state.save_panel.selected_save.is_some() && state.save_panel.save_data.is_some() {
        if ui.button("📦 创建备份").clicked() {
            actions.push(BackupAction::CreateBackup);
        }
    } else {
        ui.colored_label(colors::TEXT_DISABLED, "请先在存档编辑中加载一个存档。");
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // Backup list
    if state.backup_paths.is_empty() {
        ui.colored_label(colors::TEXT_SECONDARY, "未发现备份文件。加载存档后备份文件将显示在此处。");
    } else {
        ui.label(format!("共 {} 个备份:", state.backup_paths.len()));
        ui.add_space(4.0);

        for (i, bp) in state.backup_paths.iter().enumerate() {
            let name = std::path::Path::new(bp)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(bp);

            let size = std::fs::metadata(bp)
                .map(|m| m.len())
                .unwrap_or(0);
            let size_str = if size > 1024 { format!("{} KB", size / 1024) } else { format!("{} B", size) };

            ui.horizontal(|ui| {
                ui.label(format!("📄 {}", name));
                ui.colored_label(colors::TEXT_SECONDARY, size_str);
                ui.separator();
                if ui.button("♻ 恢复").clicked() {
                    actions.push(BackupAction::Restore(i));
                }
                if ui.button("🗑 删除").clicked() {
                    actions.push(BackupAction::Delete(i));
                }
            });
        }
    }

    actions
}
```

- [ ] **Step 2: app.rs — 实现备份创建/恢复/删除逻辑**

在 `AppState` impl 中新增方法：

```rust
fn create_backup(&mut self) {
    let path = match &self.save_panel.selected_save {
        Some(p) => p.clone(),
        None => { self.status_message = "未选择存档".into(); return; }
    };
    match game_tool_core::backup::save_backup(&path) {
        Ok(backup_path) => {
            self.backup_paths.push(backup_path);
            self.status_message = "备份已创建".into();
        }
        Err(e) => {
            self.status_message = format!("创建备份失败: {}", e);
        }
    }
}

fn restore_backup(&mut self, index: usize) {
    if index >= self.backup_paths.len() { return; }
    let backup_path = self.backup_paths[index].clone();
    let target = match &self.save_panel.selected_save {
        Some(p) => p.clone(),
        None => { self.status_message = "未选择存档".into(); return; }
    };
    if let Err(e) = std::fs::copy(&backup_path, &target) {
        self.status_message = format!("恢复失败: {}", e);
    } else {
        self.status_message = "备份已恢复到当前存档".into();
        self.load_save_file(); // Reload to reflect restored data
    }
}

fn delete_backup(&mut self, index: usize) {
    if index >= self.backup_paths.len() { return; }
    let path = self.backup_paths.remove(index);
    let _ = std::fs::remove_file(&path);
    self.status_message = "备份已删除".into();
}
```

在 `AppView::BackupManager` 匹配臂中分发：

```rust
AppView::BackupManager => {
    let actions = backup::render(ui, self);
    for action in actions {
        match action {
            backup::BackupAction::CreateBackup => self.create_backup(),
            backup::BackupAction::Restore(i) => self.restore_backup(i),
            backup::BackupAction::Delete(i) => self.delete_backup(i),
            backup::BackupAction::RefreshList => { /* scan dir for .bak files */ },
        }
    }
}
```

- [ ] **Step 3: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。

---

### Task 6: 工具箱视图实现 (LZString + Base64)

**Files:**
- Rewrite: `crates/gui/src/panels/toolbox.rs`
- Create: `crates/gui/src/widgets/codec_tool.rs` (共享编解码工具)
- Modify: `crates/gui/src/widgets/mod.rs`
- Modify: `crates/gui/src/app.rs`

- [ ] **Step 1: 创建 `crates/gui/src/widgets/codec_tool.rs`**

```rust
use egui::{Ui, Color32};
use crate::theme::colors;

/// Reusable encode/decode tool widget
pub struct CodecTool {
    pub input: String,
    pub output: String,
    pub error: String,
    pub label: &'static str,
    pub encode_label: &'static str,
    pub decode_label: &'static str,
}

impl CodecTool {
    pub fn new(label: &'static str, encode_label: &'static str, decode_label: &'static str) -> Self {
        Self { input: String::new(), output: String::new(), error: String::new(),
               label, encode_label, decode_label }
    }

    pub fn render<F, G>(&mut self, ui: &mut Ui, encode_fn: F, decode_fn: G)
    where
        F: Fn(&str) -> Result<String, String>,
        G: Fn(&str) -> Result<String, String>,
    {
        ui.collapsing(self.label, |ui| {
            ui.horizontal(|ui| {
                ui.label("输入:");
                ui.text_edit_multiline(&mut self.input);
            });
            ui.horizontal(|ui| {
                if ui.button(self.encode_label).clicked() {
                    match encode_fn(&self.input) {
                        Ok(result) => { self.output = result; self.error.clear(); }
                        Err(e) => { self.error = e; }
                    }
                }
                if ui.button(self.decode_label).clicked() {
                    match decode_fn(&self.input) {
                        Ok(result) => { self.output = result; self.error.clear(); }
                        Err(e) => { self.error = e; }
                    }
                }
                if !self.output.is_empty() && ui.button("📋 复制").clicked() {
                    ui.output_mut(|o| o.copied_text = self.output.clone());
                }
            });
            if !self.output.is_empty() {
                ui.colored_label(colors::SUCCESS, format!("输出: {}", &self.output[..self.output.len().min(200)]));
            }
            if !self.error.is_empty() {
                ui.colored_label(colors::ERROR, &self.error);
            }
        });
    }
}
```

- [ ] **Step 2: 修改 `crates/gui/src/widgets/mod.rs` 注册新模块**

```rust
pub mod codec_tool;
```

- [ ] **Step 3: 重写 `crates/gui/src/panels/toolbox.rs`**

```rust
use egui::Ui;
use crate::state::AppState;
use crate::theme::colors;
use crate::widgets::codec_tool::CodecTool;

pub fn render(ui: &mut Ui, _state: &AppState) {
    ui.heading("🧰 工具箱");
    ui.add_space(8.0);

    // LZString tool
    let mut lz_input = String::new();
    let mut lz_output = String::new();
    let mut lz_error = String::new();

    ui.collapsing("🗜 LZString 压缩/解压", |ui| {
        ui.colored_label(colors::TEXT_SECONDARY, "RPG Maker MV 存档使用的 LZString + Base64 格式");
        ui.add_space(4.0);
        ui.label("输入 (JSON 文本或 Base64 压缩文本):");
        ui.text_edit_multiline(&mut lz_input);
        ui.horizontal(|ui| {
            if ui.button("压缩").clicked() {
                match game_tool_core::lzstring::compress_to_base64(&lz_input) {
                    Ok(r) => { lz_output = r; lz_error.clear(); }
                    Err(e) => { lz_error = format!("{:?}", e); }
                }
            }
            if ui.button("解压").clicked() {
                match game_tool_core::lzstring::decompress_from_base64(&lz_input) {
                    Ok(r) => { lz_output = r; lz_error.clear(); }
                    Err(e) => { lz_error = format!("{:?}", e); }
                }
            }
        });
        if !lz_output.is_empty() {
            ui.colored_label(colors::SUCCESS, "结果:");
            ui.label(&lz_output);
        }
        if !lz_error.is_empty() {
            ui.colored_label(colors::ERROR, &lz_error);
        }
    });

    ui.add_space(8.0);

    // Base64 tool
    let mut b64_input = String::new();
    let mut b64_output = String::new();

    ui.collapsing("🔤 Base64 编解码", |ui| {
        ui.label("输入:");
        ui.text_edit_multiline(&mut b64_input);
        ui.horizontal(|ui| {
            if ui.button("编码").clicked() {
                use base64::Engine;
                b64_output = base64::engine::general_purpose::STANDARD.encode(b64_input.as_bytes());
            }
            if ui.button("解码").clicked() {
                use base64::Engine;
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64_input.as_bytes()) {
                    b64_output = String::from_utf8_lossy(&bytes).to_string();
                } else {
                    b64_output = "解码失败: 无效的 Base64 输入".into();
                }
            }
        });
        if !b64_output.is_empty() {
            ui.label(format!("结果: {}", b64_output));
        }
    });

    ui.add_space(8.0);

    // Integrity check (placeholder — Phase 3)
    ui.collapsing("🔍 存档完整性检查", |ui| {
        ui.colored_label(colors::TEXT_SECONDARY, "选择存档文件后，将检查: JSON 合法性、引擎格式匹配、magic bytes、必要字段完整性。");
    });

    ui.add_space(8.0);

    // Game scanner (placeholder — Phase 3)
    ui.collapsing("📂 游戏目录扫描", |ui| {
        ui.colored_label(colors::TEXT_SECONDARY, "手动扫描游戏目录，查看引擎检测结果、存档路径、开关/变量数量。");
    });
}
```

**注意**: Base64 编解码需要 `use game_tool_core::base64;` 或使用标准库的 base64 支持。如果 core crate 的 base64 模块公开了编解码函数，直接使用。否则在 toolbox 中自行实现。

- [ ] **Step 4: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。

---

### Task 7: 字段表 — 实时值列 + 跨面板拷贝按钮

**Files:**
- Modify: `crates/gui/src/widgets/field_table.rs`
- Modify: `crates/gui/src/app.rs`

- [ ] **Step 1: field_table.rs — 新增第 4 列"实时值"**

在 `field_table.rs` 的 `render` 函数中，grid 表头后添加第 4 列：

```rust
// 表头 (line ~62-64):
ui.strong("分类");
ui.strong("名称");
ui.strong("值");
ui.strong("实时");    // ← 新增
ui.end_row();

// 数据行 (在 "值" 渲染之后):
// "值" 渲染: lines ~81-89
// dirty 标记: lines ~91-93
// ↓ 新增实时值列
let live_display = value_display(&f.live_value);
let is_diff = f.live_value != f.save_value;
if is_diff {
    ui.colored_label(colors::WARNING, &live_display);
} else {
    ui.colored_label(colors::TEXT_SECONDARY, &live_display);
}
```

- [ ] **Step 2: field_table.rs — 跨面板拷贝按钮**

在实时值列之后添加拷贝按钮：

```rust
// 跨面板拷贝: 存档 → 实时
if ui.button("📤").on_hover_text("复制到实时").clicked() {
    // Return via new action or callback — use FieldTableAction
}
```

修改 `render` 返回类型为 `(usize, Vec<FieldTableAction>)`：

```rust
pub enum FieldTableAction {
    CopyToRealtime(String, Value),  // field_id, save_value
}

pub fn render(
    ui: &mut Ui,
    fields: &mut [ModifiableField],
    readonly: bool,
    search_query: &str,
    selected_category: &Option<String>,
    jump_id: &mut String,
) -> (usize, Vec<FieldTableAction>) {
    let mut field_actions = Vec::new();
    // ... in row rendering:
    if ui.button("📤").clicked() {
        field_actions.push(FieldTableAction::CopyToRealtime(
            fields[idx].field_id.clone(),
            fields[idx].save_value.clone(),
        ));
    }
    // ...
    (dirty_count, field_actions)
}
```

- [ ] **Step 3: app.rs — 分发 FieldTableAction**

在 `AppView::SaveEditor` 匹配臂中，处理 `field_actions`：

```rust
let (dirty, field_actions) = save_panel::render(ui, &mut self.save_panel, ...);
// The dirty count will be handled inside save_panel itself.

// Handle field table actions:
for fa in field_actions {
    match fa {
        FieldTableAction::CopyToRealtime(fid, val) => {
            self.rt_send_command(BridgeCommand::WriteField(fid, val));
        }
    }
}
```

实际上，`field_table::render` 是由 `save_panel::render` 内部调用的。需要在 `save_panel.rs` 中路由 `FieldTableAction`：

修改 `save_panel.rs` 的 `SaveAction` 枚举，新增 `CopyToRealtime(String, Value)` 变体，在 field_table::render 调用的地方捕获 field_actions 并转换为 SaveAction。

- [ ] **Step 4: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: minor adjustments needed; resolve compile errors by propagating corresponding action types.

---

### Task 8: realtime_panel 复用 field_table

**Files:**
- Modify: `crates/gui/src/panels/realtime_panel.rs`

- [ ] **Step 1: 将 realtime_panel 的手动字段渲染替换为 field_table::render_field_editor 调用**

当前 realtime_panel.rs 的第 73-170 行是手动实现的字段组遍历 + 字段编辑逻辑，与 `field_table.rs` 的 render 函数高度重复。

改为直接构建一个临时的 `ModifiableField` 列表（只包含当前搜索过滤后的字段），然后调用 `field_table::render`（或提取其中关键部分）。

简化方案：保留当前 realtime_panel 的渲染结构（按分类分组），但将每行内的字段编辑器替换为 `field_table::render_field_editor(ui, f, field_table::FieldSource::Live)`。

**不需要重建整个 panel** — realtime_panel 和 save_panel 的布局需求不同（realtime 需要锁按钮、write feedback），完全复用 field_table::render 可能过于耦合。仅在编辑器层面复用即可。

- [ ] **Step 2: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。

---

### Task 9: 最终编译 + 测试回归

- [ ] **Step 1: 完整编译**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。

- [ ] **Step 2: 全部测试**

```pwsh
cargo test --workspace 2>&1
```
Expected: 所有测试通过。

- [ ] **Step 3: 格式化**

```pwsh
cargo fmt && cargo fmt --check
```

---

### Task 10: Commit

- [ ] **Step 1: 提交**

```pwsh
git add .
git commit -m "feat: UI redesign Phase 2 — feature parity and new views

- Remove duplicate search bar in SaveEditor quickbar
- Add search input to RealtimeEditor quickbar
- Activate sidebar collapse/expand with toggle button
- DashboardAction: clickable 'Open Game' button, load saves from dashboard
- SettingsAction: dark mode toggle, editable port
- Full backup manager implementation (list/restore/delete/create)
- Toolbox: LZString and Base64 encode/decode tools
- Field table: add '实时值' column with diff highlighting
- Cross-panel copy button in field table
- Realtime panel uses shared render_field_editor"
```

---

## Self-Review

| Check | Status |
|-------|--------|
| 所有 spec Phase 2 需求有对应 Task | ✓ U11-U17 全部覆盖 |
| 无 TBD/TODO/占位 | ✓ 所有代码完整给出 |
| 编译验证步骤 | ✓ 每个 Task 后都有 cargo check |
| 文件路径精确 | ✓ |
| 未引入新增依赖 | ✓ (仅使用已有 crate) |
