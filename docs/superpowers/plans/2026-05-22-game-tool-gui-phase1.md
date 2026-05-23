# Phase 1 实施计划: 根基修复

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix variable name gap (realtime uses generic names while save panel uses game-specific names), and eliminate code duplication in field construction + rendering.

**Architecture:** Three tightly-coupled changes: (1) Promote `GameConfig` from local variable to `AppState` shared field, (2) Extract shared `build_rpgmaker_fields()` from duplicated construction logic, (3) Extract `render_field_editor()` from duplicated rendering logic.

**Tech Stack:** Rust, egui 0.31, game_tool_core, game_tool_rpgmaker

**Spec:** `docs/superpowers/specs/2026-05-22-game-tool-gui-v2-design.md` (Phase 1, F1-F3)

---

### Task 1: F1 — GameConfig 提升到 AppState，实时面板使用游戏名称

**Files:**
- Modify: `crates/gui/src/state.rs:79-90` — AppState 新增 `game_config`
- Modify: `crates/gui/src/app.rs:12-72` — new() 初始化、switch_game() 加载 GameConfig
- Modify: `crates/gui/src/app.rs:239` — drain_rt_results 传入 game_config
- Modify: `crates/gui/src/factory.rs:57-64` — game_state_to_fields 新增参数
- Modify: `crates/gui/src/factory.rs:67-156` — rpgmaker_state_to_fields 使用 GameConfig 名称
- Modify: `crates/gui/src/widgets/summary_card.rs:6-7` — 新增 currency_unit 参数

- [ ] **Step 1: state.rs — AppState 新增 game_config 字段**

在 `use game_tool_core::detector::EngineType;` 后增加导入:
```rust
use game_tool_rpgmaker::scanner::GameConfig;
```

在 `AppState` 的 `pub engine: EngineType,` 后增加:
```rust
    pub game_config: Option<GameConfig>,
```

- [ ] **Step 2: app.rs — new() 初始化 game_config，switch_game() 加载**

在 `AppState::new()` 中，`engine` 检测后添加 `game_config` 初始化。在 `let (engine, game_title) = ...` 块之后增加:
```rust
let game_config = if let Some(ref dir) = game_dir {
    if engine != EngineType::Unknown {
        let gc = game_tool_rpgmaker::scanner::scan_game_directory(dir);
        if gc.data_loaded { Some(gc) } else { None }
    } else { None }
} else { None };

let game_title = game_config.as_ref()
    .map(|gc| gc.game_title.clone())
    .unwrap_or_default();
```

注意：上面会与已有的 `game_title` 冲突。需重构 `new()` 中的初始化逻辑 —— 先检测引擎，再加载 GameConfig，从 GameConfig 中取游戏标题。

完整重构后的 `AppState::new()`:
```rust
pub fn new(game_dir: Option<String>) -> Self {
    let config = load_config().unwrap_or_default();
    let port = config.tcp_port;

    let engine = game_dir.as_ref()
        .map(|d| detect_by_filesystem(d))
        .unwrap_or(EngineType::Unknown);

    let game_config = if let Some(ref dir) = game_dir {
        if engine != EngineType::Unknown {
            let gc = game_tool_rpgmaker::scanner::scan_game_directory(dir);
            if gc.data_loaded { Some(gc) } else { None }
        } else { None }
    } else { None };

    let game_title = game_config.as_ref()
        .map(|gc| gc.game_title.clone())
        .unwrap_or_default();

    let panel_mode = factory::engine_to_panel_mode(&engine);
    let readonly = factory::is_readonly(&engine);
    let format = create_format(&engine);
    let save_files = if let (Some(ref dir), Some(ref fmt)) = (&game_dir, &format) {
        discovery::find_save_files(dir, &**fmt)
    } else { Vec::new() };

    let plugin_installed = if factory::supports_realtime(&engine) {
        if let Some(ref dir) = game_dir {
            match engine {
                EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs =>
                    game_tool_rpgmaker::tcp::is_plugin_installed(dir),
                EngineType::RenPy =>
                    game_tool_renpy::bridge::is_plugin_installed(dir),
                _ => false,
            }
        } else { false }
    } else { false };

    Self {
        game_dir, game_title, engine,
        game_config,
        config,
        save_panel: SavePanelState {
            format, save_files, panel_mode, readonly,
            selected_save: None, save_data: None, summary: None,
            fields: Vec::new(), dirty_count: 0,
            selected_category: None, search_query: String::new(),
        },
        rt_panel: RtPanelState {
            conn: None, fields: Vec::new(),
            view_mode: RtViewMode::Compact,
            plugin_installed,
            host: "127.0.0.1".into(), port,
            pending_connect: false, pending_disconnect: false,
            error_message: String::new(), error_remaining: 0,
            write_feedback: String::new(), write_feedback_remaining: 0,
        },
        status_message: String::new(),
        show_unsaved_dialog: false,
        pending_game_switch: false,
    }
}
```

