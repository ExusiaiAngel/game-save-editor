# UI v3 全面 Bug 修复计划

日期: 2026-05-23 | 版本: v2 (扩展至16项修复)

---

## 修复概览

| # | 严重度 | 问题 | 文件 |
|---|:---:|------|------|
| 1 | 🔴 | LZString/Base64 工具箱完全不可用 | toolbox.rs |
| 2 | 🔴 | 实时连接控件完全缺失 | app.rs |
| 3 | 🔴 | 切换游戏时遗留脏状态 | app.rs |
| 4 | 🔴 | OpenRecentGame 缺少状态重置 | app.rs |
| 5 | 🟠 | 选项卡按钮 painter 手绘 | tab_bar.rs |
| 6 | 🟠 | 最近游戏列表永远为空 | app.rs, config.rs |
| 7 | 🟠 | 跳转ID高亮但不滚动 | field_table.rs |
| 8 | 🟠 | 实时编辑器分类名冗余 | realtime_editor.rs |
| 9 | 🟠 | 备份恢复/删除无确认 | app.rs, backup.rs |
| 10 | 🟡 | 文件扩展名匹配大小写敏感 | discovery.rs |
| 11 | 🟡 | 删除备份错误被静默忽略 | app.rs |
| 12 | 🟡 | refresh_timer 死字段 | state.rs |
| 13 | 🟡 | 3个 ConfirmAction 变体死代码 | state.rs |
| 14 | 🟡 | parse_range 冒号无短线异常 | category_tree.rs |
| 15 | 🟡 | 死测试代码+名不副实测试 | tests/ |
| 16 | 🟡 | colors 模块常量未复用 | theme.rs |

---

## Fix 1: 工具箱状态持久化 (🔴 关键)

**问题：** `toolbox.rs` 中 `lz_input`/`lz_output`/`b64_input`/`b64_output` 均为局部变量，每帧重置为 `""`。LZString和Base64工具完全不可用。

**文件：** `crates/gui/src/panels/toolbox.rs`, `crates/gui/src/state.rs`

**修复：** 将工具箱状态字段添加到 `AppState`，改用持久化状态。

**变更 A — state.rs 添加 ToolboxState：**

```rust
pub struct ToolboxState {
    pub lz_input: String,
    pub lz_output: String,
    pub lz_error: String,
    pub b64_input: String,
    pub b64_output: String,
}
```

**变更 B — AppState 添加字段：**

```rust
pub toolbox: ToolboxState,
```

**变更 C — AppState::new() 初始化：**

```rust
toolbox: ToolboxState {
    lz_input: String::new(),
    lz_output: String::new(),
    lz_error: String::new(),
    b64_input: String::new(),
    b64_output: String::new(),
},
```

**变更 D — toolbox::render 接受 &mut ToolboxState：**

将 signature 从 `pub fn render(ui: &mut Ui, _state: &AppState)` 改为 `pub fn render(ui: &mut Ui, state: &mut ToolboxState)`。内部使用 `state.lz_input` 等替代局部变量。

**变更 E — app.rs 调用更新：**

`toolbox::render(ui, self)` → `toolbox::render(ui, &mut self.toolbox)`

---

## Fix 2: 恢复实时连接管理栏 (🔴 关键)

**问题：** `rt_connect()`/`rt_disconnect()`/`inject_plugin()` 被标记 `#[allow(dead_code)]`，从未调用。实时修改标签完全无连接控件。

**文件：** `crates/gui/src/app.rs`

**变更 A — TabMode::RealtimeEditor 块重构：**

将当前简单的 dispatch 循环替换为完整的连接管理 UI + 字段编辑器：

