# GameSaveEditor GUI v2 统一改进设计

日期: 2026-05-22 | 版本: v2 | 状态: 待审阅

---

## 一、概述

基于两轮深度代码审查（45 项原始缺口，去重合并为 36 项独立需求），对 GUI 进行系统性改进。核心目标：

1. **变量名断层修复** — 实时面板变量名从硬编码通用名改为游戏内实际名称
2. **功能对等** — 实时面板功能补至与存档面板一致
3. **UX 打磨** — 备份管理、最近游戏、快捷键、tooltip 等实用工具级体验
4. **架构加固** — 代码去重、状态机、异步加载、线程安全

---

## 二、架构变更

### 2.1 核心变更：GameConfig 提升到 AppState

**当前**:
```
switch_game() → 创建 GameConfig 但仅活在 scanner.rs 局部作用域
                 ↓
save panel: scan_fields → scan_all_modifiable → scan_game_directory 加载名称 ✓
realtime panel: game_state_to_fields → 硬编码 "开关 #N" ✗
```

**改后**:
```
switch_game()
  → game_config = Some(scan_game_directory(game_dir))  // 存入 AppState
  → save panel: scan_fields 可复用已有 game_config
  → realtime panel: game_state_to_fields(&gs, &engine, &game_config)  // 使用游戏名称 ✓
```

### 2.2 代码去重

| 重复点 | 改前 | 改后 |
|--------|------|------|
| 字段构造 | `scanner.rs:161行` + `factory.rs:89行` | 提取公共函数 `extract_rpgmaker_fields(data, &GameConfig) -> Vec<ModifiableField>` |
| 字段渲染 | `field_table.rs:29-94行` + `realtime_panel.rs:92-148行` | 提取 `fn render_field_editor(ui, field, readonly, source) -> Option<Value>` 共享编辑控件 |

### 2.3 新字段

```rust
// state.rs - AppState 新增
pub struct AppState {
    pub game_config: Option<GameConfig>,  // [F1] 游戏名称配置
    pub phase: AppPhase,                  // [F24] 显式状态机
    pub recent_games: Vec<String>,        // [F17] 最近游戏列表
    pub dark_mode: bool,                  // [F25] 暗色主题
}

pub enum AppPhase {
    NoGame,
    GameSelected,
    SaveLoaded,
    Connected,
}

// state.rs - RtPanelState 新增
pub struct RtPanelState {
    pub search_query: String,            // [F6]
    pub selected_category: Option<String>, // [F7]
    pub field_count: usize,              // [F11]
    pub auto_refresh: bool,              // [F10]
    pub refresh_timer: u32,              // [F10]
    pub locked_fields: HashSet<String>,  // [F12]
}

// state.rs - SavePanelState 新增
pub struct SavePanelState {
    pub backups: Vec<BackupInfo>,        // [F16]
}

pub struct BackupInfo {
    pub path: String,
    pub timestamp: String,
    pub size: u64,
}
```

---

## 三、Phase 1 — 根基修复（3 项）

### F1: GameConfig 共享

**修改文件**:
- `state.rs`: AppState 新增 `game_config: Option<GameConfig>`
- `app.rs`: `switch_game()` 中调用 `scan_game_directory()` 存入；`AppState::new()` 初始化
- `factory.rs`: `game_state_to_fields(&GameState, &EngineType, &GameConfig)` 新增参数；`rpgmaker_state_to_fields()` 使用 `config.switch_names.get(&i)` 等
- `widgets/summary_card.rs`: 新增 `currency_unit: &str` 参数，标签从硬编码 "金币" 改为 `"{} ({})"` 动态格式

**数据流**:
```
switch_game(dir)
  → self.game_config = Some(scan_game_directory(dir))
  → save panel: format.scan_fields(&data, dir)  // 内部自行调用 scan_game_directory，结果一致
  → realtime panel: drain_rt_results()
      → factory::game_state_to_fields(&gs, &engine, self.game_config.as_ref())
      → display_name = switch_name(&config, i)  // "门" 而非 "开关 #12"
```

### F2: 字段构造去重

