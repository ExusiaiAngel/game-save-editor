//! 游戏数据扫描器 — 扫描 RPG Maker 存档 + 游戏配置

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde_json::Value;
use game_tool_core::ModifiableField;
use crate::jsonex;

// ── 游戏数据扫描 ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GameConfig {
    pub game_title: String,
    pub currency_unit: String,
    pub data_loaded: bool,
    pub switch_names: HashMap<usize, String>,
    pub variable_names: HashMap<usize, String>,
    pub actor_names: HashMap<usize, String>,
    pub item_names: HashMap<usize, String>,
}

impl Default for GameConfig {
    fn default() -> Self { Self {
        game_title: String::new(), currency_unit: "G".into(), data_loaded: false,
        switch_names: HashMap::new(), variable_names: HashMap::new(),
        actor_names: HashMap::new(), item_names: HashMap::new(),
    }}
}

pub fn scan_game_directory(game_dir: &str) -> GameConfig {
    let mut config = GameConfig::default();
    let data_dir = match find_data_dir(game_dir) {
        Some(d) => d, None => return config,
    };

    let sys = load_json(&data_dir, "System.json").unwrap_or_default();
    config.game_title = sys.get("gameTitle").and_then(|v| v.as_str()).unwrap_or("").into();
    config.currency_unit = sys.get("currencyUnit").and_then(|v| v.as_str()).unwrap_or("G").into();

    load_names(&sys, "switches", &mut config.switch_names);
    load_names(&sys, "variables", &mut config.variable_names);
    load_id_names(&data_dir, "Actors.json", &mut config.actor_names);
    load_id_names(&data_dir, "Items.json", &mut config.item_names);

    config.data_loaded = true;
    config
}

// ── 内部加载函数 ─────────────────────────────────────────

fn find_data_dir(game_dir: &str) -> Option<String> {
    let dir = Path::new(game_dir);
    for sub in &["www/data", "data"] {
        let d = dir.join(sub);
        if d.is_dir() && d.join("System.json").is_file() { return Some(d.to_string_lossy().to_string()); }
    }
    None
}
fn load_json(data_dir: &str, filename: &str) -> Option<Value> {
    let path = Path::new(data_dir).join(filename);
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}
fn load_names(sys: &Value, key: &str, map: &mut HashMap<usize, String>) {
    if let Some(arr) = sys.get(key).and_then(|v| v.as_array()) {
        for (i, name) in arr.iter().enumerate() {
            if let Some(s) = name.as_str() { if !s.trim().is_empty() { map.insert(i, s.trim().into()); } }
        }
    }
}
fn load_id_names(data_dir: &str, filename: &str, map: &mut HashMap<usize, String>) {
    let arr = load_json(data_dir, filename).and_then(|v| v.as_array().cloned()).unwrap_or_default();
    for item in arr {
        if let (Some(id), Some(name)) = (item.get("id").and_then(|v| v.as_i64()), item.get("name").and_then(|v| v.as_str())) {
            if !name.trim().is_empty() { map.insert(id as usize, name.trim().into()); }
        }
    }
}

pub fn actor_name(config: &GameConfig, id: usize) -> String { config.actor_names.get(&id).cloned().unwrap_or_else(|| format!("角色 #{}", id)) }
pub fn item_name(config: &GameConfig, id: usize) -> String { config.item_names.get(&id).cloned().unwrap_or_else(|| format!("物品 #{}", id)) }
pub fn switch_name(config: &GameConfig, id: usize) -> String { config.switch_names.get(&id).cloned().unwrap_or_else(|| format!("开关 #{}", id)) }
pub fn variable_name(config: &GameConfig, id: usize) -> String { config.variable_names.get(&id).cloned().unwrap_or_else(|| format!("变量 #{}", id)) }

// ── 字段扫描 ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GameScanResult {
    pub game_dir: String,
    pub game_title: String,
    pub has_save_data: bool,
    pub has_live_data: bool,
    pub fields: Vec<ModifiableField>,
    pub categories: HashMap<String, Vec<ModifiableField>>,
}

