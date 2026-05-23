# GameSaveEditor UI 重设计 v3 — 精简选项卡

日期: 2026-05-23 | 版本: v2 | 状态: 已确认

---

## 一、概述

### 1.1 设计目标

废弃 v1/v2 的侧栏导航方案，改为**顶部选项卡 + 单页高密度布局**。核心原则：信息密度最大化、操作步数最小化、结构简单化。

### 1.2 与 v1/v2 的关系

**完全替代** `2026-05-23-game-tool-ui-redesign.md`。v3 仅参考 v1/v2 的功能定义（字段表、备份、工具箱功能规格），布局架构和导航方案独立设计。

### 1.3 约束

| 维度 | 约束 |
|------|------|
| 框架 | Rust + egui 0.31 + eframe 0.31 |
| 引擎支持 | RPG Maker MV/MZ/NW.js, Ren'Py, Unreal, Unity, Godot |
| 布局风格 | 固定布局（不可拖拽/自定义面板） |
| 目标窗口 | 1200x800（最小 1024x640） |
| 暗色模式 | 默认开启 |

---

## 二、布局架构

### 2.1 三区域结构

```
┌──────────────────────────────────────────────────────────┐
│ TOP BAR    GameSaveEditor    🎮游戏名 | 引擎 | ●状态       │  ← 36px
├──────────────────────────────────────────────────────────┤
│ [📂存档] [⚡实时] [🗄备份] [🧰工具] [⚙设置]  [切换游戏]    │  ← 32px 选项卡栏
├──────────────────────────────────────────────────────────┤
│                                                          │
│  当前选项卡内容（全宽，无侧栏）                              │
│                                                          │
├──────────────────────────────────────────────────────────┤
│ STATUS   连接状态 | 修改数 | 字段数 | 文件名                 │  ← 24px
└──────────────────────────────────────────────────────────┘
```

### 2.2 无游戏启动页

无游戏加载时，不显示选项卡栏，显示单页目录选择：

```
┌──────────────────────────────────────────────────────┐
│ TOP BAR   GameSaveEditor    [🧰工具] [⚙设置]         │
├──────────────────────────────────────────────────────┤
│                                                      │
│            🎮 选择一个游戏目录开始                       │
│         [📂 打开游戏目录...]   (居中大按钮)               │
│                                                      │
│   ── 最近游戏 ──────────────────────────────────      │
│   📁 path/to/game1    引擎名            [打开]        │
│   📁 path/to/game2    引擎名            [打开]        │
│                                                      │
├──────────────────────────────────────────────────────┤
│ STATUS   未加载游戏                                    │
└──────────────────────────────────────────────────────┘
```

### 2.3 AppView 枚举（废弃，改为 TabMode）

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum TabMode {
    SaveEditor,       // 默认选中（有游戏时）
    RealtimeEditor,   // 仅支持实时引擎时可用
    BackupManager,    // 需加载游戏
    Toolbox,          // 始终可用
    Settings,         // 始终可用
}
```

### 2.4 AppState 变更

```rust
pub struct AppState {
    // 保留字段
    pub game_dir: Option<String>,
    pub game_title: String,
    pub engine: EngineType,
    pub game_config: Option<GameConfig>,
    pub dark_mode: bool,
    pub rt_panel: RtPanelState,
    pub save_panel: SavePanelState,
    pub backup_paths: Vec<String>,
    pub recent_games: Vec<String>,

    // 新增字段
    pub active_tab: TabMode,           // 当前选项卡
    pub status_message: String,        // 状态栏文本
    pub show_unsaved_dialog: bool,     // 未保存确认对话框
    pub show_confirm_dialog: Option<ConfirmDialog>, // 通用确认对话框
}

pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub on_confirm: ConfirmAction,
}