**修改文件**:
- `crates/engines/rpgmaker/src/scanner.rs`: 将 `scan_all_modifiable()` 中的字段构造逻辑提取为 `pub fn build_rpgmaker_fields(data: &Value, config: &GameConfig) -> Vec<ModifiableField>`
- `crates/gui/src/factory.rs`: `rpgmaker_state_to_fields()` 改为调用 `scanner::build_rpgmaker_fields()`
- `scanner.rs` 原 `scan_all_modifiable()` 改为调用 `build_rpgmaker_fields()` + 包装

### F3: 字段渲染去重

**修改文件**:
- `crates/gui/src/widgets/field_table.rs`: 新增 `pub fn render_field_editor(ui, field, source: FieldSource) -> Option<Value>` 公共编辑控件
- `crates/gui/src/panels/realtime_panel.rs`: Compact/Full 模式调用 `render_field_editor()`

```rust
pub enum FieldSource { Save, Live }

pub fn render_field_editor(ui: &mut Ui, field: &ModifiableField, source: FieldSource) -> Option<Value> {
    let val = match source { FieldSource::Save => &field.save_value, FieldSource::Live => &field.live_value };
    match field.field_type.as_str() {
        "bool" => { let mut v = val.as_bool().unwrap_or(false); ui.checkbox(&mut v, "").changed().then(|| Value::Bool(v)) }
        "int" => { let mut v = val.as_i64().unwrap_or(0) as i32; ui.add(DragValue::new(&mut v).range(...)).changed().then(|| Value::Number(v.into())) }
        "float" => { let mut v = val.as_f64().unwrap_or(0.0); ui.add(DragValue::new(&mut v)).changed().then(|| serde_json::Number::from_f64(v).map(Value::Number)) }
        _ => { let mut v = val.as_str().unwrap_or("").to_string(); ui.text_edit_singleline(&mut v).changed().then(|| Value::String(v)) }
    }
}
```

---

## 四、Phase 2 — 功能对等（10 项）

### F4-F5: 字段补充

**修改文件**: `factory.rs` — `rpgmaker_state_to_fields()` / `build_rpgmaker_fields()` 补充：
- `selfSwitches` → `self_switch` 类别字段（从 `extensions["self_switches"]` 读取）
- `_level` → actor level 字段
- item 字段零值保留不消失

### F6-F7: 搜索 + 分类过滤

**修改文件**:
- `state.rs`: `RtPanelState` 新增 `search_query`, `selected_category`
- `panels/realtime_panel.rs`: 渲染 `search_bar::render()` + `category_tree::render()`；字段列表按搜索词/选中分类过滤
- `widgets/category_tree.rs`: 标签列表从局部变量提升为常量 `CATEGORY_LABELS`

### F8: Full 模式可编辑

**修改文件**: `panels/realtime_panel.rs` — Full 模式的 `ui.label(&ds)` 替换为 `render_field_editor(ui, f, FieldSource::Live)`

### F9: 移除 20 字段限制

**修改文件**: `panels/realtime_panel.rs:93` — `.take(20)` 改为展示全部字段 + 虚拟滚动（ScrollArea）

### F10: 自动刷新

**修改文件**:
- `state.rs`: `RtPanelState` 新增 `refresh_timer: u32`
- `app.rs`: 每帧 `refresh_timer += 1`；当 `refresh_timer >= 180` 且 `status == Connected` 且 `auto_refresh` 时，发送 `ReadAll` 并重置计时器
- `panels/realtime_panel.rs`: 新增 `[⏸ 暂停刷新]` / `[▶ 自动刷新]` 切换按钮

### F11: 字段计数

**修改文件**: `panels/realtime_panel.rs` — 标题行显示 `"实时修改 (共 {} 个字段)"`

### F12: 冻结/锁定

**修改文件**:
- `state.rs`: `RtPanelState` 新增 `locked_fields: HashSet<String>`
- `panels/realtime_panel.rs`: 每个字段行末尾新增 `🔒` / `🔓` 切换按钮
- `app.rs`: `drain_rt_results()` 中更新 `live_value` 前检查 `locked_fields.contains(&field_id)`，锁定字段跳过更新

### F13: 跨面板拷贝