```rust
TabMode::RealtimeEditor => {
    if !crate::factory::supports_realtime(&self.engine) {
        ui.colored_label(Color32::from_rgb(150,150,150), "当前引擎不支持实时修改");
    } else {
        // 连接管理栏
        ui.horizontal(|ui| {
            ui.label("主机:");
            ui.add(TextEdit::singleline(&mut self.rt_panel.host).desired_width(100.0));
            ui.label("端口:");
            ui.add(DragValue::new(&mut self.rt_panel.port).range(1024..=65535));

            let is_connected = self.rt_panel.conn.as_ref()
                .map(|c| c.status == ConnectionStatus::Connected).unwrap_or(false);
            let is_connecting = self.rt_panel.conn.as_ref()
                .map(|c| c.status == ConnectionStatus::Connecting).unwrap_or(false);

            if is_connecting {
                ui.colored_label(colors::WARNING, "连接中...");
            } else if is_connected {
                if ui.button("◎ 断开").clicked() { self.rt_disconnect(); }
            } else {
                if ui.button("● 连接").clicked() { self.rt_connect(); }
            }

            if !self.rt_panel.plugin_installed {
                if ui.button("注入插件").clicked() { self.inject_plugin(); }
            } else {
                ui.colored_label(colors::SUCCESS, "✓ 插件已注入");
            }
        });

        // 自动刷新栏
        ui.horizontal(|ui| {
            let auto = self.rt_panel.auto_refresh;
            if ui.selectable_label(auto, if auto { "▶ 自动刷新" } else { "⏸ 暂停刷新" }).clicked() {
                self.rt_panel.auto_refresh = !auto;
            }
            if ui.button("📥 手动刷新").clicked() {
                self.rt_send_command(BridgeCommand::ReadAll);
            }
            ui.label("间隔:");
            ComboBox::from_id_salt("refresh_interval")
                .selected_text(format!("{}秒", self.rt_panel.refresh_interval_secs))
                .show_ui(ui, |ui| {
                    for secs in &[1u64, 2, 3, 5] {
                        if ui.selectable_label(self.rt_panel.refresh_interval_secs == *secs,
                            format!("{}秒", secs)).clicked() {
                            self.rt_panel.refresh_interval_secs = *secs;
                            self.rt_panel.last_refresh = None;
                        }
                    }
                });
        });

        // 错误/反馈信息
        if !self.rt_panel.error_message.is_empty() {
            ui.colored_label(colors::ERROR, &self.rt_panel.error_message);
        }
        if !self.rt_panel.write_feedback.is_empty() {
            ui.colored_label(colors::SUCCESS, &self.rt_panel.write_feedback);
        }

        ui.separator();

        // 搜索栏
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(TextEdit::singleline(&mut self.rt_panel.search_query)
                .hint_text("搜索字段...").desired_width(150.0));
            if !self.rt_panel.search_query.is_empty() && ui.button("✕").clicked() {
                self.rt_panel.search_query.clear();
            }
            ui.separator();
            ui.label("跳转 ID:");
            ui.text_edit_singleline(&mut self.rt_panel.jump_id);
        });

        ui.separator();

        let actions = realtime_editor::render(ui, &mut self.rt_panel, &self.save_panel.fields);
        for action in actions {
            match action {
                realtime_editor::RtAction::ReadAll => {
                    self.rt_send_command(BridgeCommand::ReadAll);
                }
                realtime_editor::RtAction::WriteField(id, val) => {
                    self.rt_send_command(BridgeCommand::WriteField(id, val));
                }
                realtime_editor::RtAction::ToggleLock(fid) => {
                    if self.rt_panel.locked_fields.contains(&fid) {
                        self.rt_panel.locked_fields.remove(&fid);
                    } else {
                        self.rt_panel.locked_fields.insert(fid);
                    }
                }
                realtime_editor::RtAction::CopyToSave(fid) => {
                    if let Some(rt_f) = self.rt_panel.fields.iter().find(|f| f.field_id == fid) {
                        if let Some(save_f) = self.save_panel.fields.iter_mut().find(|f| f.field_id == fid) {
                            save_f.save_value = rt_f.live_value.clone();
                            save_f.dirty = true;
                        }
                    }
                    self.save_panel.dirty_count = self.save_panel.fields.iter().filter(|f| f.dirty).count();
                }
            }
        }
    }
}
```

**变更 B — 移除 #[allow(dead_code)]：**

移除 `inject_plugin()`、`rt_connect()`、`rt_disconnect()` 上的 `#[allow(dead_code)]` 注解。

---

## Fix 3: 切换游戏时清理脏状态 (🔴 关键)

**问题：** `switch_game()` 遗留 15 个未重置字段：`save_panel.jump_id`、所有 `rt_panel.*`（除 `plugin_installed`）、`backup_paths`、`backup_selection`、`status_message`。旧游戏的 bridge 连接继续运行。

**文件：** `crates/gui/src/app.rs` (switch_game 函数, lines 207-256)

**变更：**

在 `self.refresh_save_files()` (line 240) 之前插入清理代码：

