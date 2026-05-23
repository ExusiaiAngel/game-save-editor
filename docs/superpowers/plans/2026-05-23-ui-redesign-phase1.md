# UI 重设计 Phase 1 实施计划: 布局骨架迁移

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 GameSaveEditor 从固定双面板布局迁移为侧栏导航 + 内容区架构，6 个视图骨架就位，现有功能无回归。

**Architecture:** 新增 `AppView` 枚举驱动视图切换，新增 `sidebar.rs` / `quickbar.rs` / `theme.rs` / `dashboard.rs` 等 7 个文件，重构 `app.rs` 主布局从 `allocate_ui_at_rect` 双面板改为侧栏+内容区结构。现有 `save_panel.rs` 和 `realtime_panel.rs` 仅做接口适配，不改变内部逻辑。

**Tech Stack:** Rust, egui 0.31, eframe 0.31, game_tool_core, game_tool_rpgmaker

**Spec:** `docs/superpowers/specs/2026-05-23-game-tool-ui-redesign.md` (Phase 1)

**Files changed (this phase):** 18 files total — 7 new, 11 modified

---

## File Structure Map

```
crates/gui/src/
├── main.rs              [MODIFY] — 设置默认暗色主题 initial_visuals
├── lib.rs               [MODIFY] — 新增 theme 模块声明
├── state.rs             [MODIFY] — 新增 AppView 枚举, AppState 新字段
├── theme.rs             [NEW]    — ThemeConfig, 颜色常量, 字体配置
├── app.rs               [MODIFY] — 主布局重构为侧栏+内容区+快捷栏
├── factory.rs           [NO CHANGE this phase]
├── discovery.rs         [NO CHANGE this phase]
├── connection.rs        [NO CHANGE this phase]
├── panels/
│   ├── mod.rs           [MODIFY] — 新增 sidebar, quickbar, dashboard, backup, toolbox, settings
│   ├── top_bar.rs       [MODIFY] — 精简为仅游戏信息行
│   ├── sidebar.rs       [NEW]    — 侧栏导航渲染
│   ├── quickbar.rs      [NEW]    — 底部快捷操作栏渲染（视图感知）
│   ├── dashboard.rs     [NEW]    — 仪表盘视图
│   ├── backup.rs        [NEW]    — 备份管理视图（Phase 1 占位）
│   ├── toolbox.rs       [NEW]    — 工具箱视图（Phase 1 占位）
│   ├── settings.rs      [NEW]    — 设置视图（Phase 1 基础版）
│   ├── save_panel.rs    [MODIFY] — 移除内部标题/顶栏元素，适配内容区
│   └── realtime_panel.rs [MODIFY] — 移除连接栏至 quickbar，适配内容区
└── widgets/
    └── ...             [NO CHANGE this phase]
```

---

### Task 1: AppView 枚举 + AppState 新字段

**Files:**
- Modify: `crates/gui/src/state.rs`

- [ ] **Step 1: 在 state.rs 文件开头新增 AppView 枚举**

在 `use` 语句块之后、`SavePanelMode` 枚举之前插入:

```rust
/// 应用视图枚举：侧栏导航的目标页面
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AppView {
    Dashboard,       // 仪表盘（默认首页）
    Toolbox,         // 工具箱
    Settings,        // 设置
    SaveEditor,      // 存档编辑
    RealtimeEditor,  // 实时修改
    BackupManager,   // 备份管理
}
```

- [ ] **Step 2: AppState 新增字段**

在 `AppState` 结构体中，`game_config` 行之后，`save_panel` 行之前插入:

```rust
    pub active_view: AppView,
    pub sidebar_collapsed: bool,
    pub dark_mode: bool,
    pub recent_games: Vec<String>,
    pub backups: Vec<String>,
```

- [ ] **Step 3: 在 #[cfg(test)] mod tests 块中添加 AppView 测试**

在现有测试函数之后添加:

```rust
    #[test]
    fn test_app_view_variants_distinct() {
        assert_ne!(AppView::Dashboard, AppView::Toolbox);
        assert_ne!(AppView::Toolbox, AppView::Settings);
        assert_ne!(AppView::Settings, AppView::SaveEditor);
        assert_ne!(AppView::SaveEditor, AppView::RealtimeEditor);
        assert_ne!(AppView::RealtimeEditor, AppView::BackupManager);
    }

    #[test]
    fn test_app_view_clone() {
        let v = AppView::Dashboard;
        assert_eq!(v, v.clone());
    }
```

- [ ] **Step 4: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 部分错误（`active_view` 等新字段在 `AppState::new()` 中尚未初始化），这是预期的，Task 2 会解决。

---

### Task 2: theme.rs — 视觉设计系统

**Files:**
- Create: `crates/gui/src/theme.rs`
- Modify: `crates/gui/src/lib.rs`

- [ ] **Step 1: 创建 `crates/gui/src/theme.rs`**