**修改文件**:
- `panels/save_panel.rs`: 字段行添加 `→` 按钮，点击将 `save_value` 写入实时（发送 `WriteField`）
- `panels/realtime_panel.rs`: 字段行添加 `←` 按钮，点击将 `live_value` 复制到存档的 `save_value` 并标记 dirty
- `app.rs`: 新增 `fn copy_to_save(&mut self, field_id: &str)` 和 `fn copy_to_realtime(&self, field_id: &str, value: Value)` 方法

### F14: 存档面板显示实时值

**修改文件**: `widgets/field_table.rs` — 表头新增"实时值"列，读取 `f.live_value`；当 `live_value != save_value` 时高亮差异

---

## 五、Phase 3 — UX 打磨（15 项）

### F15: 金币变量关联

**修改文件**: `factory.rs` — gold 字段设置 `gold_var_id`；对应变量字段 `display_name` 追加 `" (金币变量)"`

### F16: 备份浏览器

**修改文件**:
- `state.rs`: `SavePanelState` 新增 `backups: Vec<BackupInfo>`
- `panels/save_panel.rs`: 新增 `[备份管理]` 按钮 → 弹窗列出 `.bak` 文件 + `[恢复]` `[删除]` 按钮
- `app.rs`: `fn load_backups()`, `fn restore_backup(path)`, `fn delete_backup(path)` 方法

### F17: 最近游戏列表

**修改文件**:
- `state.rs`: `AppState` 新增 `recent_games: Vec<String>`
- `app.rs`: `AppState::new()` 从 `config.toml` 读取最近游戏；`switch_game()` 成功后将路径写入
- `main.rs`: 若无初始 `game_dir`，替换启动对话框为"最近游戏"列表 + `[浏览...]` 按钮

### F18: 字段 tooltip

**修改文件**: `widgets/field_table.rs` — 名称列 hover 时用 `ui.label(...).on_hover_text(format!("ID: {}\n范围: [{}, {}]", field.field_id, field.min_val, field.max_val))`

### F19: 存档文件信息增强

**修改文件**: `panels/save_panel.rs` — ComboBox 每行改为：`"file1.rpgsave  |  128 KB  |  2026-05-22 14:30"`

### F20: 键盘快捷键

**修改文件**: `app.rs` — `update()` 中 `ctx.input(|i| { if i.key_pressed(Key::S) && i.modifiers.ctrl { save_current() } })` 等

### F21: 插件注入备份

**修改文件**:
- `crates/engines/rpgmaker/src/tcp.rs`: `inject_plugin()` 注入前将目标文件复制为 `.bak.orig`
- `crates/engines/renpy/src/bridge.rs`: 同上

### F22: 线程 panic 防护

**修改文件**: `connection.rs` — `thread::spawn()` 闭包包裹 `std::panic::catch_unwind()`，panic 时发送 `Error("连接异常断开")` 和 `Disconnected`

### F23: 加载指示器

**修改文件**:
- `state.rs`: `SavePanelState` 新增 `loading: bool`
- `panels/save_panel.rs`: 加载中显示 `ui.spinner()` + `"加载中..."`
- `app.rs`: `load_save_file()` 前后设置 `loading`

### F24: 状态机

**修改文件**:
- `state.rs`: 新增 `AppPhase` 枚举
- `app.rs`: 所有 UI 渲染方法查询 `self.phase` 决定启用/禁用

### F25: 暗色主题

**修改文件**:
- `state.rs`: `AppState` 新增 `dark_mode: bool`
- `app.rs`: `update()` 中根据 `dark_mode` 调用 `ctx.set_visuals()`
- `panels/top_bar.rs`: 新增 ☀/🌙 切换按钮

### F26: Compact 支持所有类型编辑

**修改文件**: `panels/realtime_panel.rs` — `_ => ui.label()` 改为 `render_field_editor()`，支持 string TextEdit 和 float DragValue

### F27: Ren'Py 存档提取 store 变量

**修改文件**: `crates/engines/renpy/src/format.rs` — `scan_fields()` 递归遍历 `_meta` JSON Object，将每个叶子键值对转换为 `ModifiableField`