pub enum ConfirmAction {
    DiscardAndSwitch,
    DeleteBackups(Vec<usize>),
    RestoreBackup(usize),
    ClearRecentGames,
}
```

### 2.5 代码架构变更

| 文件 | 变更 |
|------|------|
| `crates/gui/src/app.rs` | 重写主布局：启动页/选项卡切换；路由分发 |
| `crates/gui/src/state.rs` | TabMode 枚举替代 AppView；新增 ConfirmDialog |
| `crates/gui/src/panels/startup.rs` | 新建：启动页渲染 |
| `crates/gui/src/panels/save_editor.rs` | 新建：合并 save_panel + dashboard 概览 |
| `crates/gui/src/panels/realtime_editor.rs` | 重命名自 realtime_panel，适配新布局 |
| `crates/gui/src/panels/backup.rs` | 适配选项卡布局，新增差异对比 |
| `crates/gui/src/panels/toolbox.rs` | 适配选项卡布局 |
| `crates/gui/src/panels/settings.rs` | 适配选项卡布局 |
| `crates/gui/src/panels/top_bar.rs` | 精简内容 |
| `crates/gui/src/panels/tab_bar.rs` | 新建：选项卡栏渲染 |
| `crates/gui/src/panels/status_bar.rs` | 新建：状态栏渲染 |
| `crates/gui/src/panels/mod.rs` | 更新模块声明 |
| ~~`crates/gui/src/panels/sidebar.rs`~~ | 移除 |
| ~~`crates/gui/src/panels/quickbar.rs`~~ | 移除（合并到 status_bar） |
| ~~`crates/gui/src/panels/dashboard.rs`~~ | 移除（合并到 startup + save_editor） |
| ~~`crates/gui/src/panels/save_panel.rs`~~ | 移除（合并到 save_editor） |
| `crates/gui/src/theme.rs` | 保留，微调配色 |
| `crates/gui/src/widgets/` | 保留，field_table 新增差异列 |

---

## 三、选项卡设计

### 3.1 选项卡栏 (tab_bar.rs)

```rust
pub enum TabAction {
    SwitchTab(TabMode),
    SwitchGame,
}
pub fn render(ui: &mut Ui, state: &AppState) -> Vec<TabAction>;
```

- 5 个选项卡始终显示（有游戏时），高度 32px
- 选中态：底部 2px 强调色下划线 + 文字高亮
- 禁用态（无游戏时的存档/实时/备份）：灰色文字，不可点击
- 尾部 `[切换游戏...]` 按钮
- 禁用态 hover 提示原因：
  - 存档/备份："请先选择游戏目录"
  - 实时："当前引擎不支持实时修改"

### 3.2 默认选项卡选择逻辑

```
if has_game → SaveEditor
if !has_game → 不显示选项卡栏，显示启动页
切换游戏 → 保持当前选项卡（如果新游戏不支持实时且当前是实时→切到存档）
```

### 3.3 无游戏时的选项卡行为

- 选项卡栏不渲染
- 工具箱、设置可访问 → 通过启动页 TopBar 的固定按钮入口：
  - TopBar 右侧固定显示 `[🧰工具] [⚙设置]`
  - 点击后在内容区显示该页面（仍无选项卡栏）

---

## 四、启动页 (startup.rs)

```rust
pub enum StartupAction {
    OpenGameDir,
    OpenRecentGame(String),
    OpenToolbox,
    OpenSettings,
}
pub fn render(ui: &mut Ui, state: &AppState) -> Vec<StartupAction>;
```

### 4.1 布局

- 垂直居中：图标 + "选择一个游戏目录开始" 标题
- 大按钮 `[📂 打开游戏目录...]`（调用 rfd 文件夹选择对话框）
- 分隔线 `── 最近游戏 ──`
- 最近游戏列表：每行 `📁 路径 引擎名 [打开]`
- 最近游戏最多显示 5 条
- 空列表时不显示该区域

### 4.2 交互

- 打开游戏目录→加载游戏→自动切换到存档标签
- 点击最近游戏条目→直接加载→自动切换到存档标签
- TopBar 的工具/设置按钮 → 在无选项卡栏状态下显示对应页面

---

## 五、存档编辑标签 (save_editor.rs)

```rust
pub enum SaveEditorAction {
    LoadSave, RefreshFiles, Save, UndoDirty,
    Search(String), SelectCategory(String), JumpToId(String),
}
pub fn render(ui: &mut Ui, state: &mut AppState) -> Vec<SaveEditorAction>;
```

### 5.1 自上而下布局

```
┌ 游戏概览行 ────────────────────────────────────┐
│ 🎮 游戏名 | 引擎 | ●连接状态 | 金币单位: G    │  ← 紧凑一行
└────────────────────────────────────────────────┘

┌ 存档操作栏 ────────────────────────────────────┐
│ 📁 [file.rpgsave ▾] 128KB 2026-05-22 [🔄刷新] [💾保存] │
└────────────────────────────────────────────────┘