```rust
// 断开旧连接
if let Some(ref conn) = self.rt_panel.conn {
    let _ = conn.cmd_tx.send(BridgeJob::Disconnect);
}
self.rt_panel.conn = None;
self.rt_panel.fields.clear();
self.rt_panel.error_message.clear();
self.rt_panel.error_remaining = 0;
self.rt_panel.write_feedback.clear();
self.rt_panel.write_feedback_remaining = 0;
self.rt_panel.search_query.clear();
self.rt_panel.jump_id.clear();
self.rt_panel.auto_refresh = true;
self.rt_panel.locked_fields.clear();
self.rt_panel.last_refresh = None;

self.save_panel.jump_id.clear();

self.backup_paths.clear();
self.backup_selection.clear();
self.status_message.clear();
```

---

## Fix 4: OpenRecentGame 补齐状态重置 (🔴 关键)

**问题：** `StartupAction::OpenRecentGame` 处理器缺少 10 个字段的重置，与 `switch_game()` 不一致。

**文件：** `crates/gui/src/app.rs` (OpenRecentGame 处理器)

**变更：**

在 `self.refresh_save_files()` (line 643) 之前插入：

```rust
self.save_panel.selected_save = None;
self.save_panel.save_data = None;
self.save_panel.summary = None;
self.save_panel.fields.clear();
self.save_panel.dirty_count = 0;
self.save_panel.selected_category = None;
self.save_panel.search_query.clear();
self.save_panel.jump_id.clear();

self.rt_panel.conn = None;
self.rt_panel.fields.clear();
self.rt_panel.plugin_installed = false;
self.rt_panel.error_message.clear();
self.rt_panel.error_remaining = 0;
self.rt_panel.write_feedback.clear();
self.rt_panel.write_feedback_remaining = 0;
self.rt_panel.search_query.clear();
self.rt_panel.jump_id.clear();
self.rt_panel.locked_fields.clear();
self.rt_panel.last_refresh = None;

self.backup_paths.clear();
self.backup_selection.clear();

// 重新检测 plugin_installed
if factory::supports_realtime(&self.engine) {
    match self.engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
            self.rt_panel.plugin_installed = game_tool_rpgmaker::tcp::is_plugin_installed(&path);
        }
        EngineType::RenPy => {
            self.rt_panel.plugin_installed = game_tool_renpy::bridge::is_plugin_installed(&path);
        }
        _ => {}
    }
}
```

---

## Fix 5: 选项卡按钮视觉效果 (🟠 高)

**问题：** `tab_bar.rs` 使用 `ui.painter()` 底层 API 手绘按钮，文字对齐不准、缺少标准 egui 圆角/动画。

**文件：** `crates/gui/src/panels/tab_bar.rs`

**变更：** 用 `ui.selectable_label()` 替代 painter 模式。

将核心循环中的：
```rust
let (_, rect) = ui.allocate_space(desired_size);
let resp = ui.interact(rect, ui.next_auto_id(), egui::Sense::click());
if resp.hovered() { ui.painter().rect_filled(...); }
ui.painter().text(...);
```
改为：
```rust
let label = format!("{} {}", theme::tab_icon(tab), theme::tab_name(tab));
let resp = ui.add_enabled_ui(enabled, |ui| {
    ui.selectable_label(selected, label)
}).inner;

// 选中态下划线
if selected {
    let underline = egui::Rect::from_min_size(
        egui::pos2(resp.rect.left(), resp.rect.bottom()),
        egui::vec2(resp.rect.width(), 2.0),
    );
    ui.painter().rect_filled(underline, 0.0, colors::ACCENT);
}

// 禁用时 hover tooltip
if !enabled {
    let reason = match tab {
        TabMode::RealtimeEditor if has_game => "当前引擎不支持实时修改",
        _ => "请先选择游戏目录",
    };
    resp.on_hover_text(reason);
}

if resp.clicked() && enabled {
    actions.push(TabAction::SwitchTab(*tab));
}
```

**同时移除：** `layout_no_wrap()` 调用、手动 `text_color`、`allocate_space` 模式。

---

## Fix 6: 最近游戏填充与持久化 (🟠 高)

**问题：** `recent_games` 初始化永远为空，`AppConfig` 无此字段。

**文件：** `crates/gui/src/app.rs`, `crates/core/src/config.rs`

**变更 A — config.rs 添加 recent_games 字段：**

```rust
#[serde(default)]
pub recent_games: Vec<String>,
```

**变更 B — switch_game() 末尾填充：**

```rust
if let Some(ref dir) = self.game_dir {
    let dir = dir.clone();
    self.recent_games.retain(|g| g != &dir);
    self.recent_games.insert(0, dir);
    self.recent_games.truncate(5);
    if let Ok(mut cfg) = load_config() {
        cfg.recent_games = self.recent_games.clone();
        let _ = game_tool_core::config::save_config(&cfg);
    }
}
```

