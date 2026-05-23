//! 游戏数据扫描器。
//!
//! 扫描 RPG Maker 游戏目录（游戏配置、数据文件）和存档数据，
//! 将所有可修改字段（金币、开关、变量、角色、物品、独立开关）汇总输出。

use crate::jsonex;
use game_tool_core::ModifiableField;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ── 游戏数据扫描 ────────────────────────────────────────

/// 游戏配置信息，从 RPG Maker 数据文件（System.json 等）中提取。
#[derive(Debug, Clone)]
pub struct GameConfig {
    /// 游戏标题
    pub game_title: String,
    /// 货币单位（默认 "G"）
    pub currency_unit: String,
    /// 是否成功加载了数据文件
    pub data_loaded: bool,
    /// 开关名称映射（索引 → 名称）
    pub switch_names: HashMap<usize, String>,
    /// 变量名称映射（索引 → 名称）
    pub variable_names: HashMap<usize, String>,
    /// 角色名称映射（ID → 名称）
    pub actor_names: HashMap<usize, String>,
    /// 物品名称映射（ID → 名称）
    pub item_names: HashMap<usize, String>,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            game_title: String::new(),
            currency_unit: "G".into(),
            data_loaded: false,
            switch_names: HashMap::new(),
            variable_names: HashMap::new(),
            actor_names: HashMap::new(),
            item_names: HashMap::new(),
        }
    }
}

/// 扫描游戏目录，加载配置和数据文件。
///
/// 查找以下文件并提取名称映射：
/// - `System.json`: 游戏标题、货币单位、开关/变量名称
/// - `Actors.json`: 角色 ID → 名称
/// - `Items.json`: 物品 ID → 名称
pub fn scan_game_directory(game_dir: &str) -> GameConfig {
    let mut config = GameConfig::default();
    let data_dir = match find_data_dir(game_dir) {
        Some(d) => d,
        None => return config,
    };

    let sys = load_json(&data_dir, "System.json").unwrap_or_default();
    config.game_title = sys
        .get("gameTitle")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .into();
    config.currency_unit = sys
        .get("currencyUnit")
        .and_then(|v| v.as_str())
        .unwrap_or("G")
        .into();

    // 加载名称映射
    load_names(&sys, "switches", &mut config.switch_names);
    load_names(&sys, "variables", &mut config.variable_names);
    load_id_names(&data_dir, "Actors.json", &mut config.actor_names);
    load_id_names(&data_dir, "Items.json", &mut config.item_names);

    config.data_loaded = true;
    config
}

// ── 内部加载函数 ─────────────────────────────────────────

/// 查找 RPG Maker 游戏的数据目录。
///
/// 搜索 `www/data` 或 `data`，要求存在 `System.json`。
fn find_data_dir(game_dir: &str) -> Option<String> {
    let dir = Path::new(game_dir);
    for sub in &["www/data", "data"] {
        let d = dir.join(sub);
        if d.is_dir() && d.join("System.json").is_file() {
            return Some(d.to_string_lossy().to_string());
        }
    }
    None
}

/// 从数据目录加载 JSON 文件。
fn load_json(data_dir: &str, filename: &str) -> Option<Value> {
    let path = Path::new(data_dir).join(filename);
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// 从 System.json 中加载简单名称数组（开关名/变量名）。
///
/// 数组索引即 ID，非空名称存入映射。
fn load_names(sys: &Value, key: &str, map: &mut HashMap<usize, String>) {
    if let Some(arr) = sys.get(key).and_then(|v| v.as_array()) {
        for (i, name) in arr.iter().enumerate() {
            if let Some(s) = name.as_str() {
                if !s.trim().is_empty() {
                    map.insert(i, s.trim().into());
                }
            }
        }
    }
}

/// 从数据文件中加载 ID → 名称映射（角色/物品）。
///
/// 文件格式为 JSON 对象数组，每条包含 `id` 和 `name` 字段。
fn load_id_names(data_dir: &str, filename: &str, map: &mut HashMap<usize, String>) {
    let arr = load_json(data_dir, filename)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    for item in arr {
        if let (Some(id), Some(name)) = (
            item.get("id").and_then(|v| v.as_i64()),
            item.get("name").and_then(|v| v.as_str()),
        ) {
            if !name.trim().is_empty() {
                map.insert(id as usize, name.trim().into());
            }
        }
    }
}

/// 获取角色显示名（无匹配时返回 "角色 #N"）。
pub fn actor_name(config: &GameConfig, id: usize) -> String {
    config
        .actor_names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("角色 #{}", id))
}