┌ 筛选栏 ───────────────────────────────────────┐
│ [全部分类▾] 🔍搜索:[...] ✕ 跳转ID: [___]  [⤺撤销修改] │
└────────────────────────────────────────────────┘

┌ 摘要卡片（加载存档后）─────────────────────────┐
│ 💰9999G | 👥4人 | 🎒23种 | ⏱12:34:56 | 💾第12次 │
└────────────────────────────────────────────────┘

┌ 字段表（全宽滚动）─────────────────────────────┐
│ 分类 | 名称 | 保存值 | 实时值 | 状态           │
│ ─────────────────────────────────────          │
│ ...行数据...                                  │
│                                                │
│ 共 N 项                                        │
└────────────────────────────────────────────────┘
```

### 5.2 字段表第 4 列（实时值）

- **仅在连接成功时显示**该列；未连接时该列隐藏
- 实时值 = 保存值 → 灰色显示
- 实时值 ≠ 保存值 → 黄色高亮，行首显示 `←` 差异指示器
- 实时值列为空（无连接）→ 不渲染该列

### 5.3 字段表状态列

- `*` 黄色：该字段已修改（dirty）
- `←` 黄色：实时值与保存值不同（差异指示）
- 可同时显示 `*` `←`

### 5.4 数据流

```
编辑保存值 → field.dirty = true → dirty_count++
点击[💾保存] → format.save() 写入磁盘 → dirty_count = 0
点击[⤺撤销修改] → 重置所有 dirty 字段为原始值 → dirty_count = 0
```

---

## 六、实时修改标签 (realtime_editor.rs)

```rust
pub enum RealtimeAction {
    Connect, Disconnect, InjectPlugin, Refresh,
    ToggleAutoRefresh, SetInterval(u32),
    WriteField(String, Value), ToggleLock(String),
    CopyToSave(String),
}
```

### 6.1 布局

```
┌ 连接管理栏 ───────────────────────────────────┐
│ 主机:[127.0.0.1] 端口:[19999] [●连接] [◎断开] [注入插件]   │
│ [▶自动刷新] [📥手动刷新] 间隔:[3秒▾]                      │
└────────────────────────────────────────────────┘

┌ 筛选栏 ───────────────────────────────────────┐
│ [全部分类▾] 🔍搜索:[...] ✕ 跳转ID: [___]                │
└────────────────────────────────────────────────┘

┌ 实时字段表（全宽滚动）─────────────────────────┐
│ 分类 | 名称 | 实时值 | 保存值 | 🔒 | 操作       │
│ ────────────────────────────────────────────    │
│ ...行数据...                                  │
│                                                │
│ 共 N 项                                        │
└────────────────────────────────────────────────┘
```

### 6.2 字段表列定义

| 列 | 内容 | 交互 |
|----|------|------|
| 分类 | field.category | 只读 |
| 名称 | field.display_name | 只读 |
| 实时值 | field.live_value | 可编辑（连接时），修改后即时发送 WriteField |
| 保存值 | field.save_value | 只读对照，差异高亮 |
| 🔒 | 锁定/解锁 toggle | 点击切换 field.locked |
| 操作 | [📤→存档] | 将实时值复制到对应的 save_value |

### 6.3 连接状态管理

```
Connected → 编辑控件可用，自动刷新有效
Disconnected → 所有编辑控件禁用，锁定列禁用
Connecting → 编辑控件禁用，显示 "连接中..." 加载指示
```

### 6.4 自动刷新

- 间隔选项：1秒/2秒/3秒/5秒（ComboBox）
- 实现：`refresh_timer` 改用实际时间（`Instant::now()`），非帧计数
- 锁定字段：刷新时跳过值更新（保留本地编辑值）
- 切换标签：自动刷新线程不中断

---

## 七、备份管理标签 (backup.rs)

```rust
pub enum BackupAction {
    CreateBackup, Restore(usize), Delete(usize),
    BatchDelete(Vec<usize>), Compare(usize, usize), RefreshList,
}
```

### 7.1 布局

```
┌ 操作栏 ───────────────────────────────────────┐
│ 📁 当前存档: [file.rpgsave ▾]   [💾创建备份]            │
└────────────────────────────────────────────────┘

┌ 备份列表 ─────────────────────────────────────┐
│ ☐ 文件名                大小    时间        操作    │
│ ────────────────────────────────────────────────    │
│ ☐ file1_xxx.bak       128KB  05-23 14:30 [恢复][删除] │
│ ...                                              │
│                                                  │
│ 已选 N 项  [📊对比选中] [🗑批量删除]                 │
└────────────────────────────────────────────────┘