pub fn scan_all_modifiable(game_dir: &str, save_data: Option<&Value>, live_state: Option<&Value>) -> GameScanResult {
    let config = scan_game_directory(game_dir);
    let game_title = if config.game_title.is_empty() {
        Path::new(game_dir).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    } else { config.game_title.clone() };

    let mut result = GameScanResult {
        game_dir: game_dir.into(), game_title, fields: Vec::new(), categories: HashMap::new(),
        has_save_data: save_data.is_some(), has_live_data: live_state.is_some(),
    };

    let gold = save_data.and_then(|d| d.get("party")).and_then(|p| p.get("_gold")).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let gold_var_id = find_gold_var_id(&config, save_data);

    result.fields.push(ModifiableField { category: "gold".into(), field_id: "gold".into(),
        display_name: format!("金币 ({})", config.currency_unit), field_type: "int".into(),
        save_value: Value::Number(gold.into()), min_val: 0, max_val: 99_999_999,
        gold_var_id, ..Default::default() });

    scan_map_fields(&mut result, &config, save_data, live_state, "switch", "switches",
        |v| v.as_bool().unwrap_or(false), |id| switch_name(&config, id), false, 0, 1);
    scan_map_fields(&mut result, &config, save_data, live_state, "variable", "variables",
        |v| v.as_i64().unwrap_or(0) as i32, |id| {
            if id as i32 == gold_var_id { format!("{} (金币变量)", variable_name(&config, id)) }
            else { variable_name(&config, id) }
        }, 0, -9_999_999, 99_999_999);

    // 角色
    if let Some(actors) = save_data.and_then(|d| d.pointer("/party/_actors")).and_then(|v| v.as_array()) {
        for actor in actors {
            let id = actor.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let name = actor_name(&config, id as usize);
            let hp = actor.get("_hp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let mp = actor.get("_mp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let level = actor.get("_level").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
            result.fields.push(field("actor", &format!("actor_{}_hp", id), &format!("{} HP", name), id, hp, 0, 999_999));
            result.fields.push(field("actor", &format!("actor_{}_mp", id), &format!("{} MP", name), id, mp, 0, 999_999));
            result.fields.push(field("actor", &format!("actor_{}_level", id), &format!("{} 等级", name), id, level, 1, 99));
        }
    }

    // 物品
    if let Some(items) = save_data.and_then(|d| d.pointer("/party/_items")).and_then(|v| v.as_object()) {
        for (k, v) in items {
            if jsonex::is_meta_key(k) { continue; }
            let id: i32 = k.parse().unwrap_or(0); let count = v.as_i64().unwrap_or(0) as i32;
            if count > 0 { result.fields.push(field("item", &format!("item_{}", id), &item_name(&config, id as usize), id, count, 0, 999)); }
        }
    }

    // Self Switches
    if let Some(sw) = save_data.and_then(|d| d.get("selfSwitches")).and_then(|v| v.as_object()) {
        for (k, v) in sw {
            if jsonex::is_meta_key(k) { continue; }
            result.fields.push(ModifiableField { category: "self_switch".into(), field_id: format!("ss_{}", k),
                display_name: format!("Self Switch: {}", k), field_type: "bool".into(),
                save_value: Value::Bool(v.as_bool().unwrap_or(false)), min_val: 0, max_val: 1,
                ..Default::default() });
        }
    }

    for f in &result.fields { result.categories.entry(f.category.clone()).or_default().push(f.clone()); }
    result
}

fn field(cat: &str, fid: &str, name: &str, id: i32, val: i32, min: i32, max: i32) -> ModifiableField {
    ModifiableField { category: cat.into(), field_id: fid.into(), display_name: name.into(),
        item_id: id, field_type: "int".into(), save_value: Value::Number(val.into()),
        min_val: min, max_val: max, ..Default::default() }
}

fn scan_map_fields<T: Clone + Into<Value> + 'static>(
    result: &mut GameScanResult, _config: &GameConfig,
    save_data: Option<&Value>, live_state: Option<&Value>,
    category: &str, data_key: &str,
    parse: fn(&Value) -> T,
    name_fn: impl Fn(usize) -> String,
    default_val: T, min: i32, max: i32,
) {
    let map = extract_map(save_data, data_key, parse);
    let count = map.keys().max().copied().map(|k| k + 1).unwrap_or(0);
    for i in 0..count {
        let val = map.get(&i).cloned().unwrap_or_else(|| default_val.clone());
        let live_val = live_state.and_then(|s| s.pointer(&format!("/extensions/{}/{}", data_key, i)).and_then(|v| Some(parse(v))));
        result.fields.push(ModifiableField {
            category: category.into(), field_id: format!("{}_{}", if category == "variable" { "var" } else { category }, i),
            display_name: name_fn(i), item_id: i as i32,
            field_type: if std::any::TypeId::of::<T>() == std::any::TypeId::of::<bool>() { "bool" } else { "int" }.into(),
            save_value: val.into(), live_value: live_val.map(|v| v.into()).unwrap_or(Value::Null),
            min_val: min, max_val: max, ..Default::default()
        });
    }
}