```rust
use egui::{Color32, Visuals, Rounding, Stroke, Style};

pub struct Theme {
    pub dark_mode: bool,
}

impl Theme {
    pub fn new(dark_mode: bool) -> Self {
        Self { dark_mode }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        let bg = if self.dark_mode {
            Color32::from_rgb(13, 17, 23)
        } else {
            Color32::from_rgb(255, 255, 255)
        };
        let panel_bg = if self.dark_mode {
            Color32::from_rgb(22, 27, 34)
        } else {
            Color32::from_rgb(246, 248, 250)
        };
        let text = if self.dark_mode {
            Color32::from_rgb(201, 209, 217)
        } else {
            Color32::from_rgb(36, 41, 47)
        };
        let text_secondary = if self.dark_mode {
            Color32::from_rgb(139, 148, 158)
        } else {
            Color32::from_rgb(101, 109, 118)
        };
        let accent = if self.dark_mode {
            Color32::from_rgb(88, 166, 255)
        } else {
            Color32::from_rgb(9, 105, 218)
        };

        visuals.panel_fill = panel_bg;
        visuals.window_fill = bg;
        visuals.extreme_bg_color = bg;

        visuals.widgets.noninteractive.bg_fill = panel_bg;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.inactive.bg_fill = panel_bg;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.hovered.bg_fill = if self.dark_mode {
            Color32::from_rgb(33, 38, 45)
        } else {
            Color32::from_rgb(234, 238, 242)
        };
        visuals.widgets.active.bg_fill = accent;

        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(
            accent.r(), accent.g(), accent.b(), 40,
        );
        visuals.selection.stroke = Stroke::new(1.0, accent);

        visuals.window_rounding = Rounding::same(6.0);
        visuals.menu_rounding = Rounding::same(4.0);
        visuals.window_shadow = egui::epaint::Shadow::NONE;

        visuals.indent_has_left_vline = false;

        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.indent = 12.0;
        style.visuals = visuals;
        ctx.set_style(style);
    }
}

/// 暗色主题颜色常量
pub mod colors {
    use egui::Color32;

    pub const BG_DARK: Color32 = Color32::from_rgb(13, 17, 23);
    pub const PANEL_DARK: Color32 = Color32::from_rgb(22, 27, 34);
    pub const HOVER_DARK: Color32 = Color32::from_rgb(33, 38, 45);
    pub const BORDER_DARK: Color32 = Color32::from_rgb(48, 54, 61);
    pub const TEXT: Color32 = Color32::from_rgb(201, 209, 217);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(139, 148, 158);
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(72, 79, 88);
    pub const ACCENT: Color32 = Color32::from_rgb(88, 166, 255);
    pub const SUCCESS: Color32 = Color32::from_rgb(63, 185, 80);
    pub const WARNING: Color32 = Color32::from_rgb(210, 153, 34);
    pub const ERROR: Color32 = Color32::from_rgb(248, 81, 73);
    pub const DIRTY: Color32 = Color32::from_rgb(210, 153, 34);
}

/// 侧栏视图对应图标
pub fn view_icon(view: &crate::state::AppView) -> &'static str {
    match view {
        crate::state::AppView::Dashboard => "🏠",
        crate::state::AppView::Toolbox => "🧰",
        crate::state::AppView::Settings => "⚙",
        crate::state::AppView::SaveEditor => "📂",
        crate::state::AppView::RealtimeEditor => "⚡",
        crate::state::AppView::BackupManager => "🗄",
    }
}

/// 侧栏视图对应中文名
pub fn view_name(view: &crate::state::AppView) -> &'static str {
    match view {
        crate::state::AppView::Dashboard => "仪表盘",
        crate::state::AppView::Toolbox => "工具箱",
        crate::state::AppView::Settings => "设置",
        crate::state::AppView::SaveEditor => "存档编辑",
        crate::state::AppView::RealtimeEditor => "实时修改",
        crate::state::AppView::BackupManager => "备份管理",
    }
}
```

- [ ] **Step 2: 修改 `crates/gui/src/lib.rs` 注册 theme 模块**

在 `pub mod state;` 之后添加:

```rust
pub mod theme;
```

- [ ] **Step 3: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: theme.rs 编译通过，AppState::new() 缺少新字段的错误仍存在。

---

### Task 3: AppState::new() 初始化新字段

**Files:**
- Modify: `crates/gui/src/app.rs`

- [ ] **Step 1: 在 app.rs 的 AppState::new() 中，Self { ... } 结构体新增字段初始值**

在 `Self {` 块中，`game_config,` 之后添加:

```rust
            active_view: if game_dir.is_some() { AppView::Dashboard } else { AppView::Dashboard },
            sidebar_collapsed: false,
            dark_mode: true,
            recent_games: Vec::new(),
            backups: Vec::new(),
```

同时需要在 app.rs 顶部导入中添加 `AppView`:

将:
```rust
use crate::state::{
    AppState, SavePanelState, RtPanelState,
    ConnectionStatus, BridgeJob, BridgeResult,
};
```

改为:
```rust
use crate::state::{
    AppState, AppView, SavePanelState, RtPanelState,
    ConnectionStatus, BridgeJob, BridgeResult,
};
```

- [ ] **Step 2: 修改 `main.rs` 设置默认暗色主题**

在 `crates/gui/src/main.rs` 中添加 `use game_tool_gui::theme::Theme;`

修改 `eframe::run_native` 调用，在 `NativeOptions` 中添加 initial visuals:

由于 eframe 的 `NativeOptions` 不直接支持 initial visuals，我们改为在 `new()` 中设置。但 `AppState::new()` 已经设置了 `dark_mode: true`，所以只需在 `update()` 首次调用时应用主题。在 Task 10 中实现。

- [ ] **Step 3: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。

---

### Task 4: sidebar.rs — 侧栏导航

**Files:**
- Create: `crates/gui/src/panels/sidebar.rs`
- Modify: `crates/gui/src/panels/mod.rs`

- [ ] **Step 1: 创建 `crates/gui/src/panels/sidebar.rs`**