- [ ] **Step 3: app.rs — switch_game() 同步加载 GameConfig + 插件检测提前**

在 `switch_game()` 的 `self.engine = detect_by_filesystem(&dir_str);` 后增加:
```rust
self.game_config = if self.engine != EngineType::Unknown {
    let gc = game_tool_rpgmaker::scanner::scan_game_directory(&dir_str);
    if gc.data_loaded { self.game_title = gc.game_title.clone(); Some(gc) }
    else { None }
} else { None };

if factory::supports_realtime(&self.engine) { ... } // 现有插件检测代码保留
```

简化：直接替换 `switch_game()` 中 engine 后的逻辑。

- [ ] **Step 4: app.rs — drain_rt_results 传入 game_config**

修改 `drain_rt_results()` 中 line 239 的调用（当前为 `factory::game_state_to_fields(&gs, &self.engine)`）:

```rust
self.rt_panel.fields = factory::game_state_to_fields(
    &gs, &self.engine, self.game_config.as_ref()
);
```

- [ ] **Step 5: factory.rs — game_state_to_fields 新增 &GameConfig 参数**

修改函数签名和分派:
```rust
use game_tool_rpgmaker::scanner::GameConfig;

pub fn game_state_to_fields(
    state: &GameState,
    engine: &EngineType,
    config: Option<&GameConfig>,
) -> Vec<ModifiableField> {
    match engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs =>
            rpgmaker_state_to_fields(state, config),
        EngineType::RenPy =>
            renpy_state_to_fields(state),
        _ => vec![],
    }
}
```

修改 `rpgmaker_state_to_fields` 签名，使用 config 中的名称:
```rust
fn rpgmaker_state_to_fields(state: &GameState, config: Option<&GameConfig>) -> Vec<ModifiableField> {
    let mut fields = Vec::new();
    let ext = &state.extensions;

    // Gold — 使用货币单位
    if let Some(gold) = ext.get("gold").and_then(|v| v.as_i64()) {
        let display_name = config.map(|c| {
            if c.currency_unit.is_empty() { "金币".into() }
            else { format!("金币 ({})", c.currency_unit) }
        }).unwrap_or_else(|| "金币".into());
        fields.push(ModifiableField {
            category: "gold".into(), field_id: "gold".into(),
            display_name, field_type: "int".into(),
            live_value: Value::Number(gold.into()),
            min_val: 0, max_val: 99_999_999,
            ..Default::default()
        });
    }

    // Switches — 使用 switch_name
    if let Some(switches) = ext.get("switches").and_then(|v| v.as_object()) {
        for (k, val) in switches {
            if let Ok(i) = k.parse::<usize>() {
                let display_name = config
                    .map(|c| game_tool_rpgmaker::scanner::switch_name(c, i))
                    .unwrap_or_else(|| format!("开关 #{}", i));
                fields.push(ModifiableField {
                    category: "switch".into(),
                    field_id: format!("switch_{}", i),
                    display_name,
                    item_id: i as i32, field_type: "bool".into(),
                    live_value: val.clone(),
                    min_val: 0, max_val: 1,
                    ..Default::default()
                });
            }
        }
    }

    // Variables — 使用 variable_name
    if let Some(vars) = ext.get("variables").and_then(|v| v.as_object()) {
        for (k, val) in vars {
            if let Ok(i) = k.parse::<usize>() {
                let v = val.as_i64().unwrap_or(0) as i32;
                let display_name = config
                    .map(|c| game_tool_rpgmaker::scanner::variable_name(c, i))
                    .unwrap_or_else(|| format!("变量 #{}", i));
                fields.push(ModifiableField {
                    category: "variable".into(),
                    field_id: format!("var_{}", i),
                    display_name,
                    item_id: i as i32, field_type: "int".into(),
                    live_value: Value::Number(v.into()),
                    min_val: -9_999_999, max_val: 99_999_999,
                    ..Default::default()
                });
            }
        }
    }

    // Items — 使用 item_name
    if let Some(items) = ext.get("items").and_then(|v| v.as_object()) {
        for (k, count) in items {
            if let Ok(i) = k.parse::<usize>() {
                let c = count.as_i64().unwrap_or(0) as i32;
                if c > 0 {
                    let display_name = config
                        .map(|c| game_tool_rpgmaker::scanner::item_name(c, i))
                        .unwrap_or_else(|| format!("物品 #{}", i));
                    fields.push(ModifiableField {
                        category: "item".into(),
                        field_id: format!("item_{}", i),
                        display_name,
                        item_id: i as i32, field_type: "int".into(),
                        live_value: Value::Number(c.into()),
                        min_val: 0, max_val: 999,
                        ..Default::default()
                    });
                }
            }
        }
    }

    // Actors — 使用 actor_name
    if let Some(party) = ext.get("party").and_then(|v| v.as_array()) {
        for actor in party {
            let id = actor.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let hp = actor.get("_hp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let mp = actor.get("_mp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let name = config
                .map(|c| game_tool_rpgmaker::scanner::actor_name(c, id as usize))
                .unwrap_or_else(|| format!("角色 #{}", id));
            fields.push(ModifiableField {
                category: "actor".into(), field_id: format!("actor_{}_hp", id),
                display_name: format!("{} HP", name), item_id: id, field_type: "int".into(),
                live_value: Value::Number(hp.into()),
                min_val: 0, max_val: 999_999,
                ..Default::default()
            });
            fields.push(ModifiableField {
                category: "actor".into(), field_id: format!("actor_{}_mp", id),
                display_name: format!("{} MP", name), item_id: id, field_type: "int".into(),
                live_value: Value::Number(mp.into()),
                min_val: 0, max_val: 999_999,
                ..Default::default()
            });
        }
    }

    fields
}
```

