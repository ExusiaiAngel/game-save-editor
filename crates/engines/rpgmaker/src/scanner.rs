//! 游戏数据扫描器 — 综合扫描游戏内所有可修改项目

use std::collections::HashMap;
use serde_json::Value;
use game_tool_core::ModifiableField;

use crate::gamedata::{self, GameConfig};
use crate::jsonex;

#[derive(Debug, Clone)]
pub struct ScanField {
    pub category: String,
    pub field_id: String,
    pub display_name: String,
    pub item_id: i32,
    pub field_type: String,
    pub save_value: Value,
    pub live_value: Value,
    pub default_value: Value,
    pub min_val: i32,
    pub max_val: i32,
    pub description: String,
    pub dirty: bool,
    pub gold_var_id: i32,
}

#[derive(Debug, Clone)]
pub struct GameScanResult {
    pub game_dir: String,
    pub game_title: String,
    pub has_save_data: bool,
    pub has_live_data: bool,
    pub fields: Vec<ScanField>,
    pub categories: HashMap<String, Vec<ScanField>>,
}

pub fn to_modifiable_field(f: &ScanField) -> ModifiableField {
    ModifiableField {
        category: f.category.clone(),
        field_id: f.field_id.clone(),
        display_name: f.display_name.clone(),
        item_id: f.item_id,
        field_type: f.field_type.clone(),
        save_value: f.save_value.clone(),
        live_value: f.live_value.clone(),
        default_value: f.default_value.clone(),
        min_val: f.min_val,
        max_val: f.max_val,
        description: f.description.clone(),
        dirty: f.dirty,
    }
}