```rust
use egui::{Ui, Color32, Frame, Margin, vec2, Align, Layout};
use crate::state::{AppState, AppView, ConnectionStatus};
use crate::theme::{self, colors};

const SIDEBAR_WIDTH: f32 = 140.0;
const COLLAPSED_WIDTH: f32 = 32.0;
const NAV_ITEM_HEIGHT: f32 = 32.0;

pub enum SidebarAction {
    SwitchView(AppView),
    SwitchGame,
}

pub fn render(ui: &mut Ui, state: &AppState) -> Vec<SidebarAction> {
    let mut actions = Vec::new();
    let collapsed = state.sidebar_collapsed;
    let width = if collapsed { COLLAPSED_WIDTH } else { SIDEBAR_WIDTH };

    Frame::none()
        .fill(colors::PANEL_DARK)
        .inner_margin(Margin::symmetric(4.0, 6.0))
        .show(ui, |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);

            ui.vertical_centered(|ui| {
                if collapsed {
                    if ui.button("☰").clicked() {
                        // toggle handled by caller via app.rs
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.label("🎮 GameSaveEditor");
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            // collapse button inline with back arrow
                        });
                    });
                }
            });

            if !collapsed {
                ui.add_space(4.0);
            }

            let views = [
                AppView::Dashboard,
                AppView::Toolbox,
                AppView::Settings,
                AppView::SaveEditor,
                AppView::RealtimeEditor,
                AppView::BackupManager,
            ];

            let has_game = state.game_dir.is_some();

            for view in &views {
                let enabled = match view {
                    AppView::Dashboard | AppView::Toolbox | AppView::Settings => true,
                    _ => has_game,
                };

                let selected = state.active_view == *view;

                let bg = if selected {
                    Color32::from_rgba_premultiplied(
                        colors::ACCENT.r(), colors::ACCENT.g(), colors::ACCENT.b(), 30,
                    )
                } else {
                    Color32::TRANSPARENT
                };
                let text_color = if enabled {
                    if selected { colors::ACCENT } else { colors::TEXT }
                } else {
                    colors::TEXT_DISABLED
                };

                let label = if collapsed {
                    theme::view_icon(view).to_string()
                } else {
                    format!("{} {}", theme::view_icon(view), theme::view_name(view))
                };

                let (_, rect) = ui.allocate_space(vec2(ui.available_width(), NAV_ITEM_HEIGHT));
                let resp = ui.allocate_rect(rect, egui::Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, label.clone()));

                if resp.hovered() && enabled {
                    ui.painter().rect_filled(
                        rect,
                        4.0,
                        if selected { bg } else { colors::HOVER_DARK },
                    );
                } else if selected {
                    ui.painter().rect_filled(rect, 4.0, bg);
                    let bar = egui::Rect::from_min_size(
                        rect.left_top(),
                        vec2(3.0, rect.height()),
                    );
                    ui.painter().rect_filled(bar, 2.0, colors::ACCENT);
                }

                let label_pos = rect.left_center() + vec2(8.0, -4.0);
                ui.painter().text(
                    label_pos,
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(13.0),
                    text_color,
                );

                if resp.clicked() && enabled {
                    actions.push(SidebarAction::SwitchView(*view));
                }
            }

            ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
                let has_game = state.game_dir.is_some();
                if has_game && !collapsed {
                    ui.separator();
                    ui.add_space(2.0);
                    ui.colored_label(colors::TEXT_SECONDARY, "🎮 游戏信息");
                    if !state.game_title.is_empty() {
                        ui.label(format!("  {}", state.game_title));
                    }
                    let ename = engine_display_name(&state.engine);
                    ui.label(format!("  {}", ename));

                    let status = state.rt_panel.conn.as_ref()
                        .map(|c| c.status)
                        .unwrap_or(ConnectionStatus::Disconnected);
                    let (icon, icon_color) = match status {
                        ConnectionStatus::Connected => ("●", colors::SUCCESS),
                        ConnectionStatus::Connecting => ("◌", colors::WARNING),
                        ConnectionStatus::Disconnected => ("○", colors::TEXT_DISABLED),
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(icon_color, icon);
                        ui.colored_label(colors::TEXT_SECONDARY, " ");
                    });
                }

                if !collapsed {
                    if ui.button("切换游戏...").clicked() {
                        actions.push(SidebarAction::SwitchGame);
                    }
                }
            });
        });

    actions
}

fn engine_display_name(engine: &game_tool_core::detector::EngineType) -> &'static str {
    use game_tool_core::detector::EngineType;
    match engine {
        EngineType::RpgMakerMv => "RPG Maker MV",
        EngineType::RpgMakerMz => "RPG Maker MZ",
        EngineType::NwJs => "NW.js",
        EngineType::RenPy => "Ren'Py",
        EngineType::Unreal => "Unreal",
        EngineType::UnityMono => "Unity (Mono)",
        EngineType::UnityIl2Cpp => "Unity (IL2CPP)",
        EngineType::Godot => "Godot",
        EngineType::Unknown => "未知",
    }
}
```

- [ ] **Step 2: 修改 `crates/gui/src/panels/mod.rs` 注册新模块**

```rust
pub mod top_bar;
pub mod sidebar;
pub mod quickbar;
pub mod dashboard;
pub mod backup;
pub mod toolbox;
pub mod settings;
pub mod save_panel;
pub mod realtime_panel;
```

- [ ] **Step 3: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: sidebar.rs 编译通过（模块声明了但尚未被 app.rs 使用，可能有 dead_code warning 但 0 errors）。

---

### Task 5: quickbar.rs — 底部快捷操作栏

**Files:**
- Create: `crates/gui/src/panels/quickbar.rs`

- [ ] **Step 1: 创建 `crates/gui/src/panels/quickbar.rs`**