- [ ] **Step 6: summary_card.rs — 新增 currency_unit 参数**

修改 `render` 签名:
```rust
pub fn render(ui: &mut Ui, summary: &SaveSummary, currency_unit: &str) {
```

修改 line 17 的金币标签:
```rust
let gold_label = if currency_unit.is_empty() { "金币".to_string() }
    else { format!("金币 ({})", currency_unit) };
ui.label(format!(
    "{}: {}  队伍: {}人  物品: {}种  存档次数: {}  时长: {}",
    gold_label, summary.gold, summary.party_size, summary.item_count,
    summary.save_count, time_str,
));
```

- [ ] **Step 7: save_panel.rs — 传入 currency_unit 到 summary_card**

在调用 `summary_card::render(ui, summary);` 处增加参数:
```rust
let currency = self.state中... // 需要从 AppState 获取
```

由于 `save_panel.rs` 不持有 AppState 引用，需要修改 `SavePanelState` 或通过参数传递。最简方式：`save_panel::render()` 新增 `game_config: Option<&GameConfig>` 参数。

修改 `save_panel::render` 签名，新增参数，传递给 summary_card。

- [ ] **Step 8: app.rs update() — 传入 game_config 到 save_panel**

修改 line ~343 的 save_panel::render 调用，传入 `self.game_config.as_ref()`。

- [ ] **Step 9: 编译验证**

```bash
cargo check -p game-tool-gui 2>&1
```
Expected: 0 errors, 0 warnings.

---

### Task 2: F2 — 字段构造去重 (scanner.rs 导出 + factory.rs 复用)

**Files:**
- Modify: `crates/engines/rpgmaker/src/scanner.rs` — 新增 `pub fn build_rpgmaker_fields()`
- Modify: `crates/gui/src/factory.rs` — `rpgmaker_state_to_fields` 改为调用 scanner

- [ ] **Step 1: scanner.rs — 提取公共函数**

在 `scan_all_modifiable` 函数之前，新增独立的字段构造函数。它从 `save_data` (Value) 和 `GameConfig` 构建字段列表，不依赖 `GameScanResult` 包装:

```rust
/// 从 RPG Maker 存档数据中提取所有可修改字段（供保存和实时面板共用）
pub fn build_rpgmaker_fields(data: &Value, config: &GameConfig) -> Vec<ModifiableField> {
    let mut fields = Vec::new();

    // Gold
    let gold = data.get("party").and_then(|p| p.get("_gold"))
        .and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let gold_var_id = find_gold_var_id(config, Some(data));
    let gold_name = if config.currency_unit.is_empty() {
        "金币".to_string()
    } else {
        format!("金币 ({})", config.currency_unit)
    };
    fields.push(ModifiableField {
        category: "gold".into(), field_id: "gold".into(),
        display_name: gold_name, field_type: "int".into(),
        save_value: Value::Number(gold.into()),
        min_val: 0, max_val: 99_999_999,
        gold_var_id,
        ..Default::default()
    });

    // Switches
    let switches = extract_map(Some(data), "switches",
        |v| v.as_bool().unwrap_or(false));
    let switch_count = switches.keys().max().map(|k| k + 1).unwrap_or(0);
    for i in 0..switch_count {
        let val = switches.get(&i).copied().unwrap_or(false);
        fields.push(ModifiableField {
            category: "switch".into(),
            field_id: format!("switch_{}", i),
            display_name: switch_name(config, i),
            item_id: i as i32, field_type: "bool".into(),
            save_value: Value::Bool(val),
            min_val: 0, max_val: 1,
            ..Default::default()
        });
    }

    // Variables
    let variables = extract_map(Some(data), "variables",
        |v| v.as_i64().unwrap_or(0) as i32);
    let var_count = variables.keys().max().map(|k| k + 1).unwrap_or(0);
    for i in 0..var_count {
        let val = variables.get(&i).copied().unwrap_or(0);
        let mut name = variable_name(config, i);
        if i as i32 == gold_var_id {
            name = format!("{} (金币变量)", name);
        }
        fields.push(ModifiableField {
            category: "variable".into(),
            field_id: format!("var_{}", i),
            display_name: name,
            item_id: i as i32, field_type: "int".into(),
            save_value: Value::Number(val.into()),
            min_val: -9_999_999, max_val: 99_999_999,
            ..Default::default()
        });
    }

    // Actors
    if let Some(actors) = data.pointer("/party/_actors").and_then(|v| v.as_array()) {
        for actor in actors {
            let id = actor.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let name = actor_name(config, id as usize);
            let hp = actor.get("_hp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let mp = actor.get("_mp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let level = actor.get("_level").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
            fields.push(ModifiableField {
                category: "actor".into(), field_id: format!("actor_{}_hp", id),
                display_name: format!("{} HP", name), item_id: id, field_type: "int".into(),
                save_value: Value::Number(hp.into()),
                min_val: 0, max_val: 999_999, ..Default::default()
            });
            fields.push(ModifiableField {
                category: "actor".into(), field_id: format!("actor_{}_mp", id),
                display_name: format!("{} MP", name), item_id: id, field_type: "int".into(),
                save_value: Value::Number(mp.into()),
                min_val: 0, max_val: 999_999, ..Default::default()
            });
            fields.push(ModifiableField {
                category: "actor".into(), field_id: format!("actor_{}_level", id),
                display_name: format!("{} 等级", name), item_id: id, field_type: "int".into(),
                save_value: Value::Number(level.into()),
                min_val: 1, max_val: 99, ..Default::default()
            });
        }
    }

    // Items
    if let Some(items) = data.pointer("/party/_items").and_then(|v| v.as_object()) {
        for (k, v) in items {
            if crate::jsonex::is_meta_key(k) { continue; }
            let id: i32 = k.parse().unwrap_or(0);
            let count = v.as_i64().unwrap_or(0) as i32;
            if count > 0 {
                fields.push(ModifiableField {
                    category: "item".into(),
                    field_id: format!("item_{}", id),
                    display_name: item_name(config, id as usize),
                    item_id: id, field_type: "int".into(),
                    save_value: Value::Number(count.into()),
                    min_val: 0, max_val: 999, ..Default::default()
                });
            }
        }
    }

    // Self Switches
    if let Some(sw) = data.get("selfSwitches").and_then(|v| v.as_object()) {
        for (k, v) in sw {
            if crate::jsonex::is_meta_key(k) { continue; }
            fields.push(ModifiableField {
                category: "self_switch".into(),
                field_id: format!("ss_{}", k),
                display_name: format!("Self Switch: {}", k),
                field_type: "bool".into(),
                save_value: Value::Bool(v.as_bool().unwrap_or(false)),
                min_val: 0, max_val: 1, ..Default::default()
            });
        }
    }

    fields
}
```

- [ ] **Step 2: scanner.rs — scan_all_modifiable 改为调用新函数**

将 `scan_all_modifiable` 中的字段构造部分替换为调用 `build_rpgmaker_fields`，仅保留 `GameScanResult` 包装逻辑:

```rust
pub fn scan_all_modifiable(game_dir: &str, save_data: Option<&Value>, live_state: Option<&Value>) -> GameScanResult {
    let config = scan_game_directory(game_dir);
    let game_title = if config.game_title.is_empty() {
        std::path::Path::new(game_dir).file_name()
            .map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    } else { config.game_title.clone() };

    let mut result = GameScanResult {
        game_dir: game_dir.into(), game_title, fields: Vec::new(), categories: HashMap::new(),
        has_save_data: save_data.is_some(), has_live_data: live_state.is_some(),
    };

    if let Some(data) = save_data {
        result.fields = build_rpgmaker_fields(data, &config);
    }

    for f in &result.fields {
        result.categories.entry(f.category.clone()).or_default().push(f.clone());
    }
    result
}
```