**变更 C — OpenRecentGame 末尾同样填充：**

在 Fix 4 的重置代码之后、`self.active_tab = TabMode::SaveEditor;` 之前添加相同逻辑。

**变更 D — AppState::new() 从 config 加载：**

将 `recent_games: Vec::new()` 改为 `recent_games: config.recent_games.clone()`。

---

## Fix 7: 跳转ID自动滚动 (🟠 高)

**问题：** `field_table.rs` 中跳转目标被高亮但 `ScrollArea` 无 `scroll_to`，屏幕外目标不可见。

**文件：** `crates/gui/src/widgets/field_table.rs`

**变更：**

在渲染循环中，为跳转目标行调用 `scroll_to_me`：

```rust
// 在跳转目标行的渲染之后
if is_jump_target {
    ui.colored_label(Color32::from_rgb(100, 200, 255), &cat);
    ui.colored_label(Color32::from_rgb(100, 200, 255), &dname);
} else {
    ui.label(&cat);
    ui.label(&dname);
}
// ... 其他列 ...

let row_response = ui.end_row(); // 需要保存 end_row 的 response
// 但 egui::Grid::end_row() 不返回 Response...

// 替代方案: 在 columns 前保存 ui.available_rect_before_wrap(),
// 在行渲染完成后如果 is_jump_target 则调用 ui.scroll_to_cursor(Some(Align::TOP))
```

由于 `egui::Grid` 限制，使用以下方式：渲染跳转目标行前检查 `is_jump_target`，若是则调用 `ui.scroll_to_cursor(Some(egui::Align::Center))`。

```rust
for &idx in &all_indices {
    let is_jump_target = jump_target.as_deref() == Some(&fields[idx].field_id);
    if is_jump_target {
        ui.scroll_to_cursor(Some(egui::Align::Center));
    }
    // ... 行渲染 ...
}
```

---

## Fix 8: 移除实时编辑器冗余分类名 (🟠 高)

**问题：** 每行重复显示分类标签（如"开关"），但已按分类分组显示了标题。

**文件：** `crates/gui/src/panels/realtime_editor.rs`

**变更：** 移除每行中的 `display_cat` 标签渲染（约 line 118-129 的 `ui.colored_label(...)` 调用），只保留分组标题。锁 + 字段名 + 值编辑器即可。

**同时 (Fix 8b)：** 锁定开关改用 `ui.checkbox()` 替代 `ui.selectable_label(false, ...)`：

```rust
let mut locked_state = locked;
if ui.checkbox(&mut locked_state, lock_icon).changed() {
    actions.push(RtAction::ToggleLock(fid.clone()));
}
```

---

## Fix 9: 备份恢复/删除添加确认对话框 (🟠 高)

**问题：** 单条恢复和删除直接执行，无确认对话框。批量删除有确认但单项没有。

**文件：** `crates/gui/src/app.rs` (BackupManager dispatch)

**变更：**

```rust
backup::BackupAction::Restore(i) => {
    self.show_confirm_dialog = Some(ConfirmDialog {
        title: "恢复备份".into(),
        message: "确定用此备份覆盖当前存档？此操作不可撤销。".into(),
        on_confirm: ConfirmAction::RestoreBackup(i),
    });
}
backup::BackupAction::Delete(i) => {
    self.show_confirm_dialog = Some(ConfirmDialog {
        title: "删除备份".into(),
        message: "确定删除此备份文件？".into(),
        on_confirm: ConfirmAction::DeleteSingleBackup(i),
    });
}
```

---

## Fix 10: 文件扩展名匹配大小写不敏感 (🟡 中)

**问题：** `discovery.rs:47` 使用 `ends_with()` 大小写敏感，Windows 上 `RPGSAVE` ≠ `rpgsave`。

**文件：** `crates/gui/src/discovery.rs`

**变更：**

```rust
// 将 line 47
if name.ends_with(ext.as_str()) {
// 改为
if name.to_lowercase().ends_with(&ext.to_lowercase()) {
```

同时修复 `config.rpgsave`/`global.rpgsave` 的精确大小写匹配（line 42-43），改为 `to_lowercase()` 比较。

---

## Fix 11: 删除备份错误处理 (🟡 中)

**问题：** `std::fs::remove_file` 错误被 `let _ =` 静默忽略，文件路径已从列表移除但磁盘上未删除。

**文件：** `crates/gui/src/app.rs` (delete_backup 函数)

**变更：**