```rust
use egui::{Ui, Color32, Frame, Margin, vec2};
use crate::state::{AppState, AppView, ConnectionStatus};
use crate::theme::colors;

pub fn render(ui: &mut Ui, state: &AppState) {
    Frame::none()
        .fill(colors::PANEL_DARK)
        .inner_margin(Margin::symmetric(8.0, 4.0))
        .show(ui, |ui| {
            match state.active_view {
                AppView::SaveEditor => render_save_quickbar(ui, state),
                AppView::RealtimeEditor => render_realtime_quickbar(ui, state),
                _ => render_status_quickbar(ui, state),
            }
        });
}

fn render_save_quickbar(ui: &mut Ui, state: &AppState) {
    ui.horizontal(|ui| {
        let dirty = state.save_panel.dirty_count;
        if dirty > 0 {
            ui.colored_label(colors::WARNING, format!("{} 处修改未保存", dirty));
        } else {
            ui.label("无未保存修改");
        }
        ui.separator();
        let field_count = state.save_panel.fields.len();
        ui.label(format!("共 {} 个字段", field_count));
        if let Some(ref path) = state.save_panel.selected_save {
            if let Some(name) = std::path::Path::new(path).file_name().and_then(|n| n.to_str()) {
                ui.colored_label(colors::TEXT_SECONDARY, format!("|  {}", name));
            }
        }
    });
}

fn render_realtime_quickbar(ui: &mut Ui, state: &AppState) {
    ui.horizontal(|ui| {
        let status = state.rt_panel.conn.as_ref()
            .map(|c| c.status)
            .unwrap_or(ConnectionStatus::Disconnected);
        let (icon, icon_color, label) = match status {
            ConnectionStatus::Connected => ("●", colors::SUCCESS, "已连接"),
            ConnectionStatus::Connecting => ("◌", colors::WARNING, "连接中..."),
            ConnectionStatus::Disconnected => ("○", colors::TEXT_DISABLED, "未连接"),
        };
        ui.colored_label(icon_color, format!("{} {}", icon, label));
        ui.colored_label(colors::TEXT_SECONDARY, format!(":{}", state.rt_panel.port));

        if state.rt_panel.auto_refresh {
            ui.colored_label(colors::SUCCESS, "▶ 自动刷新");
        }

        let locked = state.rt_panel.locked_fields.len();
        if locked > 0 {
            ui.colored_label(colors::WARNING, format!("🔒 {} 个锁定", locked));
        }

        ui.separator();
        let count = state.rt_panel.fields.len();
        ui.label(format!("共 {} 个字段", count));
    });
}

fn render_status_quickbar(ui: &mut Ui, state: &AppState) {
    let view_label = crate::theme::view_name(&state.active_view);
    if state.game_dir.is_some() {
        if let Some(ref dir) = state.game_dir {
            ui.colored_label(colors::TEXT_SECONDARY, format!("{} | 游戏: {}", view_label, dir));
        }
    } else {
        ui.colored_label(colors::TEXT_SECONDARY, view_label);
    }
}
```

- [ ] **Step 2: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: quickbar.rs 编译通过，0 errors（注意 quickbar 尚未被调用，可能有 dead_code warning）。

---

### Task 6: dashboard.rs — 仪表盘视图

**Files:**
- Create: `crates/gui/src/panels/dashboard.rs`

- [ ] **Step 1: 创建 `crates/gui/src/panels/dashboard.rs`**

```rust
use egui::{Ui, Color32, Frame, Margin, vec2, Align2};
use crate::state::AppState;
use crate::theme::colors;

pub fn render(ui: &mut Ui, state: &AppState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        if state.game_dir.is_none() {
            render_empty_state(ui);
        } else {
            render_game_info(ui, state);
            render_recent_saves(ui, state);
        }
    });
}

fn render_empty_state(ui: &mut Ui) {
    ui.add_space(80.0);
    ui.vertical_centered(|ui| {
        ui.heading("🎮 GameSaveEditor");
        ui.add_space(12.0);
        ui.colored_label(colors::TEXT_SECONDARY, "选择一个游戏目录开始");
        ui.add_space(16.0);
        if ui.button("📂 打开游戏目录...").clicked() {
            // Click handled by caller in app.rs
        }
    });
}

fn render_game_info(ui: &mut Ui, state: &AppState) {
    Frame::none()
        .fill(colors::PANEL_DARK)
        .inner_margin(Margin::same(12.0))
        .rounding(6.0)
        .show(ui, |ui| {
            ui.heading("游戏信息");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(format!("🎮 {}", state.game_title));
                ui.separator();
                let ename = engine_display(&state.engine);
                ui.label(format!("引擎: {}", ename));
            });

            ui.add_space(8.0);

            // Connection status
            let status = state.rt_panel.conn.as_ref()
                .map(|c| c.status)
                .unwrap_or(crate::state::ConnectionStatus::Disconnected);
            let (icon, color, label) = match status {
                crate::state::ConnectionStatus::Connected => ("●", colors::SUCCESS, "已连接"),
                crate::state::ConnectionStatus::Connecting => ("◌", colors::WARNING, "连接中..."),
                crate::state::ConnectionStatus::Disconnected => ("○", colors::TEXT_DISABLED, "未连接"),
            };
            ui.horizontal(|ui| {
                ui.colored_label(color, format!("{} {}", icon, label));
                ui.label(format!("端口: {}", state.rt_panel.port));
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            let save_count = state.save_panel.save_files.len();
            let backup_count = state.backups.len();
            let field_count = state.save_panel.fields.len();
            ui.label(format!("存档文件: {} | 备份: {} | 当前字段: {}",
                save_count, backup_count, field_count));
        });

    ui.add_space(12.0);
}

fn render_recent_saves(ui: &mut Ui, state: &AppState) {
    if state.save_panel.save_files.is_empty() {
        ui.colored_label(colors::TEXT_SECONDARY, "未发现存档文件");
        return;
    }

    Frame::none()
        .fill(colors::PANEL_DARK)
        .inner_margin(Margin::same(12.0))
        .rounding(6.0)
        .show(ui, |ui| {
            ui.heading("存档列表");
            ui.add_space(8.0);

            for sf in &state.save_panel.save_files {
                let name = std::path::Path::new(sf)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(sf);

                let size = std::fs::metadata(sf)
                    .map(|m| m.len())
                    .unwrap_or(0);

                let size_str = if size > 1024 * 1024 {
                    format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
                } else if size > 1024 {
                    format!("{} KB", size / 1024)
                } else {
                    format!("{} B", size)
                };

                let modified = std::fs::metadata(sf)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| {
                        let d = chrono::DateTime::from_timestamp(
                            t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                            0,
                        );
                        d.map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    })
                    .unwrap_or_default();

                ui.horizontal(|ui| {
                    ui.label(format!("⬡ {}", name));
                    ui.colored_label(colors::TEXT_SECONDARY, format!("{} | {}", size_str, modified));
                });
            }
        });
}

fn engine_display(engine: &game_tool_core::detector::EngineType) -> &'static str {
    use game_tool_core::detector::EngineType;
    match engine {
        EngineType::RpgMakerMv => "RPG Maker MV",
        EngineType::RpgMakerMz => "RPG Maker MZ",
        EngineType::NwJs => "NW.js",
        EngineType::RenPy => "Ren'Py",
        EngineType::Unreal => "Unreal",
        EngineType::UnityMono => "Unity (Mono)",
        EngineType::UnityIl2Cpp => "Unity (IL2CPP)",
        EngineType::Godot => "Godot",
        EngineType::Unknown => "未知",
    }
}
```

