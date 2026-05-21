//! 通用 JSON 存档格式处理器

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};
use game_tool_core::{
    ISaveFormat, ModifiableField, SaveSummary,
    error::GameToolError,
    backup,
};

pub struct GenericJsonFormat;

impl Default for GenericJsonFormat {
    fn default() -> Self { Self }
}

impl GenericJsonFormat {
    pub fn new() -> Self { Self }
}

impl ISaveFormat for GenericJsonFormat {
    fn name(&self) -> &str { "JSON (通用)" }
    fn extensions(&self) -> Vec<String> { vec![".json".into()] }
    fn engine_type(&self) -> &str { "generic" }
    fn magic_bytes(&self) -> Option<&[u8]> { None }

    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        let raw = fs::read_to_string(filepath)
            .map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
        let root: Value = serde_json::from_str(&raw)
            .map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;

        let flat = flatten_json(&root, "");
        let mut data = Map::new();
        data.insert("_format".into(), Value::String("generic_json".into()));
        data.insert("_root".into(), root);
        data.insert("_flat".into(), Value::Object(flat));
        Ok(Value::Object(data))
    }

    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);
        let flat = data.get("_flat").and_then(|v| v.as_object())
            .cloned().unwrap_or_default();
        let root = unflatten_json(&flat);
        let json_str = serde_json::to_string_pretty(&root)
            .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        fs::write(path, &json_str)
            .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        Ok(())
    }

    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["data", "saves", "save", "game"] {
            let d = dir.join(sub);
            if d.is_dir() { return Some(d.to_string_lossy().to_string()); }
        }
        None
    }

    fn get_summary(&self, data: &Value) -> SaveSummary {
        let flat = data.get("_flat").and_then(|v| v.as_object());
        let gold = find_gold_like(flat).unwrap_or(0);
        let field_count = flat.map(|m| m.len() as i32).unwrap_or(0);
        SaveSummary {
            gold,
            item_count: field_count,
            ..Default::default()
        }
    }

    fn scan_fields(&self, data: &Value, _game_dir: &str) -> Vec<ModifiableField> {
        let mut fields = Vec::new();
        if let Some(flat) = data.get("_flat").and_then(|v| v.as_object()) {
            for (key, value) in flat {
                let field_type = match value {
                    Value::Bool(_) => "bool",
                    Value::Number(n) if n.is_f64() => "float",
                    Value::Number(_) => "int",
                    Value::String(_) => "str",
                    _ => "str",
                };
                let category = guess_category(key);
                let display_name = FIELD_NAME_MAP.get(key.as_str())
                    .copied().unwrap_or(key.as_str());
                fields.push(ModifiableField {
                    category,
                    field_id: format!("json_{}", key),
                    display_name: display_name.to_string(),
                    field_type: field_type.into(),
                    save_value: value.clone(),
                    ..Default::default()
                });
            }
        }
        fields
    }

    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError> {
        let key = field.field_id.strip_prefix("json_").unwrap_or(&field.field_id).to_string();
        if let Some(flat) = data.pointer_mut("/_flat") {
            if let Some(obj) = flat.as_object_mut() {
                obj.insert(key, field.save_value.clone());
            }
        }
        Ok(())
    }
}

fn flatten_json(value: &Value, prefix: &str) -> Map<String, Value> {
    let mut result = Map::new();
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let new_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };
                if val.is_object() || val.is_array() {
                    result.extend(flatten_json(val, &new_prefix));
                } else {
                    result.insert(new_prefix, val.clone());
                }
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                let new_prefix = format!("{}[{}]", prefix, i);
                result.extend(flatten_json(val, &new_prefix));
            }
        }
        _ => {}
    }
    result
}

fn unflatten_json(flat: &Map<String, Value>) -> Value {
    let mut result = Map::new();
    for (key, value) in flat {
        insert_by_path(&mut result, key, value.clone());
    }
    Value::Object(result)
}

fn insert_by_path(map: &mut Map<String, Value>, path: &str, value: Value) {
    if let Some(dot_pos) = path.find('.') {
        let key = &path[..dot_pos];
        let rest = &path[dot_pos+1..];
        let entry = map.entry(key.to_string()).or_insert_with(|| {
            if rest.starts_with('[') { Value::Array(Vec::new()) } else { Value::Object(Map::new()) }
        });
        insert_by_path_inner(entry, rest, value);
    } else {
        map.insert(path.to_string(), value);
    }
}