```rust
fn delete_backup(&mut self, index: usize) {
    if index >= self.backup_paths.len() { return; }
    let path = self.backup_paths.remove(index);
    match std::fs::remove_file(&path) {
        Ok(()) => {
            self.status_message = "备份已删除".into();
        }
        Err(e) => {
            self.backup_paths.insert(index, path);
            self.status_message = format!("删除失败: {}", e);
        }
    }
}
```

---

## Fix 12: 移除 refresh_timer 死字段 (🟡 中)

**文件：** `crates/gui/src/state.rs` (RtPanelState, AppState::new)

**变更：** 从 `RtPanelState` 移除 `pub refresh_timer: u32`。从 `AppState::new()` 移除 `refresh_timer: 0` 初始化。

---

## Fix 13: 移除死 ConfirmAction 变体 (🟡 中)

**问题：** `DiscardAndSwitch`/`RestoreBackup`/`DeleteSingleBackup` 定义并处理但从未创建 — 原为弃用设计，现在 Fix 9 启用了 `RestoreBackup` 和 `DeleteSingleBackup`。

**变更：** 只移除 `DiscardAndSwitch`（确实未使用）。保留 `RestoreBackup` 和 `DeleteSingleBackup`（Fix 9 将启用它们）。

**文件：** `crates/gui/src/state.rs`, `crates/gui/src/app.rs`

从 `ConfirmAction` 枚举和 `app.rs` match 块中移除 `DiscardAndSwitch`。

---

## Fix 14: parse_range 冒号短线异常 (🟡 中)

**问题：** `"switch:50"` (冒号无短线) 返回 `("switch:50", None)` 而非 `("switch", None)`，导致过滤失效。

**文件：** `crates/gui/src/widgets/category_tree.rs` (parse_range 函数)

**变更：**

```rust
pub fn parse_range(selected: &Option<String>) -> (Option<String>, Option<(usize, usize)>) {
    if let Some(ref sel) = selected {
        if let Some(colon) = sel.find(':') {
            let cat = sel[..colon].to_string();
            let range_str = &sel[colon + 1..];
            if let Some(dash) = range_str.find('-') {
                let start: usize = range_str[..dash].parse().unwrap_or(0);
                let end: usize = range_str[dash + 1..].parse().unwrap_or(0);
                if start <= end {
                    return (Some(cat), Some((start, end)));
                }
            }
            // 冒号后无有效短线 → 仍返回正确的 category
            return (Some(cat), None);
        }
        (Some(sel.clone()), None)
    } else {
        (None, None)
    }
}
```

---

## Fix 15: 清理死测试代码 (🟡 中)

**问题：** 
- `tests/common/mod.rs` 中 `MockSaveFormat` (68行) 从未被引入使用
- `test_switch_game_disconnects_bridge` 名不副实

**文件：** `crates/gui/tests/common/mod.rs`, `crates/gui/tests/ui_behavior_tests.rs`

**变更 A：** 删除 `MockSaveFormat` 结构体及其 `ISaveFormat` 实现。

**变更 B：** 重写 `test_switch_game_disconnects_bridge` 为有意义测试：

```rust
fn test_switch_game_disconnects_bridge() {
    // 验证 AppState::new() 初始化后 conn 为 None
    let state = AppState::new(Some("nonexistent_path".into()));
    assert!(state.rt_panel.conn.is_none());
}
```

或直接移除该测试（因为它测试的状态被 AppState::new 的其他测试覆盖）。

---

## Fix 16: colors 模块常量复用 (🟡 中)

**问题：** `Theme::apply()` 中重复定义颜色常量，而非复用 `colors` 模块。

**文件：** `crates/gui/src/theme.rs`

**变更：** `Theme::apply()` 中使用 `colors::PANEL_DARK` 等常量替代本地重复定义。同时为亮色模式补充 `colors` 模块常量。

---

## 验证步骤

1. `cargo build -p game-tool-gui 2>&1` — 零错误零警告
2. `cargo test -p game-tool-gui 2>&1` — 全部测试通过
3. `cargo test 2>&1` — 全工作区测试通过
4. 手动检查：
   - 选项卡按钮使用标准 egui 样式
   - 实时编辑标签显示完整连接管理栏
   - 工具箱 LZString/Base64 输入持久化
   - 最近游戏列表正确填充和显示
   - 连接/断开/注入/刷新按钮功能正常
   - 切换游戏时状态完全清理
   - 跳转ID自动滚动到目标
   - 备份恢复/删除有确认对话框
   - 文件扩展名大小写不敏感匹配