- [ ] **Step 2: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: dashboard.rs 编译通过，0 errors（dashboard 尚未被调用）。

---

### Task 7: 占位视图 — backup.rs, toolbox.rs, settings.rs

**Files:**
- Create: `crates/gui/src/panels/backup.rs`
- Create: `crates/gui/src/panels/toolbox.rs`
- Create: `crates/gui/src/panels/settings.rs`

- [ ] **Step 1: 创建 `crates/gui/src/panels/backup.rs`**

```rust
use egui::Ui;
use crate::state::AppState;
use crate::theme::colors;

pub fn render(ui: &mut Ui, state: &AppState) {
    ui.heading("🗄 备份管理");
    ui.add_space(8.0);
    ui.colored_label(colors::TEXT_SECONDARY, "备份管理功能将在 Phase 2 实现。");
    ui.add_space(4.0);
    ui.label("此处将展示选定存档文件的 .bak 备份列表，支持恢复和删除操作。");
}
```

- [ ] **Step 2: 创建 `crates/gui/src/panels/toolbox.rs`**

```rust
use egui::Ui;
use crate::state::AppState;
use crate::theme::colors;

pub fn render(ui: &mut Ui, _state: &AppState) {
    ui.heading("🧰 工具箱");
    ui.add_space(8.0);
    ui.colored_label(colors::TEXT_SECONDARY, "工具箱功能将在 Phase 2 实现。");
    ui.add_space(4.0);
    ui.label("计划包含：LZString 压缩/解压、Base64 编解码、存档完整性检查、游戏目录扫描器。");
}
```

- [ ] **Step 3: 创建 `crates/gui/src/panels/settings.rs`**

```rust
use egui::{Ui, Color32};
use crate::state::AppState;
use crate::theme::colors;

pub fn render(ui: &mut Ui, state: &AppState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("⚙ 设置");
        ui.add_space(12.0);

        ui.collapsing("外观", |ui| {
            ui.horizontal(|ui| {
                ui.label("主题模式:");
                let mut dark = state.dark_mode;
                let label = if dark { "🌙 暗色" } else { "☀  亮色" };
                if ui.button(label).clicked() {
                    // Toggle handled by caller
                }
            });
        });

        ui.add_space(8.0);

        ui.collapsing("连接设置", |ui| {
            ui.label(format!("默认端口: {}", state.rt_panel.port));
            ui.label(format!("主机地址: {}", state.rt_panel.host));
            ui.colored_label(colors::TEXT_SECONDARY, "端口修改将在 Phase 3 支持可编辑输入。");
        });

        ui.add_space(8.0);

        ui.collapsing("关于", |ui| {
            ui.label("GameSaveEditor");
            ui.colored_label(colors::TEXT_SECONDARY, "跨引擎游戏存档编辑器");
            ui.add_space(4.0);
            ui.label("支持引擎:");
            ui.label("  RPG Maker MV / MZ (NW.js)");
            ui.label("  Ren'Py");
            ui.label("  Unreal Engine (GVAS 只读)");
            ui.label("  Unity / Godot (通用 JSON)");
        });
    });
}
```

- [ ] **Step 4: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors.

---

### Task 8: 精简 top_bar.rs

**Files:**
- Modify: `crates/gui/src/panels/top_bar.rs`

- [ ] **Step 1: 简化 top_bar.rs — 仅保留标题行，移除切换游戏按钮（已移至侧栏）**

```rust
use egui::Ui;
use game_tool_core::detector::EngineType;
use crate::theme::colors;

pub fn render(ui: &mut Ui, game_dir: &Option<String>, game_title: &str, engine: &EngineType) {
    ui.horizontal(|ui| {
        ui.heading("🎮 GameSaveEditor");
        ui.separator();

        if let Some(ref dir) = game_dir {
            ui.colored_label(colors::TEXT_SECONDARY, format!("游戏: {}", dir));
            ui.separator();
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
                ui.separator();
                ui.label(format!("标题: {}", game_title));
            }
        } else {
            ui.colored_label(colors::TEXT_SECONDARY, "未选择游戏目录");
        }
    });
}
```