fn extract_map<T: Clone>(save_data: Option<&Value>, key: &str, parse: fn(&Value) -> T) -> HashMap<usize, T> {
    let mut map = HashMap::new();
    if let Some(data) = save_data {
        if let Some(arr) = data.get(key) {
            if let Some(list) = arr.as_array() {
                for (i, v) in list.iter().enumerate() { map.insert(i, parse(v)); }
            } else if arr.is_object() {
                let resolved = jsonex::resolve_array_flat(arr);
                for (i, v) in resolved.iter().enumerate() {
                    if let Some(val) = v { map.insert(i, parse(val)); }
                }
            }
        }
    }
    map
}

fn find_gold_var_id(config: &GameConfig, save_data: Option<&Value>) -> i32 {
    let kw = ["金币", "金钱", "gold", "money", "Gold", "Money", "GOLD", "所持金"];
    for (&i, name) in &config.variable_names {
        let lower = name.to_lowercase();
        if kw.iter().any(|k| lower.contains(&k.to_lowercase())) {
            if let Some(data) = save_data {
                if let Some(vars) = data.get("variables") {
                    let val = vars.as_array().and_then(|a| a.get(i).and_then(|v| v.as_i64())).unwrap_or(0) as i32;
                    if val > 0 && val < 99_999_999 { return i as i32; }
                }
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test] fn test_scan_no_data_dir() { let d = tempfile::tempdir().unwrap(); assert!(!scan_game_directory(&d.path().to_string_lossy()).data_loaded); }
    #[test] fn test_scan_with_system_json() {
        let d = tempfile::tempdir().unwrap(); let dd = d.path().join("www/data"); fs::create_dir_all(&dd).unwrap();
        fs::write(dd.join("System.json"), json!({"gameTitle":"Test","switches":["","Door"],"variables":["","Steps"]}).to_string()).unwrap();
        let c = scan_game_directory(&d.path().to_string_lossy()); assert_eq!(c.switch_names.get(&1).unwrap(), "Door");
    }
    #[test] fn test_scan_with_actors() {
        let d = tempfile::tempdir().unwrap(); let dd = d.path().join("www/data"); fs::create_dir_all(&dd).unwrap();
        fs::write(dd.join("System.json"), r#"{"switches":[],"variables":[]}"#).unwrap();
        fs::write(dd.join("Actors.json"), r#"[{"id":1,"name":"Alice"}]"#).unwrap();
        assert_eq!(scan_game_directory(&d.path().to_string_lossy()).actor_names.get(&1).unwrap(), "Alice");
    }
    #[test] fn test_name_fallback() { let c = GameConfig::default(); assert_eq!(actor_name(&c, 5), "角色 #5"); assert_eq!(item_name(&c, 10), "物品 #10"); }

    fn make_save() -> Value { json!({"party":{"_gold":3000,"_actors":[{"_actorId":1,"_name":"A","_hp":100,"_mp":50,"_level":5}],"_items":{"1":10,"2":5}},"switches":[false,true,false],"variables":[0,42,0],"selfSwitches":{"1_1_A":true}}) }
    #[test] fn test_scan_gold() { let r = scan_all_modifiable(&tempfile::tempdir().unwrap().path().to_string_lossy(), Some(&make_save()), None); assert_eq!(r.fields.iter().filter(|f| f.category=="gold").next().unwrap().save_value, json!(3000)); }
    #[test] fn test_scan_switches() { let r = scan_all_modifiable(&tempfile::tempdir().unwrap().path().to_string_lossy(), Some(&make_save()), None); assert_eq!(r.fields.iter().filter(|f| f.category=="switch").count(), 3); }
    #[test] fn test_scan_variables() { let r = scan_all_modifiable(&tempfile::tempdir().unwrap().path().to_string_lossy(), Some(&make_save()), None); assert_eq!(r.fields.iter().filter(|f| f.category=="variable").count(), 3); }
    #[test] fn test_scan_no_save() { let r = scan_all_modifiable(&tempfile::tempdir().unwrap().path().to_string_lossy(), None, None); assert!(!r.has_save_data); }
}
