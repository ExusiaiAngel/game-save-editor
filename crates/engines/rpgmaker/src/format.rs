//! RPG Maker MV/MZ 存档格式处理器
//!
//! 实现 SaveFormat trait，处理 .rpgsave / .rmmzsave 文件。

use std::fs;
use std::path::Path;

use serde_json::Value;
use game_tool_core::{
    ISaveFormat, ModifiableField, SaveSummary, GameToolError,
    backup,
};

use crate::jsonex;

pub struct RpgMakerFormat;

impl Default for RpgMakerFormat {
    fn default() -> Self {
        Self
    }
}

impl RpgMakerFormat {
    pub fn new() -> Self {
        Self
    }

    fn load_raw(path: &Path) -> Result<Value, GameToolError> {
        let raw = fs::read_to_string(path)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("无法读取文件: {}", e)))?;
        let raw = raw.trim().to_string();
        if raw.is_empty() {
            return Err(GameToolError::ArchiveLoadError("存档文件为空".into()));
        }
        let json_str = game_tool_core::lzstring::decompress_from_base64(&raw)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("LZ-String 解压失败: {}", e)))?;
        if json_str.is_empty() {
            return Err(GameToolError::ArchiveLoadError("解压后数据为空".into()));
        }
        let data: Value = serde_json::from_str(&json_str)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("JSON 解析失败: {}", e)))?;
        Ok(data)
    }

    fn save_raw(path: &Path, data: &Value) -> Result<(), GameToolError> {
        let json_str = serde_json::to_string(data)
            .map_err(|e| GameToolError::ArchiveSaveError(format!("JSON 序列化失败: {}", e)))?;
        let compressed = game_tool_core::lzstring::compress_to_base64(&json_str)
            .map_err(|e| GameToolError::ArchiveSaveError(format!("LZ-String 压缩失败: {}", e)))?;
        fs::write(path, &compressed)
            .map_err(|e| GameToolError::ArchiveSaveError(format!("写入文件失败: {}", e)))?;
        Ok(())
    }
}

impl ISaveFormat for RpgMakerFormat {
    fn name(&self) -> &str {
        "RPG Maker MV/MZ"
    }

    fn extensions(&self) -> Vec<String> {
        vec![".rpgsave".into(), ".rmmzsave".into()]
    }

    fn engine_type(&self) -> &str {
        "rpg_maker_mv"
    }