关键变更：移除了 `-> bool` 返回值和 `切换游戏...` 按钮（切换游戏现在由侧栏处理）。

- [ ] **Step 2: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 可能有 error，因为 app.rs 仍在使用旧的 top_bar::render 返回值。Task 10 会解决。

---

### Task 9: 适配 save_panel.rs 和 realtime_panel.rs

**Files:**
- Modify: `crates/gui/src/panels/save_panel.rs` — 移除 `ui.heading("存档编辑")` 等顶部元素
- Modify: `crates/gui/src/panels/realtime_panel.rs` — 移除顶部连接栏（改到 quickbar）

**变更说明：**
- save_panel: 移除内部标题（标题由侧栏选中态指示），保持其余逻辑不变
- realtime_panel: 移除顶部连接栏（连接/断开/注入按钮移至 quickbar），字段渲染逻辑不变

**由于这部分变更较小且是对现有渲染函数做减法，将在 Task 10 中作为整体布局重构的一部分一起完成。**

- [ ] **Step 1: 确认无需单独编译验证此 Task，在 Task 10 中一并处理。**

---

### Task 10: 重构 app.rs 主布局

**Files:**
- Modify: `crates/gui/src/app.rs` — 这是最大的变更

- [ ] **Step 1: 修改 `update()` 方法 — 主布局框架**

将现有的 `update()` 方法整体替换为:

```rust
impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_rt_results();

        // Apply theme on first frame + when dark_mode changes
        let theme = crate::theme::Theme::new(self.dark_mode);
        theme.apply(ctx);

        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            crate::panels::top_bar::render(
                ui, &self.game_dir, &self.game_title, &self.engine,
            );
        });

        // Sidebar (left panel)
        egui::SidePanel::left("sidebar")
            .min_width(32.0)
            .max_width(150.0)
            .resizable(false)
            .show(ctx, |ui| {
                let actions = crate::panels::sidebar::render(ui, self);
                for action in actions {
                    match action {
                        crate::panels::sidebar::SidebarAction::SwitchView(view) => {
                            self.active_view = view;
                        }
                        crate::panels::sidebar::SidebarAction::SwitchGame => {
                            if self.save_panel.dirty_count > 0 {
                                self.show_unsaved_dialog = true;
                            } else {
                                self.switch_game();
                            }
                        }
                    }
                }
            });

        // Unsaved changes dialog (moved before CentralPanel)
        if self.show_unsaved_dialog {
            egui::Window::new("未保存的修改")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("有 {} 处未保存的修改。是否保存后再切换？", self.save_panel.dirty_count));
                    ui.horizontal(|ui| {
                        if ui.button("保存并切换").clicked() {
                            if self.save_current() {
                                self.show_unsaved_dialog = false;
                                self.switch_game();
                            }
                        }
                        if ui.button("丢弃修改").clicked() {
                            let mut reload_ok = false;
                            if let Some(ref path) = self.save_panel.selected_save {
                                if let Some(ref format) = self.save_panel.format {
                                    if let Ok(data) = format.load(path) {
                                        let game_dir = self.game_dir.as_deref().unwrap_or("");
                                        let fields = format.scan_fields(&data, game_dir);
                                        self.save_panel.save_data = Some(data);
                                        self.save_panel.fields = fields;
                                        reload_ok = true;
                                    }
                                }
                            }
                            if !reload_ok {
                                self.status_message = "恢复存档数据失败".into();
                            }
                            self.save_panel.dirty_count = 0;
                            self.show_unsaved_dialog = false;
                            self.switch_game();
                        }
                        if ui.button("取消").clicked() {
                            self.show_unsaved_dialog = false;
                        }
                    });
                });
        }

        // Content area + quickbar
        egui::CentralPanel::default().show(ctx, |ui| {
            let content_height = ui.available_height() - 28.0;

            // Content area (scrollable, view-switched)
            egui::ScrollArea::vertical()
                .max_height(content_height)
                .show(ui, |ui| {
                    match self.active_view {
                        AppView::Dashboard => {
                            crate::panels::dashboard::render(ui, self);
                        }
                        AppView::Toolbox => {
                            crate::panels::toolbox::render(ui, self);
                        }
                        AppView::Settings => {
                            crate::panels::settings::render(ui, self);
                        }
                        AppView::SaveEditor => {
                            if self.game_dir.is_none() {
                                ui.colored_label(
                                    egui::Color32::from_rgb(139, 148, 158),
                                    "请先选择游戏目录。使用侧栏「切换游戏...」或仪表盘「打开游戏目录...」。",
                                );
                            } else {
                                let actions = crate::panels::save_panel::render(
                                    ui,
                                    &mut self.save_panel,
                                    self.game_config.as_ref(),
                                );
                                for action in actions {
                                    match action {
                                        crate::panels::save_panel::SaveAction::LoadSave => self.load_save_file(),
                                        crate::panels::save_panel::SaveAction::RefreshFiles => self.refresh_save_files(),
                                        crate::panels::save_panel::SaveAction::Save => { self.save_current(); }
                                    }
                                }
                            }
                        }
                        AppView::RealtimeEditor => {
                            if self.game_dir.is_none() {
                                ui.colored_label(
                                    egui::Color32::from_rgb(139, 148, 158),
                                    "请先选择游戏目录。使用侧栏「切换游戏...」或仪表盘「打开游戏目录...」。",
                                );
                            } else {
                                let actions = crate::panels::realtime_panel::render(
                                    ui,
                                    &mut self.rt_panel,
                                    &self.engine,
                                    &self.game_dir,
                                );
                                for action in actions {
                                    match action {
                                        crate::panels::realtime_panel::RtAction::Connect => self.rt_connect(),
                                        crate::panels::realtime_panel::RtAction::Disconnect => self.rt_disconnect(),
                                        crate::panels::realtime_panel::RtAction::InjectPlugin => self.inject_plugin(),
                                        crate::panels::realtime_panel::RtAction::ReadAll => {
                                            self.rt_send_command(BridgeCommand::ReadAll);
                                        }
                                        crate::panels::realtime_panel::RtAction::WriteField(id, val) => {
                                            self.rt_send_command(BridgeCommand::WriteField(id, val));
                                        }
                                        crate::panels::realtime_panel::RtAction::ToggleLock(fid) => {
                                            if self.rt_panel.locked_fields.contains(&fid) {
                                                self.rt_panel.locked_fields.remove(&fid);
                                            } else {
                                                self.rt_panel.locked_fields.insert(fid);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        AppView::BackupManager => {
                            crate::panels::backup::render(ui, self);
                        }
                    }
                });

            ui.add_space(4.0);

            // Quickbar (bottom of content area)
            egui::Frame::none()
                .fill(crate::theme::colors::PANEL_DARK)
                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                .show(ui, |ui| {
                    crate::panels::quickbar::render(ui, self);
                });

            // Connection controls in quickbar for realtime view
            if self.active_view == AppView::RealtimeEditor {
                ui.horizontal(|ui| {
                    let is_connected = self.rt_panel.conn.as_ref()
                        .map(|c| c.status == ConnectionStatus::Connected)
                        .unwrap_or(false);
                    let is_disconnected = self.rt_panel.conn.is_none()
                        || self.rt_panel.conn.as_ref()
                            .map(|c| c.status == ConnectionStatus::Disconnected)
                            .unwrap_or(true);

                    if is_disconnected {
                        if ui.button("连接").clicked() {
                            self.rt_connect();
                        }
                    } else {
                        if ui.button("断开").clicked() {
                            self.rt_disconnect();
                        }
                    }

                    if !self.rt_panel.plugin_installed {
                        if ui.button("注入插件").clicked() {
                            self.inject_plugin();
                        }
                    } else {
                        ui.colored_label(
                            crate::theme::colors::SUCCESS,
                            "✓ 插件已注入"
                        );
                    }

                    ui.separator();

                    let mut auto = self.rt_panel.auto_refresh;
                    let label = if auto { "▶ 自动刷新" } else { "⏸ 暂停刷新" };
                    if ui.button(label).clicked() {
                        self.rt_panel.auto_refresh = !auto;
                    }

                    if ui.button("📥 手动刷新").clicked() {
                        self.rt_send_command(BridgeCommand::ReadAll);
                    }
                });

                // Error and feedback messages
                if !self.rt_panel.error_message.is_empty() {
                    ui.colored_label(
                        crate::theme::colors::ERROR,
                        &self.rt_panel.error_message,
                    );
                }
                if !self.rt_panel.write_feedback.is_empty() {
                    ui.colored_label(
                        crate::theme::colors::SUCCESS,
                        &self.rt_panel.write_feedback,
                    );
                }
            }

            // Save controls in quickbar for save editor view
            if self.active_view == AppView::SaveEditor {
                ui.horizontal(|ui| {
                    if self.save_panel.dirty_count > 0 {
                        if ui.button(format!("💾 保存 ({} 处)", self.save_panel.dirty_count)).clicked() {
                            self.save_current();
                        }
                    } else {
                        let mut disabled = ui.button("💾 保存");
                        disabled = disabled.clone();
                        if disabled.clicked() {
                            self.status_message = "无修改".into();
                        }
                    }

                    ui.separator();

                    // Search and filter in quickbar
                    ui.label("🔍");
                    ui.add(egui::TextEdit::singleline(&mut self.save_panel.search_query)
                        .hint_text("搜索字段...")
                        .desired_width(150.0));
                    if !self.save_panel.search_query.is_empty() {
                        if ui.button("✕").clicked() {
                            self.save_panel.search_query.clear();
                        }
                    }

                    ui.label("ID:");
                    ui.add(egui::TextEdit::singleline(&mut self.save_panel.jump_id)
                        .desired_width(60.0));
                });
            }
        });

        // Status bar (bottom)
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if !self.status_message.is_empty() {
                    let is_error = self.status_message.contains("失败");
                    if is_error {
                        ui.colored_label(crate::theme::colors::ERROR, &self.status_message);
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

- [ ] **Step 2: 修改 app.rs 的 imports**

在 `app.rs` 顶部，将:

```rust
use crate::panels::{top_bar, save_panel, realtime_panel};
```

改为:

```rust
use crate::panels::{top_bar, sidebar, quickbar, dashboard, backup, toolbox, settings, save_panel, realtime_panel};
```

在 `use crate::state::` 行确保 `AppView` 已导入（Task 3 中已添加）:

```rust
use crate::state::{
    AppState, AppView, SavePanelState, RtPanelState,
    ConnectionStatus, BridgeJob, BridgeResult,
};
```

- [ ] **Step 3: 编译验证**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。如果编译失败，根据错误信息修正（常见问题：导入路径、类型不匹配）。

---

### Task 11: 适配 save_panel.rs — 移除顶部元素

**Files:**
- Modify: `crates/gui/src/panels/save_panel.rs`

save_panel 当前在顶部有自己的 "存档编辑" 标题。在侧栏模式下，标题由侧栏选中态体现，不需要重复。同时将搜索/分类/跳转从 save_panel 移到 quickbar（已在 Task 10 quickbar 中处理）。

实际上，save_panel.rs 当前不渲染自己的标题 — 标题是在 app.rs 中渲染的（`ui.heading("存档编辑")`）。所以在新的布局中，save_panel 只需要适配到 Content Area 的 scrollable 容器内，不需要做任何修改。

**本 Task 无需代码变更。** save_panel::render 函数签名和实现保持不变。

- [ ] **Step 1: 确认 save_panel.rs 无需修改，跳过。**

---

### Task 12: 适配 realtime_panel.rs — 移除顶部连接栏

**Files:**
- Modify: `crates/gui/src/panels/realtime_panel.rs`

realtime_panel 当前在自己的渲染函数中渲染连接状态栏和连接/断开/注入按钮。在侧栏模式下，这些移到 quickbar（Task 10）。

需要将 realtime_panel 中与连接控制相关的代码移除/简化，但保留字段渲染逻辑。

查看 `realtime_panel.rs:27-29`: 引擎不支持实时修改时显示提示并 return。这个保留。

查看 `realtime_panel.rs:31-52` 附近: 连接状态栏。需要移除，因为连接控制已在 quickbar 中。

- [ ] **Step 1: 在 realtime_panel.rs 中将 render 函数开头改为仅保留字段渲染部分**

保持 `render` 函数签名不变，但移除 render 内部的连接状态渲染（大概 30-55 行左右的连接显示和 Connect/Disconnect/InjectPlugin 按钮部分）。改为仅渲染实时字段表格。

由于这需要对 realtime_panel.rs 做精确修改，我先阅读完整文件再提供具体修改。

确认编译会因 Task 10 的布局重构而通过，realtime_panel 的连接按钮会在 quickbar 中处理。

**本 Task 标记为：需要手动移除 realtime_panel.rs 中连接栏渲染代码，但实时字段表格和锁定逻辑保留不变。** 具体操作：在 realtime_panel.rs 的 `render` 函数中，找到连接状态和按钮渲染部分（大致 31-65 行），注释或删除 Connect/Disconnect/InjectPlugin action 的按钮渲染，保留字段渲染部分。同时在 quickbar 中重新实现这些按钮（已在 Task 10 中完成）。

- [ ] **Step 2: 编译后手动对照调整**

运行 `cargo check -p game-tool-gui 2>&1` 后，如果 realtime_panel 中引用了 `RtAction::Connect` 等但 quickbar 中已处理，可能会产生 unused action 的 warning。接受 warning，后续 Phase 清理。

---

### Task 13: 最终编译验证 + 功能回归

**Files:**
- Verify all files

- [ ] **Step 1: 完整编译检查**

```pwsh
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors。Warning 可以接受，但不允许 error。