┌ 差异对比（选中2项时展开）──────────────────────┐
│ A: file_xxx  vs  B: file_yyy                  │
│ 字段        备份A     备份B      变化            │
│ ────────────────────────────────────             │
│ 金币        9999      8500      +1499 ↗         │
│ ...                                              │
└────────────────────────────────────────────────┘
```

### 7.2 关键逻辑

- 当前存档与存档编辑标签**联动**：修改任何一端，另一端同步
- 备份文件自动发现：扫描存档文件同目录下的 `.bak`/`.orig` 文件
- 恢复/删除前弹出 `ConfirmDialog`
- 差异对比：选中 2 项后自动展开面板，复用存档解析引擎提取字段值对比
- 无存档时：显示 "请先在存档编辑中选择存档文件"

---

## 八、工具箱标签 (toolbox.rs)

### 8.1 布局

4 个可折叠子工具，默认展开 LZString，其余收起。

**LZString 压缩/解压：**

```
┌ LZString ───────────────────────── [−收起] ─┐
│ 模式: [●压缩 ○解压]                          │
│                                              │
│ 输入:                                        │
│ ┌──────────────────────────────────┐         │
│ │ (多行文本编辑框，最小6行高度)        │         │
│ └──────────────────────────────────┘         │
│                                              │
│ [执行]  结果:                                │
│ ┌──────────────────────────────────┐         │
│ │ (只读文本)                        │  [📋复制] │
│ └──────────────────────────────────┘         │
└──────────────────────────────────────────────┘
```

**Base64 编解码：** 交互模式同上。

**存档完整性检查：**

```
┌ 存档完整性检查 ──────────────────── [+展开] ─┐
│ [📁选择文件...] path/to/save                  │
│ [🔍检测]                                      │
│                                              │
│ ✅ JSON 格式合法                               │
│ ❌ Magic bytes 不匹配 — 文件可能已损坏           │
└──────────────────────────────────────────────┘
```

**游戏目录扫描：**

```
┌ 游戏目录扫描 ────────────────────── [+展开] ─┐
│ [📁选择目录...] path/to/game                  │
│ [🔍扫描]                                      │
│ 引擎: RPG Maker MV                            │
│ 开关:200 | 变量:500 | 物品:150               │
│ [📋导出JSON]                                  │
└──────────────────────────────────────────────┘
```

### 8.2 关键逻辑

- 所有文件选择器独立，不绑定当前游戏
- LZString/Base64 复用 `game_tool_core` 现有函数
- 复制按钮：成功后短暂显示 "已复制!" 反馈
- 完整性检查：文件独立选择，检查 JSON 合法性/格式匹配/magic/必要字段
- 目录扫描：独立目录选择，不影响主 game_dir

---

## 九、设置标签 (settings.rs)

### 9.1 布局

```
┌ 外观 ─────────────────────────────────────────┐
│ [🌙暗色] / [☀亮色]  当前: 暗色                  │
└───────────────────────────────────────────────┘

┌ 连接 ─────────────────────────────────────────┐
│ 主机: [127.0.0.1        ]                      │
│ 端口: [19999] [▼▲]      范围: 1024-65535       │
└───────────────────────────────────────────────┘

┌ 最近游戏 ─────────────────────────────────────┐
│ D:\Games\Adventure              [×移除]       │
│ ...                                          │
│ [🗑清除全部记录]                              │
└───────────────────────────────────────────────┘

┌ 配置 ─────────────────────────────────────────┐
│ 配置目录: C:\Users\...\GameSaveEditor\         │
│ [📂打开目录]                                   │
└───────────────────────────────────────────────┘