pub fn scan_all_modifiable(
    game_dir: &str,
    save_data: Option<&Value>,
    live_state: Option<&Value>,
) -> GameScanResult {
    let config = gamedata::scan_game_directory(game_dir);
    let game_title = if config.game_title.is_empty() {
        std::path::Path::new(game_dir)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        config.game_title.clone()
    };

    let has_save_data = save_data.is_some();
    let has_live_data = live_state.is_some();

    let mut result = GameScanResult {
        game_dir: game_dir.to_string(),
        game_title,
        has_save_data,
        has_live_data,
        fields: Vec::new(),
        categories: HashMap::new(),
    };

    let gold = save_data
        .and_then(|d| d.get("party"))
        .and_then(|p| p.get("_gold"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    let gold_var_id = find_gold_variable_id(&config, save_data);

    result.fields.push(ScanField {
        category: "gold".into(),
        field_id: "gold".into(),
        display_name: format!("金币 ({})", config.currency_unit),
        item_id: 0,
        field_type: "int".into(),
        save_value: Value::Number(gold.into()),
        live_value: Value::Null,
        default_value: Value::Number(0.into()),
        min_val: 0,
        max_val: 99_999_999,
        description: String::new(),
        dirty: false,
        gold_var_id,
    });

    // Switches
    let switches_map = extract_switches(save_data);
    let switch_count = switches_map.keys().max().copied().map(|k| k + 1).unwrap_or(0);
    for i in 0..switch_count {
        let val = switches_map.get(&i).copied().unwrap_or(false);
        let live_val = live_state.and_then(|s| {
            s.get("extensions")
                .and_then(|e| e.get("switches"))
                .and_then(|sw| sw.get(i.to_string()))
                .and_then(|v| v.as_bool())
        });
        let name = gamedata::switch_name(&config, i);
        result.fields.push(ScanField {
            category: "switch".into(),
            field_id: format!("switch_{}", i),
            display_name: name,
            item_id: i as i32,
            field_type: "bool".into(),
            save_value: Value::Bool(val),
            live_value: live_val.map(Value::Bool).unwrap_or(Value::Null),
            default_value: Value::Bool(false),
            min_val: 0,
            max_val: 1,
            description: String::new(),
            dirty: false,
            gold_var_id: 0,
        });
    }

    // Variables
    let variables_map = extract_variables(save_data);
    let var_count = variables_map.keys().max().copied().map(|k| k + 1).unwrap_or(0);
    for i in 0..var_count {
        let val = variables_map.get(&i).copied().unwrap_or(0);
        let live_val = live_state.and_then(|s| {
            s.get("extensions")
                .and_then(|e| e.get("variables"))
                .and_then(|v| v.get(i.to_string()))
                .and_then(|v| v.as_i64())
        });
        let name = if i as i32 == gold_var_id {
            format!("{} (金币变量)", gamedata::variable_name(&config, i))
        } else {
            gamedata::variable_name(&config, i)
        };
        result.fields.push(ScanField {
            category: "variable".into(),
            field_id: format!("var_{}", i),
            display_name: name,
            item_id: i as i32,
            field_type: "int".into(),
            save_value: Value::Number(val.into()),
            live_value: live_val.map(|v| Value::Number(v.into())).unwrap_or(Value::Null),
            default_value: Value::Number(0.into()),
            min_val: -9_999_999,
            max_val: 99_999_999,
            description: String::new(),
            dirty: false,
            gold_var_id: 0,
        });
    }

    // Actors
    if let Some(actors) = save_data
        .and_then(|d| d.get("party"))
        .and_then(|p| p.get("_actors"))
        .and_then(|v| v.as_array())
    {
        for actor in actors {
            let id = actor.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let name = gamedata::actor_name(&config, id as usize);
            let hp = actor.get("_hp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let mp = actor.get("_mp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let level = actor.get("_level").and_then(|v| v.as_i64()).unwrap_or(1) as i32;

            result.fields.push(ScanField {
                category: "actor".into(),
                field_id: format!("actor_{}_hp", id),
                display_name: format!("{} HP", name),
                item_id: id,
                field_type: "int".into(),
                save_value: Value::Number(hp.into()),
                live_value: Value::Null,
                default_value: Value::Number(1.into()),
                min_val: 0,
                max_val: 999_999,
                description: String::new(),
                dirty: false,
                gold_var_id: 0,
            });
            result.fields.push(ScanField {
                category: "actor".into(),
                field_id: format!("actor_{}_mp", id),
                display_name: format!("{} MP", name),
                item_id: id,
                field_type: "int".into(),
                save_value: Value::Number(mp.into()),
                live_value: Value::Null,
                default_value: Value::Number(0.into()),
                min_val: 0,
                max_val: 999_999,
                description: String::new(),
                dirty: false,
                gold_var_id: 0,
            });
            result.fields.push(ScanField {
                category: "actor".into(),
                field_id: format!("actor_{}_level", id),
                display_name: format!("{} 等级", name),
                item_id: id,
                field_type: "int".into(),
                save_value: Value::Number(level.into()),
                live_value: Value::Null,
                default_value: Value::Number(1.into()),
                min_val: 1,
                max_val: 99,
                description: String::new(),
                dirty: false,
                gold_var_id: 0,
            });
        }
    }

    // Items
    if let Some(items) = save_data
        .and_then(|d| d.get("party"))
        .and_then(|p| p.get("_items"))
        .and_then(|v| v.as_object())
    {
        let filtered = jsonex::filter_meta_keys(items);
        for (k, v) in &filtered {
            let id: i32 = k.parse().unwrap_or(0);
            let count = v.as_i64().unwrap_or(0) as i32;
            if count > 0 {
                let name = gamedata::item_name(&config, id as usize);
                result.fields.push(ScanField {
                    category: "item".into(),
                    field_id: format!("item_{}", id),
                    display_name: name,
                    item_id: id,
                    field_type: "int".into(),
                    save_value: Value::Number(count.into()),
                    live_value: Value::Null,
                    default_value: Value::Number(0.into()),
                    min_val: 0,
                    max_val: 999,
                    description: String::new(),
                    dirty: false,
                    gold_var_id: 0,
                });
            }
        }
    }

    // Self Switches
    if let Some(self_sw) = save_data
        .and_then(|d| d.get("selfSwitches"))
        .and_then(|v| v.as_object())
    {
        let filtered = jsonex::filter_meta_keys(self_sw);
        for (k, v) in &filtered {
            let val = v.as_bool().unwrap_or(false);
            result.fields.push(ScanField {
                category: "self_switch".into(),
                field_id: format!("ss_{}", k),
                display_name: format!("Self Switch: {}", k),
                item_id: 0,
                field_type: "bool".into(),
                save_value: Value::Bool(val),
                live_value: Value::Null,
                default_value: Value::Bool(false),
                min_val: 0,
                max_val: 1,
                description: String::new(),
                dirty: false,
                gold_var_id: 0,
            });
        }
    }

    for field in &result.fields {
        result.categories.entry(field.category.clone()).or_default().push(field.clone());
    }

    result
}

fn extract_switches(save_data: Option<&Value>) -> HashMap<usize, bool> {
    let mut map = HashMap::new();
    if let Some(data) = save_data {
        if let Some(switches) = data.get("switches") {
            if let Some(arr) = switches.as_array() {
                for (i, v) in arr.iter().enumerate() {
                    map.insert(i, v.as_bool().unwrap_or(false));
                }
            } else if switches.is_object() {
                let resolved = jsonex::resolve_array_flat(switches);
                for (i, v) in resolved.iter().enumerate() {
                    if let Some(val) = v {
                        map.insert(i, val.as_bool().unwrap_or(false));
                    }
                }
            }
        }
    }
    map
}

fn extract_variables(save_data: Option<&Value>) -> HashMap<usize, i32> {
    let mut map = HashMap::new();
    if let Some(data) = save_data {
        if let Some(vars) = data.get("variables") {
            if let Some(arr) = vars.as_array() {
                for (i, v) in arr.iter().enumerate() {
                    map.insert(i, v.as_i64().unwrap_or(0) as i32);
                }
            } else if vars.is_object() {
                let resolved = jsonex::resolve_array_flat(vars);
                for (i, v) in resolved.iter().enumerate() {
                    if let Some(val) = v {
                        map.insert(i, val.as_i64().unwrap_or(0) as i32);
                    }
                }
            }
        }
    }
    map
}

fn find_gold_variable_id(config: &GameConfig, save_data: Option<&Value>) -> i32 {
    let currency_keywords = ["金币", "金钱", "gold", "money", "Gold", "Money", "GOLD", "所持金"];
    for (&i, name) in &config.variable_names {
        for kw in &currency_keywords {
            if name.to_lowercase().contains(&kw.to_lowercase()) {
                if let Some(data) = save_data {
                    if let Some(vars) = data.get("variables") {
                        let val = if let Some(arr) = vars.as_array() {
                            arr.get(i).and_then(|v| v.as_i64()).unwrap_or(0) as i32
                        } else {
                            0
                        };
                        if val > 0 && val < 99_999_999 {
                            return i as i32;
                        }
                    }
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

    fn make_save_data() -> Value {
        json!({
            "party": {
                "_gold": 3000,
                "_actors": [
                    {"_actorId": 1, "_name": "Alice", "_hp": 100, "_mp": 50, "_level": 5}
                ],
                "_items": {"1": 10, "2": 5}
            },
            "switches": [false, true, false, false, true],
            "variables": [0, 42, 0, 100, 0],
            "selfSwitches": {"1_1_A": true, "2_3_B": false}
        })
    }

    #[test]
    fn test_scan_creates_gold_field() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let gold: Vec<_> = result.fields.iter().filter(|f| f.category == "gold").collect();
        assert_eq!(gold.len(), 1);
        assert_eq!(gold[0].save_value, json!(3000));
    }

    #[test]
    fn test_scan_creates_switch_fields() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let switches: Vec<_> = result.fields.iter().filter(|f| f.category == "switch").collect();
        assert_eq!(switches.len(), 5);
        assert_eq!(switches[1].save_value, Value::Bool(true));
    }

    #[test]
    fn test_scan_creates_variable_fields() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let vars: Vec<_> = result.fields.iter().filter(|f| f.category == "variable").collect();
        assert_eq!(vars.len(), 5);
        assert_eq!(vars[1].save_value, json!(42));
    }

    #[test]
    fn test_scan_creates_actor_fields() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let actors: Vec<_> = result.fields.iter().filter(|f| f.category == "actor").collect();
        assert_eq!(actors.len(), 3);
    }

    #[test]
    fn test_scan_creates_item_fields() {
        let dir = tempfile::tempdir().unwrap();
        let save = make_save_data();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), Some(&save), None);
        let items: Vec<_> = result.fields.iter().filter(|f| f.category == "item").collect();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_scan_no_save_data() {
        let dir = tempfile::tempdir().unwrap();
        let result = scan_all_modifiable(&dir.path().to_string_lossy(), None, None);
        assert!(!result.has_save_data);
        let gold: Vec<_> = result.fields.iter().filter(|f| f.category == "gold").collect();
        assert_eq!(gold.len(), 1);
        assert_eq!(gold[0].save_value, json!(0));
    }

    #[test]
    fn test_to_modifiable_field() {
        let sf = ScanField {
            category: "gold".into(),
            field_id: "gold".into(),
            display_name: "金币".into(),
            item_id: 0,
            field_type: "int".into(),
            save_value: json!(1000),
            live_value: Value::Null,
            default_value: json!(0),
            min_val: 0,
            max_val: 9999,
            description: String::new(),
            dirty: false,
            gold_var_id: 0,
        };
        let mf = to_modifiable_field(&sf);
        assert_eq!(mf.category, "gold");
        assert_eq!(mf.save_value, json!(1000));
    }
}