    fn magic_bytes(&self) -> Option<&[u8]> {
        None
    }

    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        Self::load_raw(Path::new(filepath))
    }

    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);
        Self::save_raw(path, data)
    }

    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["www/data", "data"] {
            let d = dir.join(sub);
            if d.is_dir() && d.join("System.json").is_file() {
                return Some(d.to_string_lossy().to_string());
            }
        }
        None
    }

    fn get_summary(&self, data: &Value) -> SaveSummary {
        let gold = data.get("party")
            .and_then(|p| p.get("_gold"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let party_size = data.get("party")
            .and_then(|p| p.get("_actors"))
            .and_then(|v| v.as_array())
            .map(|a| a.len() as i32)
            .unwrap_or(0);

        let item_count = data.get("party")
            .and_then(|p| p.get("_items"))
            .and_then(|v| v.as_object())
            .map(|m| {
                jsonex::filter_meta_keys(m)
                    .values()
                    .filter(|v| v.as_i64().unwrap_or(0) > 0)
                    .count() as i32
            })
            .unwrap_or(0);

        let save_count = data.get("system")
            .and_then(|s| s.get("_saveCount"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let play_time = data.get("system")
            .and_then(|s| s.get("_playtime"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let members = data.get("party")
            .and_then(|p| p.get("_actors"))
            .and_then(|v| v.as_array())
            .map(|actors| {
                actors.iter()
                    .map(|a| {
                        let id = a.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0);
                        let name = a.get("_name").and_then(|v| v.as_str()).unwrap_or("???");
                        format!("{}:{}", id, name)
                    })
                    .collect()
            })
            .unwrap_or_default();

        SaveSummary {
            gold,
            party_size,
            item_count,
            save_count,
            play_time,
            members,
            extra: std::collections::HashMap::new(),
        }
    }

    fn scan_fields(&self, data: &Value, game_dir: &str) -> Vec<ModifiableField> {
        crate::scanner::scan_all_modifiable(game_dir, Some(data), None)
            .fields
    }

    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError> {
        let cat = &field.category;
        match cat.as_str() {
            "gold" => {
                if let Some(party) = data.pointer_mut("/party") {
                    if let Some(obj) = party.as_object_mut() {
                        let amount = field.save_value.as_i64().unwrap_or(0);
                        obj.insert("_gold".into(), Value::Number(amount.into()));
                    }
                }
            }
            "switch" => {
                let arr = jsonex::ensure_switches_array(data);
                let id = field.item_id as usize;
                if id >= arr.len() {
                    arr.resize(id + 1, Value::Bool(false));
                }
                arr[id] = field.save_value.clone();
            }
            "variable" => {
                let arr = jsonex::ensure_variables_array(data);
                let id = field.item_id as usize;
                if id >= arr.len() {
                    arr.resize(id + 1, Value::Number(0.into()));
                }
                arr[id] = field.save_value.clone();
            }
            "item" => {
                if let Some(items) = data.pointer_mut("/party/_items") {
                    if let Some(obj) = items.as_object_mut() {
                        let key = field.item_id.to_string();
                        let count = field.save_value.as_i64().unwrap_or(0);
                        obj.insert(key, Value::Number(count.into()));
                    }
                }
            }
            "actor" => {
                let fid = &field.field_id;
                if fid.ends_with("_hp") {
                    set_actor_stat(data, field.item_id, "_hp", &field.save_value);
                } else if fid.ends_with("_mp") {
                    set_actor_stat(data, field.item_id, "_mp", &field.save_value);
                } else if fid.ends_with("_level") {
                    set_actor_stat(data, field.item_id, "_level", &field.save_value);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn set_actor_stat(data: &mut Value, actor_id: i32, stat: &str, value: &Value) {
    if let Some(actors) = data.pointer_mut("/party/_actors") {
        if let Some(arr) = actors.as_array_mut() {
            for actor in arr {
                if let Some(id) = actor.get("_actorId").and_then(|v| v.as_i64()) {
                    if id as i32 == actor_id {
                        if let Some(obj) = actor.as_object_mut() {
                            let val = if value.is_boolean() {
                                if value.as_bool().unwrap_or(false) { Value::Number(1.into()) } else { Value::Number(0.into()) }
                            } else {
                                value.clone()
                            };
                            obj.insert(stat.to_string(), val);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_test_data() -> Value {
        json!({
            "party": {
                "_gold": 5000,
                "_actors": [
                    {"_actorId": 1, "_name": "Alice", "_hp": 100, "_mp": 50, "_level": 5},
                    {"_actorId": 2, "_name": "Bob", "_hp": 80, "_mp": 30, "_level": 4}
                ],
                "_items": {
                    "1": 10,
                    "2": 3
                }
            },
            "switches": [true, false, false, true],
            "variables": [0, 42, 0, 100],
            "system": {
                "_saveCount": 7,
                "_playtime": 3600
            }
        })
    }

    #[test]
    fn test_extensions() {
        let fmt = RpgMakerFormat::new();
        assert!(fmt.extensions().contains(&".rpgsave".to_string()));
        assert!(fmt.extensions().contains(&".rmmzsave".to_string()));
    }

    #[test]
    fn test_magic_bytes_is_none() {
        let fmt = RpgMakerFormat::new();
        assert!(fmt.magic_bytes().is_none());
    }

    #[test]
    fn test_get_summary() {
        let fmt = RpgMakerFormat::new();
        let data = make_test_data();
        let summary = fmt.get_summary(&data);
        assert_eq!(summary.gold, 5000);
        assert_eq!(summary.party_size, 2);
        assert_eq!(summary.save_count, 7);
        assert_eq!(summary.play_time, 3600);
        assert_eq!(summary.members.len(), 2);
    }

    #[test]
    fn test_apply_field_gold() {
        let fmt = RpgMakerFormat::new();
        let mut data = make_test_data();
        let field = ModifiableField {
            category: "gold".into(),
            field_id: "gold".into(),
            display_name: "金币".into(),
            save_value: json!(9999),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
        assert_eq!(data["party"]["_gold"], json!(9999));
    }

    #[test]
    fn test_apply_field_switch() {
        let fmt = RpgMakerFormat::new();
        let mut data = make_test_data();
        let field = ModifiableField {
            category: "switch".into(),
            field_id: "switch_1".into(),
            display_name: "开关1".into(),
            item_id: 1,
            save_value: json!(true),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
    }

    #[test]
    fn test_apply_field_variable() {
        let fmt = RpgMakerFormat::new();
        let mut data = make_test_data();
        let field = ModifiableField {
            category: "variable".into(),
            field_id: "var_2".into(),
            display_name: "变量2".into(),
            item_id: 2,
            save_value: json!(999),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
    }

    #[test]
    fn test_load_save_roundtrip() {
        let fmt = RpgMakerFormat::new();
        let data = make_test_data();
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("test.rpgsave");
        let path_str = save_path.to_string_lossy().to_string();
        fmt.save(&path_str, &data).unwrap();
        assert!(save_path.exists());
        let loaded = fmt.load(&path_str).unwrap();
        assert_eq!(loaded["party"]["_gold"], json!(5000));
        assert_eq!(loaded["system"]["_saveCount"], json!(7));
    }

    #[test]
    fn test_find_data_dir() {
        let fmt = RpgMakerFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let www_data = dir.path().join("www/data");
        fs::create_dir_all(&www_data).unwrap();
        fs::write(www_data.join("System.json"), "{}").unwrap();
        let found = fmt.find_data_dir(&dir.path().to_string_lossy());
        assert!(found.is_some());
        assert!(found.unwrap().ends_with("www/data"));
    }

    #[test]
    fn test_save_creates_backup() {
        let fmt = RpgMakerFormat::new();
        let data = make_test_data();
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("save.rpgsave");
        let path_str = save_path.to_string_lossy().to_string();
        // First save creates the file (no backup since original doesn't exist yet)
        fmt.save(&path_str, &data).unwrap();
        // Second save backs up the previous version
        fmt.save(&path_str, &data).unwrap();
        let backups: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".bak."))
            .collect();
        assert!(!backups.is_empty(), "backup should be created on second save");
    }
}