- [ ] **Step 3: factory.rs — rpgmaker_state_to_fields 改为调用 scanner::build_rpgmaker_fields**

由于 `build_rpgmaker_fields` 从 `data: &Value`（存档格式）构造，而实时数据来自 `GameState.extensions`（不同格式），不能直接复用。保留 factory 中的实时数据提取逻辑，但删除硬编码名称，改为使用 `GameConfig` 的名称函数（已在 Task 1 完成）。

Task 2 的实际效果：`build_rpgmaker_fields` 在 scanner.rs 中作为公共 API 导出。

- [ ] **Step 4: 编译验证**

```bash
cargo check -p game-tool-gui -p game-tool-rpgmaker 2>&1
```
Expected: 0 errors.

---

### Task 3: F3 — 字段渲染去重 (field_table.rs 导出共享编辑器)

**Files:**
- Modify: `crates/gui/src/widgets/field_table.rs` — 新增 `pub fn render_field_editor()` + `FieldSource` 枚举
- Modify: `crates/gui/src/panels/realtime_panel.rs` — Compact/Full 模式调用 render_field_editor

- [ ] **Step 1: field_table.rs — 新增 render_field_editor**

在文件末尾（`value_display` 函数之后）添加:

```rust
pub enum FieldSource {
    Save,
    Live,
}

/// 渲染单个字段的编辑器控件，返回用户修改后的新值
/// read_only: true 时仅显示值不提供编辑
pub fn render_field_editor(ui: &mut Ui, field: &ModifiableField, source: FieldSource) -> Option<Value> {
    let val = match source {
        FieldSource::Save => &field.save_value,
        FieldSource::Live => &field.live_value,
    };

    match field.field_type.as_str() {
        "bool" => {
            let mut v = val.as_bool().unwrap_or(false);
            if ui.checkbox(&mut v, "").changed() {
                Some(Value::Bool(v))
            } else { None }
        }
        "int" => {
            let mut v = val.as_i64().unwrap_or(0) as i32;
            let range = (field.min_val.min(field.max_val) as f64)..=(field.max_val.max(field.min_val) as f64);
            if ui.add(egui::DragValue::new(&mut v).range(range).speed(1)).changed() {
                Some(Value::Number(v.into()))
            } else { None }
        }
        "float" => {
            let mut v = val.as_f64().unwrap_or(0.0);
            if ui.add(egui::DragValue::new(&mut v).speed(0.1)).changed() {
                serde_json::Number::from_f64(v).map(Value::Number)
            } else { None }
        }
        _ => {
            let mut v = val.as_str().unwrap_or("").to_string();
            if ui.text_edit_singleline(&mut v).changed() {
                Some(Value::String(v))
            } else { None }
        }
    }
}
```

- [ ] **Step 2: field_table.rs — render 内部改用 render_field_editor**

在 `render` 函数中，将 line 46-83 的 match 分支（bool/int/float/_ 四个分支的编辑逻辑）替换为:

```rust
if readonly {
    let ds = value_display(&f.save_value);
    ui.label(&ds);
} else {
    if let Some(new_val) = render_field_editor(ui, f, FieldSource::Save) {
        fields[idx].save_value = new_val;
        fields[idx].dirty = true;
    }
}
```

同时删除原 match 中重复的 category/dname 标签行（它们已经在 grid 中渲染）。

- [ ] **Step 3: realtime_panel.rs — Compact 模式改用 render_field_editor**

在 `render_compact` 的 match 分支（line ~94-124）中，将 bool/int/_ 三个分支替换为:

```rust
if let Some(new_val) = field_table::render_field_editor(ui, f, field_table::FieldSource::Live) {
    if is_conn {
        f.live_value = new_val.clone();
        actions.push(RtAction::WriteField(f.field_id.clone(), new_val));
    }
}
```

同时也要支持 string 和 float 类型的编辑（原本 `_ =>` 分支只是 label）。

- [ ] **Step 4: realtime_panel.rs — Full 模式改用 render_field_editor**

将 Full 模式中的 `ui.label(&ds)`（当前是只读的）替换为可编辑版本:

```rust
if let Some(new_val) = field_table::render_field_editor(ui, f, field_table::FieldSource::Live) {
    if is_conn {
        f.live_value = new_val.clone();
        actions.push(RtAction::WriteField(f.field_id.clone(), new_val));
    }
}
```

- [ ] **Step 5: 编译验证 + 测试**

```bash
cargo check -p game-tool-gui 2>&1
cargo test --workspace --exclude game-tool-gui 2>&1
```
Expected: 0 errors, all tests pass.