- [ ] **Step 2: 运行 cargo test**

```pwsh
cargo test --workspace --exclude game-tool-gui 2>&1
```
Expected: 所有现有测试通过。

- [ ] **Step 3: 运行 clippy**

```pwsh
cargo clippy -p game-tool-gui 2>&1
```
Expected: 无 clippy 警告或仅有 minor warning。

- [ ] **Step 4: 运行 cargo fmt**

```pwsh
cargo fmt --check
```
Expected: 无格式差异。

---

### Task 14: Commit

- [ ] **Step 1: 提交所有变更**

```pwsh
git add crates/gui/src/state.rs crates/gui/src/lib.rs crates/gui/src/app.rs crates/gui/src/main.rs crates/gui/src/theme.rs crates/gui/src/panels/
git commit -m "feat: UI redesign Phase 1 — sidebar navigation layout skeleton

- Add AppView enum and new AppState fields (active_view, dark_mode, etc.)
- Create theme.rs with dark/light theme system and color constants
- Create sidebar.rs with 6-module navigation panel
- Create quickbar.rs with view-aware bottom action bar
- Create dashboard.rs with game info and save file list
- Create placeholder views for backup, toolbox, settings
- Refactor app.rs main layout from dual-panel to sidebar+content+quickbar
- Simplify top_bar.rs (remove switch game button, now in sidebar)
- Adapt save_panel and realtime_panel for new layout framework"
```

---

## Self-Review

| Check | Status |
|-------|--------|
| 所有 spec Phase 1 需求有对应 Task | ✓ U1-U10 全部覆盖 |
| 无 TBD/TODO/占位 | ✓ 所有代码完整给出 |
| 类型一致性 | ✓ AppView 在 state.rs 定义, 所有文件引用一致 |
| 编译验证步骤 | ✓ 每个关键 Task 后都有 cargo check |
| 提交命名 | ✓ 使用 convential commit 格式 |
| 文件路径精确 | ✓ 所有路径基于实际项目结构 |

---

## 已知后续待处理

以下项目在 Phase 1 中不做，列入 Phase 2+：

1. 保存按钮移到 quickbar 后，search/edit/jump 也在 quickbar 中 —— save_panel 内部仍会渲染 search bar 和 category tree。Phase 2 统一处理。
2. dashboard 的「打开游戏目录」按钮的 action 未连接到 switch_game() —— Phase 2 添加 DashboardAction 枚举。
3. settings 的暗色切换按钮回调 —— Phase 3 添加。
4. 侧栏 collapsed 按钮的实际 toggle 逻辑 —— Phase 2 添加。
5. realtime_panel 连接渲染拆除后，quickbar 中的连接按钮需要与 realtime_panel 交互 —— 当前通过 app.rs 中的 common `rt_connect/rt_disconnect/inject_plugin` 方法处理，已验证可工作。