### F28: Generic JSON 显示名增强

**修改文件**: `crates/engines/generic/src/format.rs` — `FIELD_NAME_MAP` 扩展 + `format_key_for_display()`: 驼峰分词 → 下划线拆分 → 查字典映射

### F29: Unreal 提示修正

**修改文件**: `panels/save_panel.rs` — 只读提示改为 `"GVAS 属性可查看，二进制回写尚未支持；编辑不会持久化"`

---

## 六、Phase 4 — 架构加固（8 项）

### F30: 存档完整性校验

**修改文件**: `app.rs` — `load_save_file()` 加载后校验 `data["_format"]` 与 `format.engine_type()` 一致

### F31: 插件版本校验

**修改文件**: `crates/engines/rpgmaker/src/tcp.rs` — `is_plugin_installed()` 除文件存在外还比对内容 hash

### F32: 拖拽支持

**修改文件**: `main.rs` — 使用 `eframe` 的 `drag_and_drop` 支持或 `winit` 文件拖放事件；拖入 `.rpgsave` 等自动加载

### F33: 批量操作

**修改文件**:
- `widgets/field_table.rs`: 新增 Ctrl/Shift 多选支持 + 底部操作栏
- `app.rs`: 批量操作逻辑

### F34: 统一字段提取 trait

**修改文件**: `crates/core/src/types.rs` — 新增 `pub trait FieldExtractor { fn extract(&self, data: &Value, game_dir: &str) -> Vec<ModifiableField>; }`

### F35: 异步存档加载

**修改文件**: `app.rs` — `load_save_file()` 改为后台线程 + `mpsc` channel，UI 显示 spinner 不冻结

### F36: 引擎特定摘要

**修改文件**: `widgets/summary_card.rs` — 接受 `EngineType`，不同引擎渲染不同内容

---

## 七、修改文件总览

| 文件 | P1 | P2 | P3 | P4 | 总改动 |
|------|:--:|:--:|:--:|:--:|:------:|
| `crates/gui/src/state.rs` | ✓ | ✓ | ✓ | ✓ | 4 阶段 |
| `crates/gui/src/app.rs` | ✓ | ✓ | ✓ | ✓ | 4 阶段 |
| `crates/gui/src/factory.rs` | ✓ | ✓ | ✓ | | 3 阶段 |
| `crates/gui/src/main.rs` | | | ✓ | ✓ | 2 阶段 |
| `crates/gui/src/connection.rs` | | | ✓ | | 1 阶段 |
| `crates/gui/src/panels/save_panel.rs` | | ✓ | ✓ | | 2 阶段 |
| `crates/gui/src/panels/realtime_panel.rs` | | ✓ | ✓ | | 2 阶段 |
| `crates/gui/src/panels/top_bar.rs` | | | ✓ | | 1 阶段 |
| `crates/gui/src/widgets/field_table.rs` | ✓ | ✓ | ✓ | ✓ | 4 阶段 |
| `crates/gui/src/widgets/category_tree.rs` | | ✓ | | | 1 阶段 |
| `crates/gui/src/widgets/summary_card.rs` | ✓ | | | ✓ | 2 阶段 |
| `crates/engines/rpgmaker/src/scanner.rs` | ✓ | | | ✓ | 2 阶段 |
| `crates/engines/rpgmaker/src/tcp.rs` | | | ✓ | | 1 阶段 |
| `crates/engines/renpy/src/format.rs` | | | ✓ | | 1 阶段 |
| `crates/engines/renpy/src/bridge.rs` | | | ✓ | | 1 阶段 |
| `crates/engines/generic/src/format.rs` | | | ✓ | | 1 阶段 |
| `crates/core/src/types.rs` | | | | ✓ | 1 阶段 |

**共修改 17 个文件，新建 0 个文件**。

---

## 八、自审

- [x] 无 "TBD" / "TODO" 占位符
- [x] Phase 间依赖关系明确（P1 → P2 → P3 → P4）
- [x] 每个改动有明确文件路径和技术方案
- [x] 与原设计文档不冲突
- [x] 范围适中：36 项，4 个阶段，可在一次开发周期内完成