/// 获取物品显示名（无匹配时返回 "物品 #N"）。
pub fn item_name(config: &GameConfig, id: usize) -> String {
    config
        .item_names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("物品 #{}", id))
}

/// 获取开关显示名（无匹配时返回 "开关 #N"）。
pub fn switch_name(config: &GameConfig, id: usize) -> String {
    config
        .switch_names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("开关 #{}", id))
}

/// 获取变量显示名（无匹配时返回 "变量 #N"）。
pub fn variable_name(config: &GameConfig, id: usize) -> String {
    config
        .variable_names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("变量 #{}", id))
}

// ── 字段扫描 ────────────────────────────────────────────

/// 游戏数据扫描结果。
///
/// 汇总了所有可在存档中修改的字段及其分类。
#[derive(Debug, Clone)]
pub struct GameScanResult {
    /// 游戏目录路径
    pub game_dir: String,
    /// 游戏标题
    pub game_title: String,
    /// 是否已加载存档数据
    pub has_save_data: bool,
    /// 是否有实时游戏数据（TCP 桥接）
    pub has_live_data: bool,
    /// 所有可修改字段的列表
    pub fields: Vec<ModifiableField>,
    /// 按类别分组的字段映射
    pub categories: HashMap<String, Vec<ModifiableField>>,
}