┌ 关于 ─────────────────────────────────────────┐
│ GameSaveEditor v0.1.0                         │
│ 跨引擎游戏存档编辑器                           │
│ 支持: RPG Maker MV/MZ/NW.js, Ren'Py,          │
│       Unreal, Unity, Godot                    │
└───────────────────────────────────────────────┘
```

### 9.2 关键逻辑

- 主题切换即时生效
- 端口修改：已连接时弹出提示 "请断开后重新连接"
- 最近游戏：读写 `config.toml`，单独移除 + 全部清除（需确认）
- 配置路径只读，点击按钮用系统文件管理器打开
- 无游戏时可通过启动页 TopBar 按钮访问设置

---

## 十、状态栏 (status_bar.rs)

状态栏统一显示在窗口底部，内容根据当前选项卡动态变化：

| 选项卡 | 状态栏内容 |
|--------|-----------|
| 存档编辑 | `＊N处修改未保存 \| 共M字段 \| filename` |
| 实时修改 | `●已连接 :PORT \| 🔒N个锁定 \| 共M字段 \| 差异K项` |
| 备份管理 | `共N个备份 \| 选中M项 \| 当前: filename` |
| 工具箱 | `工具箱 — 独立工具，无需加载游戏` |
| 设置 | `设置` |
| 启动页 | `未加载游戏` |

---

## 十一、跨标签逻辑审查

### 11.1 状态同步

| 场景 | 行为 |
|------|------|
| 存档标签修改→切到实时标签 | dirty 保留，不丢失 |
| 存档标签编辑→不保存→切换游戏 | 弹出 ConfirmDialog |
| 存档标签选文件A→切到备份标签 | 备份标签绑定文件A，刷新列表 |
| 实时标签连接中→切到其他标签 | 连接保持，bridge 线程不中断 |
| 设置改端口→切到实时标签 | 未连接→生效；已连接→提示重连 |

### 11.2 边界情况

| 场景 | 行为 |
|------|------|
| 无游戏→存档/实时/备份标签 | 灰色 disable，hover 提示原因 |
| 游戏不支持实时→实时标签 | 灰色 disable，"当前引擎不支持实时修改" |
| Unreal GVAS（只读）→编辑控件 | 全部禁用，顶部提示"只读模式" |
| 字段表 0 条 | "未找到匹配字段" |
| 备份列表 0 条 | "暂无备份" |
| 最近游戏 0 条 | 不显示该区域 |
| 切换标签时 ScrollArea | 使用 id_salt 保留滚动位置 |

### 11.3 确认对话框实现

```rust
// 在 app.rs 主循环末尾渲染，确保在所有内容之上
if let Some(ref dialog) = state.show_confirm_dialog {
    egui::Window::new(&dialog.title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(&dialog.message);
            ui.horizontal(|ui| {
                if ui.button("确定").clicked() { /* execute action */ }
                if ui.button("取消").clicked() { state.show_confirm_dialog = None; }
            });
        });
}
```

### 11.4 已修复的 v1/v2 遗留问题

| 问题 | 解决 |
|------|------|
| 侧栏折叠死变量 | 废弃 sidebar，无折叠逻辑 |
| 重复搜索栏 | 每个标签独立一个搜索栏 |
| 自动刷新用帧计数 | 改用 `Instant::now()` 计算实际时间间隔 |
| 硬编码 content_height - 100.0 | 使用 `ui.available_size()` 自动计算 |
| `engine_display_name()` 3处重复 | 提取到 `theme.rs` 作为公共函数 |
| Unicode 逃逸序列 | 新增中文文本改用字符串字面量（egui 支持 UTF-8） |

---

## 十二、视觉设计

沿用 v1 的 GitHub 暗色主题配色。调整项：

- 选项卡栏背景：`#161b22`（与面板一致）
- 选项卡选中下划线：`#58a6ff` 2px
- 状态栏背景：`#0d1117`（与主背景一致）
- 状态栏文字：`#8b949e` 12px
- 差异高亮：`#d29922`
- 所有卡片圆角统一 4px

---

## 十三、实施顺序

### Phase 1: 骨架迁移

1. 重写 `state.rs`：TabMode 枚举，ConfirmDialog
2. 新建 `tab_bar.rs`：选项卡栏
3. 新建 `status_bar.rs`：状态栏
4. 新建 `startup.rs`：启动页
5. 重写 `app.rs`：新主布局
6. 移除 `sidebar.rs`, `quickbar.rs`, `dashboard.rs`
7. 编译验证

### Phase 2: 核心标签

8. 新建 `save_editor.rs`（合并 save_panel + dashboard）
9. 重构 `realtime_panel.rs` → `realtime_editor.rs`
10. 字段表新增差异列/锁定列/操作列
11. 自动刷新改用实际时间

### Phase 3: 辅助标签

12. 重构 `backup.rs`（差异对比）
13. 重构 `toolbox.rs`（4 工具可折叠）
14. 重构 `settings.rs`
15. `engine_display_name` 提取为公共函数

### Phase 4: 打磨

16. 确认对话框组件
17. 边界情况处理
18. 编译 + 测试验证