fn insert_by_path_inner(node: &mut Value, path: &str, value: Value) {
    if let Some(bracket_end) = path.find(']') {
        let idx: usize = path[1..bracket_end].parse().unwrap_or(0);
        let rest = &path[bracket_end+1..];
        if let Some(arr) = node.as_array_mut() {
            while arr.len() <= idx { arr.push(Value::Null); }
            let rest = rest.strip_prefix('.').unwrap_or(rest);
            if rest.is_empty() {
                arr[idx] = value;
            } else {
                insert_by_path_inner(&mut arr[idx], rest, value);
            }
        }
    } else if let Some(dot_pos) = path.find('.') {
        let key = &path[..dot_pos];
        let rest = &path[dot_pos+1..];
        if let Some(obj) = node.as_object_mut() {
            let entry = obj.entry(key.to_string()).or_insert(Value::Object(Map::new()));
            insert_by_path_inner(entry, rest, value);
        }
    } else if let Some(obj) = node.as_object_mut() {
        obj.insert(path.to_string(), value);
    }
}

fn find_gold_like(flat: Option<&Map<String, Value>>) -> Option<i32> {
    let flat = flat?;
    let keywords = ["gold", "Gold", "money", "Money", "coin", "金币", "金钱"];
    for (key, val) in flat {
        for kw in &keywords {
            if key.to_lowercase().contains(&kw.to_lowercase()) {
                return val.as_i64().map(|v| v as i32);
            }
        }
    }
    None
}

fn guess_category(key: &str) -> String {
    let lower = key.to_lowercase();
    if lower.contains("gold") || lower.contains("money") || lower.contains("金币") {
        return "gold".into();
    }
    if lower.contains("hp") || lower.contains("health") {
        return "actor".into();
    }
    "general".into()
}

static FIELD_NAME_MAP: once_cell::sync::Lazy<HashMap<&str, &str>> = once_cell::sync::Lazy::new(|| {
    HashMap::from([
        ("gold", "金币"), ("money", "金钱"), ("hp", "生命值"), ("health", "生命"),
        ("mp", "魔力值"), ("level", "等级"), ("exp", "经验值"), ("name", "名称"),
    ])
});

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_flatten_simple() {
        let input = json!({"player": {"hp": 100, "mp": 50}, "gold": 1000});
        let flat = flatten_json(&input, "");
        assert_eq!(flat.get("gold").and_then(|v| v.as_i64()), Some(1000));
        assert_eq!(flat.get("player.hp").and_then(|v| v.as_i64()), Some(100));
        assert_eq!(flat.get("player.mp").and_then(|v| v.as_i64()), Some(50));
    }

    #[test]
    fn test_load_roundtrip() {
        let fmt = GenericJsonFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, r#"{"player":{"hp":100,"mp":50},"gold":1000}"#).unwrap();

        let data = fmt.load(&path.to_string_lossy()).unwrap();
        assert_eq!(data["_flat"]["gold"], json!(1000));
        assert_eq!(data["_flat"]["player.hp"], json!(100));
    }

    #[test]
    fn test_scan_fields() {
        let fmt = GenericJsonFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, r#"{"gold":1000,"name":"Hero"}"#).unwrap();

        let data = fmt.load(&path.to_string_lossy()).unwrap();
        let fields = fmt.scan_fields(&data, "");
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn test_apply_field() {
        let fmt = GenericJsonFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, r#"{"gold":1000}"#).unwrap();

        let mut data = fmt.load(&path.to_string_lossy()).unwrap();
        let field = ModifiableField {
            category: "gold".into(),
            field_id: "json_gold".into(),
            display_name: "金币".into(),
            field_type: "int".into(),
            save_value: json!(9999),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
        assert_eq!(data["_flat"]["gold"], json!(9999));
    }

    #[test]
    fn test_save_roundtrip() {
        let fmt = GenericJsonFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("save.json");
        let path_str = save_path.to_string_lossy().to_string();

        std::fs::write(&save_path, r#"{"player":{"hp":100,"mp":50}}"#).unwrap();
        let data = fmt.load(&path_str).unwrap();
        fmt.save(&path_str, &data).unwrap();

        let loaded = fmt.load(&path_str).unwrap();
        assert_eq!(loaded["_flat"]["player.hp"], json!(100));
    }
}