/// 执行完整扫描：游戏配置 + 存档数据 + 实时数据。
///
/// 扫描以下类别：
/// - **金币**: `party._gold`
/// - **开关**: `switches` 数组（支持 JSONEx）
/// - **变量**: `variables` 数组（支持 JSONEx）
/// - **角色**: `actors._data.@a` 或 `party._actors`
/// - **物品**: `party._items`（支持 JSONEx 嵌套）
/// - **独立开关**: `selfSwitches` 字典
pub fn scan_all_modifiable(
    game_dir: &str,
    save_data: Option<&Value>,
    live_state: Option<&Value>,
) -> GameScanResult {
    let config = scan_game_directory(game_dir);
    let game_title = if config.game_title.is_empty() {
        // 回退：使用目录名作为标题
        Path::new(game_dir)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        config.game_title.clone()
    };

    let mut result = GameScanResult {
        game_dir: game_dir.into(),
        game_title,
        fields: Vec::new(),
        categories: HashMap::new(),
        has_save_data: save_data.is_some(),
        has_live_data: live_state.is_some(),
    };

    // ── 金币 ──
    let gold = save_data
        .and_then(|d| d.get("party"))
        .and_then(|p| p.get("_gold"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    let gold_var_id = find_gold_var_id(&config, save_data);

    result.fields.push(ModifiableField {
        category: "gold".into(),
        field_id: "gold".into(),
        display_name: format!("金币 ({})", config.currency_unit),
        field_type: "int".into(),
        save_value: Value::Number(gold.into()),
        min_val: 0,
        max_val: 99_999_999,
        gold_var_id,
        ..Default::default()
    });

    // ── 开关 ──
    scan_map_fields(
        &mut result,
        &config,
        save_data,
        live_state,
        "switch",
        "switches",
        |v| v.as_bool().unwrap_or(false), // 解析为 bool
        |id| switch_name(&config, id),
        false, // 默认值
        0,
        1,
    );

    // ── 变量 ──
    scan_map_fields(
        &mut result,
        &config,
        save_data,
        live_state,
        "variable",
        "variables",
        |v| v.as_i64().unwrap_or(0) as i32, // 解析为 i32
        |id| {
            if id as i32 == gold_var_id {
                format!("{} (金币变量)", variable_name(&config, id))
            } else {
                variable_name(&config, id)
            }
        },
        0,           // 默认值
        -9_999_999,  // 最小值
        99_999_999,  // 最大值
    );

    // ── 角色 ──
    // 优先从 actors._data.@a (JSONEx) 读取，回退到 party._actors
    let actor_list: Vec<Value> = save_data
        .and_then(|d| {
            // 尝试 JSONEx: actors._data
            d.get("actors")
                .and_then(|a| a.get("_data"))
                .map(|inner| jsonex::resolve_array(inner))
                .filter(|v| !v.is_empty())
        })
        .or_else(|| {
            // 回退: party._actors 标准格式
            save_data
                .and_then(|d| d.pointer("/party/_actors"))
                .and_then(|v| v.as_array().cloned())
        })
        .unwrap_or_default();

    for actor in &actor_list {
        if !actor.is_object() {
            continue;
        }
        let id = actor.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        if id == 0 {
            continue;
        }
        let name = actor_name(&config, id as usize);
        let hp = actor.get("_hp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let mp = actor.get("_mp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let level = actor.get("_level").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
        result.fields.push(field(
            "actor",
            &format!("actor_{}_hp", id),
            &format!("{} HP", name),
            id,
            hp,
            0,
            999_999,
        ));
        result.fields.push(field(
            "actor",
            &format!("actor_{}_mp", id),
            &format!("{} MP", name),
            id,
            mp,
            0,
            999_999,
        ));
        result.fields.push(field(
            "actor",
            &format!("actor_{}_level", id),
            &format!("{} 等级", name),
            id,
            level,
            1,
            99,
        ));
    }

    // ── 物品 ──
    // 支持 JSONEx _data 嵌套和标准格式
    if let Some(items_val) = save_data.and_then(|d| d.pointer("/party/_items")) {
        // 先尝试 JSONEx: 检查 _data 包装
        let items_obj = if let Some(inner) = items_val.get("_data").and_then(|v| v.as_object()) {
            // _data 自身可能也是 JSONEx: 检查 @a
            if let Some(a) = inner.get("@a").and_then(|v| v.as_object()) {
                // @a 中的稀疏物品字典
                a.iter()
                    .filter(|(k, _)| !jsonex::is_meta_key(k))
                    .filter_map(|(k, v)| Some((k.clone(), v.clone())))
                    .collect::<Vec<_>>()
            } else {
                inner
                    .iter()
                    .filter(|(k, _)| !jsonex::is_meta_key(k))
                    .filter_map(|(k, v)| Some((k.clone(), v.clone())))
                    .collect()
            }
        } else if let Some(obj) = items_val.as_object() {
            // 标准物品字典
            obj.iter()
                .filter(|(k, _)| !jsonex::is_meta_key(k))
                .filter_map(|(k, v)| Some((k.clone(), v.clone())))
                .collect()
        } else {
            Vec::new()
        };

        // 只列出拥有数量 > 0 的物品
        for (k, v) in &items_obj {
            let id: i32 = k.parse().unwrap_or(0);
            let count = v.as_i64().unwrap_or(0) as i32;
            if count > 0 {
                result.fields.push(field(
                    "item",
                    &format!("item_{}", id),
                    &item_name(&config, id as usize),
                    id,
                    count,
                    0,
                    999,
                ));
            }
        }
    }

    // ── 独立开关 ──
    // 支持 JSONEx _data 嵌套格式
    if let Some(sw) = save_data
        .and_then(|d| d.get("selfSwitches"))
        .and_then(|v| v.as_object())
    {
        // 检查 _data 包装（JSONEx 格式）
        let actual = if let Some(inner) = sw.get("_data").and_then(|v| v.as_object()) {
            inner
        } else {
            sw
        };
        for (k, v) in actual {
            if jsonex::is_meta_key(k) {
                continue;
            }
            result.fields.push(ModifiableField {
                category: "self_switch".into(),
                field_id: format!("ss_{}", k),
                display_name: format!("Self Switch: {}", k),
                field_type: "bool".into(),
                save_value: Value::Bool(v.as_bool().unwrap_or(false)),
                min_val: 0,
                max_val: 1,
                ..Default::default()
            });
        }
    }

    // 按类别分组
    for f in &result.fields {
        result
            .categories
            .entry(f.category.clone())
            .or_default()
            .push(f.clone());
    }
    result
}

/// 快速构造一个 `ModifiableField`（辅助函数）。
fn field(
    cat: &str,
    fid: &str,
    name: &str,
    id: i32,
    val: i32,
    min: i32,
    max: i32,
) -> ModifiableField {
    ModifiableField {
        category: cat.into(),
        field_id: fid.into(),
        display_name: name.into(),
        item_id: id,
        field_type: "int".into(),
        save_value: Value::Number(val.into()),
        min_val: min,
        max_val: max,
        ..Default::default()
    }
}

/// 通用数组字段扫描器。
///
/// 扫描开关/变量等数组类型的字段，支持 JSONEx 格式和实时数据合并。
///
/// 类型参数 `T`: 字段值类型（`bool` 用于开关，`i32` 用于变量）。
fn scan_map_fields<T: Clone + Into<Value> + 'static>(
    result: &mut GameScanResult,
    _config: &GameConfig,
    save_data: Option<&Value>,
    live_state: Option<&Value>,
    category: &str,
    data_key: &str,
    parse: fn(&Value) -> T,
    name_fn: impl Fn(usize) -> String,
    default_val: T,
    min: i32,
    max: i32,
) {
    let map = extract_map(save_data, data_key, parse);
    let count = map.keys().max().copied().map(|k| k + 1).unwrap_or(0);
    for i in 0..count {
        let val = map.get(&i).cloned().unwrap_or_else(|| default_val.clone());
        // 尝试从实时状态中获取值
        let live_val = live_state.and_then(|s| {
            s.pointer(&format!("/extensions/{}/{}", data_key, i))
                .and_then(|v| Some(parse(v)))
        });
        result.fields.push(ModifiableField {
            category: category.into(),
            field_id: format!(
                "{}_{}",
                if category == "variable" {
                    "var"
                } else {
                    category
                },
                i
            ),
            display_name: name_fn(i),
            item_id: i as i32,
            // 根据类型参数推断字段类型
            field_type: if std::any::TypeId::of::<T>() == std::any::TypeId::of::<bool>() {
                "bool"
            } else {
                "int"
            }
            .into(),
            save_value: val.into(),
            live_value: live_val.map(|v| v.into()).unwrap_or(Value::Null),
            min_val: min,
            max_val: max,
            ..Default::default()
        });
    }
}

/// 从存档数据中提取数组字段为 `HashMap<usize, T>`。
///
/// 支持三种格式：
/// - 标准 JSON 数组
/// - JSONEx 对象（含 `_data` 包装）
/// - JSONEx `@a` 稀疏数组
fn extract_map<T: Clone>(
    save_data: Option<&Value>,
    key: &str,
    parse: fn(&Value) -> T,
) -> HashMap<usize, T> {
    let mut map = HashMap::new();
    if let Some(data) = save_data {
        if let Some(arr) = data.get(key) {
            if let Some(list) = arr.as_array() {
                // 标准数组
                for (i, v) in list.iter().enumerate() {
                    map.insert(i, parse(v));
                }
            } else if arr.is_object() {
                // 检查 JSONEx _data 包装
                let resolved = if let Some(inner) = arr.get("_data") {
                    jsonex::resolve_array_flat(inner)
                } else {
                    jsonex::resolve_array_flat(arr)
                };
                for (i, v) in resolved.iter().enumerate() {
                    if let Some(val) = v {
                        map.insert(i, parse(val));
                    }
                }
            }
        }
    }
    map
}

/// 查找与金币相关的变量 ID。
///
/// 遍历变量名称映射，查找名称中（不区分大小写）匹配以下关键词的变量：
/// `金币`、`金钱`、`gold`、`money`、`GOLD`、`所持金`
///
/// 要求变量值在有效范围内（> 0 且 < 99,999,999）。
pub fn find_gold_var_id(config: &GameConfig, save_data: Option<&Value>) -> i32 {
    let kw = [
        "金币",
        "金钱",
        "gold",
        "money",
        "Gold",
        "Money",
        "GOLD",
        "所持金",
    ];
    for (&i, name) in &config.variable_names {
        let lower = name.to_lowercase();
        if kw.iter().any(|k| lower == *k || lower.starts_with(k)) {
            if let Some(data) = save_data {
                if let Some(vars) = data.get("variables") {
                    // 先尝试标准数组，再尝试 JSONEx _data.@a
                    let val = vars
                        .as_array()
                        .and_then(|a| a.get(i).and_then(|v| v.as_i64()))
                        .or_else(|| {
                            vars.get("_data")
                                .and_then(|d| d.get("@a"))
                                .and_then(|a| a.get(i.to_string()))
                                .and_then(|v| v.as_i64())
                        })
                        .unwrap_or(0) as i32;
                    if val > 0 && val < 99_999_999 {
                        return i as i32;
                    }
                }
            }
        }
    }
    0
}

// ── 单元测试 ──
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_scan_no_data_dir() {
        let d = tempfile::tempdir().unwrap();
        assert!(!scan_game_directory(&d.path().to_string_lossy()).data_loaded);
    }
    #[test]
    fn test_scan_with_system_json() {
        let d = tempfile::tempdir().unwrap();
        let dd = d.path().join("www/data");
        fs::create_dir_all(&dd).unwrap();
        fs::write(
            dd.join("System.json"),
            json!({"gameTitle":"Test","switches":["","Door"],"variables":["","Steps"]}).to_string(),
        )
        .unwrap();
        let c = scan_game_directory(&d.path().to_string_lossy());
        assert_eq!(c.switch_names.get(&1).unwrap(), "Door");
    }
    #[test]
    fn test_scan_with_actors() {
        let d = tempfile::tempdir().unwrap();
        let dd = d.path().join("www/data");
        fs::create_dir_all(&dd).unwrap();
        fs::write(dd.join("System.json"), r#"{"switches":[],"variables":[]}"#).unwrap();
        fs::write(dd.join("Actors.json"), r#"[{"id":1,"name":"Alice"}]"#).unwrap();
        assert_eq!(
            scan_game_directory(&d.path().to_string_lossy())
                .actor_names
                .get(&1)
                .unwrap(),
            "Alice"
        );
    }
    #[test]
    fn test_name_fallback() {
        let c = GameConfig::default();
        assert_eq!(actor_name(&c, 5), "角色 #5");
        assert_eq!(item_name(&c, 10), "物品 #10");
    }

    fn make_save() -> Value {
        json!({"party":{"_gold":3000,"_actors":[{"_actorId":1,"_name":"A","_hp":100,"_mp":50,"_level":5}],"_items":{"1":10,"2":5}},"switches":[false,true,false],"variables":[0,42,0],"selfSwitches":{"1_1_A":true}})
    }
    #[test]
    fn test_scan_gold() {
        let r = scan_all_modifiable(
            &tempfile::tempdir().unwrap().path().to_string_lossy(),
            Some(&make_save()),
            None,
        );
        assert_eq!(
            r.fields
                .iter()
                .filter(|f| f.category == "gold")
                .next()
                .unwrap()
                .save_value,
            json!(3000)
        );
    }
    #[test]
    fn test_scan_switches() {
        let r = scan_all_modifiable(
            &tempfile::tempdir().unwrap().path().to_string_lossy(),
            Some(&make_save()),
            None,
        );
        assert_eq!(
            r.fields.iter().filter(|f| f.category == "switch").count(),
            3
        );
    }
    #[test]
    fn test_scan_variables() {
        let r = scan_all_modifiable(
            &tempfile::tempdir().unwrap().path().to_string_lossy(),
            Some(&make_save()),
            None,
        );
        assert_eq!(
            r.fields.iter().filter(|f| f.category == "variable").count(),
            3
        );
    }
    #[test]
    fn test_scan_no_save() {
        let r = scan_all_modifiable(
            &tempfile::tempdir().unwrap().path().to_string_lossy(),
            None,
            None,
        );
        assert!(!r.has_save_data);
    }
}
