//! RPG Maker MV/MZ 存档格式处理器。
//!
//! 实现 SaveFormat trait，处理 .rpgsave / .rmmzsave 文件。
//! 存档使用 LZ-String 压缩的 Base64 编码 JSON 格式。

use std::fs;
use std::path::Path;

use game_tool_core::{backup, GameToolError, ISaveFormat, ModifiableField, SaveSummary};
use serde_json::Value;

use crate::jsonex;

/// RPG Maker MV/MZ 存档格式处理器。
///
/// 支持标准格式和 JSONEx 扩展格式的存档读取、修改和保存。
pub struct RpgMakerFormat;

impl Default for RpgMakerFormat {
    fn default() -> Self {
        Self
    }
}

impl RpgMakerFormat {
    /// 创建新的 RPG Maker 格式处理器
    pub fn new() -> Self {
        Self
    }

    /// 从文件加载原始存档（LZ-String + Base64 → JSON）。
    ///
    /// 处理流程：
    /// 1. 读取文件文本
    /// 2. 在 Base64 解码 + LZ-String 解压
    /// 3. 将结果解析为 JSON
    fn load_raw(path: &Path) -> Result<Value, GameToolError> {
        let raw = fs::read_to_string(path)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("无法读取文件: {}", e)))?;
        let raw = raw.trim().to_string();
        if raw.is_empty() {
            return Err(GameToolError::ArchiveLoadError("存档文件为空".into()));
        }
        // LZ-String 压缩的 Base64 数据 → JSON 字符串
        let json_str = game_tool_core::lzstring::decompress_from_base64(&raw)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("LZ-String 解压失败: {}", e)))?;
        if json_str.is_empty() {
            return Err(GameToolError::ArchiveLoadError("解压后数据为空".into()));
        }
        let data: Value = serde_json::from_str(&json_str)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("JSON 解析失败: {}", e)))?;
        Ok(data)
    }

    /// 将 JSON 数据保存为原始存档格式（JSON → LZ-String + Base64）。
    ///
    /// 处理流程：
    /// 1. JSON 序列化
    /// 2. LZ-String 压缩
    /// 3. Base64 编码后写入文件
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

    /// 支持的存档扩展名：.rpgsave (MV), .rmmzsave (MZ)
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

    /// 保存存档（自动创建备份）。
    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);
        Self::save_raw(path, data)
    }

    /// 查找游戏数据目录。
    ///
    /// 搜索 `www/data` 或 `data`，需要存在 `System.json` 才认为是有效目录。
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

    /// 提取存档摘要信息。
    ///
    /// 包含：金币、队伍人数、物品数量、存档次数、游戏时间、成员列表。
    /// 支持标准格式和 JSONEx 格式（actors._data.@a）。
    fn get_summary(&self, data: &Value) -> SaveSummary {
        // 金币来源: party._gold
        let gold = data
            .get("party")
            .and_then(|p| p.get("_gold"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        // 队伍人数：先尝试 party._actors，回退到 JSONEx: actors._data.@a
        let party_size = data
            .get("party")
            .and_then(|p| p.get("_actors"))
            .and_then(|v| v.as_array())
            .map(|a| a.len() as i32)
            .or_else(|| {
                data.get("actors")
                    .and_then(|a| a.get("_data"))
                    .map(|inner| jsonex::resolve_array(inner).len() as i32)
            })
            .unwrap_or(0);

        // 物品数量：统计拥有数 > 0 的道具
        let item_count = data
            .get("party")
            .and_then(|p| p.get("_items"))
            .and_then(|v| {
                // 检查 _data 包装（JSONEx）
                if let Some(inner) = v.get("_data").and_then(|d| d.as_object()) {
                    Some(
                        jsonex::filter_meta_keys(inner)
                            .values()
                            .filter(|v| v.as_i64().unwrap_or(0) > 0)
                            .count() as i32,
                    )
                } else if let Some(obj) = v.as_object() {
                    Some(
                        jsonex::filter_meta_keys(obj)
                            .values()
                            .filter(|v| v.as_i64().unwrap_or(0) > 0)
                            .count() as i32,
                    )
                } else {
                    None
                }
            })
            .unwrap_or(0);

        // 存档次数
        let save_count = data
            .get("system")
            .and_then(|s| s.get("_saveCount"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        // 游戏时间（秒）：优先 _playtime，回退到 _framesOnSave / 60
        let play_time = data
            .get("system")
            .and_then(|s| s.get("_playtime"))
            .and_then(|v| v.as_i64())
            .or_else(|| {
                data.get("system")
                    .and_then(|s| s.get("_framesOnSave"))
                    .and_then(|v| v.as_i64())
                    .map(|frames| frames / 60)
            })
            .unwrap_or(0) as i32;

        // 成员列表（ID:名称格式）
        let members = data
            .get("party")
            .and_then(|p| p.get("_actors"))
            .and_then(|v| v.as_array())
            .map(|actors| {
                actors
                    .iter()
                    .filter_map(|a| {
                        let id = a.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0);
                        if id == 0 {
                            return None;
                        }
                        let name = a.get("_name").and_then(|v| v.as_str()).unwrap_or("???");
                        Some(format!("{}:{}", id, name))
                    })
                    .collect()
            })
            .or_else(|| {
                // JSONEx 回退: actors._data.@a
                data.get("actors")
                    .and_then(|a| a.get("_data"))
                    .map(|inner| {
                        jsonex::resolve_array(inner)
                            .iter()
                            .filter_map(|a| {
                                let id = a.get("_actorId").and_then(|v| v.as_i64()).unwrap_or(0);
                                if id == 0 {
                                    return None;
                                }
                                let name = a.get("_name").and_then(|v| v.as_str()).unwrap_or("???");
                                Some(format!("{}:{}", id, name))
                            })
                            .collect()
                    })
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

    /// 扫描所有可修改字段（委托给 scanner 模块）。
    fn scan_fields(&self, data: &Value, game_dir: &str) -> Vec<ModifiableField> {
        crate::scanner::scan_all_modifiable(game_dir, Some(data), None).fields
    }

    /// 应用字段修改。
    ///
    /// 按类别分发处理：
    /// - `gold`: 修改 `party._gold`
    /// - `switch`: 通过 `ensure_switches_array` 修改
    /// - `variable`: 通过 `ensure_variables_array` 修改
    /// - `item`: 修改 `party._items` 中的物品数量
    /// - `actor`: 修改角色 HP/MP/等级（支持 JSONEx 和标准格式）
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
                // 确保 switches 结构存在并获取数组引用
                let arr = jsonex::ensure_switches_array(data);
                let id = field.item_id as usize;
                if id >= arr.len() {
                    arr.resize(id + 1, Value::Bool(false));
                }
                arr[id] = field.save_value.clone();
            }
            "variable" => {
                // 确保 variables 结构存在并获取数组引用
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
                // 根据 field_id 后缀判断修改的属性类型
                if fid.ends_with("_hp") {
                    set_actor_stat(data, field.item_id, "_hp", &field.save_value)?;
                } else if fid.ends_with("_mp") {
                    set_actor_stat(data, field.item_id, "_mp", &field.save_value)?;
                } else if fid.ends_with("_level") {
                    set_actor_stat(data, field.item_id, "_level", &field.save_value)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// 设置指定角色的某个属性值。
///
/// 支持两种数据格式：
/// 1. **JSONEx 格式**: `actors._data.@a[...]`
/// 2. **标准格式**: `party._actors[...]`
///
/// 布尔值会被转换为数字（true → 1, false → 0）。
fn set_actor_stat(
    data: &mut Value,
    actor_id: i32,
    stat: &str,
    value: &Value,
) -> Result<(), GameToolError> {
    // 布尔值归一化为数字
    let val = if value.is_boolean() {
        if value.as_bool().unwrap_or(false) {
            Value::Number(1.into())
        } else {
            Value::Number(0.into())
        }
    } else {
        value.clone()
    };

    // 优先尝试 JSONEx: actors._data.@a
    let updated = data
        .pointer_mut("/actors/_data")
        .and_then(|inner| {
            if let Some(a_arr) = inner.get_mut("@a").and_then(|a| a.as_array_mut()) {
                for actor in a_arr {
                    if actor.get("_actorId").and_then(|v| v.as_i64()) == Some(actor_id as i64) {
                        if let Some(obj) = actor.as_object_mut() {
                            obj.insert(stat.to_string(), val.clone());
                            return Some(true);
                        }
                    }
                }
            }
            None
        })
        .or_else(|| {
            // 回退: party._actors（标准格式）
            data.pointer_mut("/party/_actors").and_then(|actors| {
                if let Some(arr) = actors.as_array_mut() {
                    for actor in arr {
                        if actor.get("_actorId").and_then(|v| v.as_i64()) == Some(actor_id as i64) {
                            if let Some(obj) = actor.as_object_mut() {
                                obj.insert(stat.to_string(), val);
                                return Some(true);
                            }
                        }
                    }
                }
                None
            })
        });

    if updated.is_none() {
        return Err(GameToolError::ArchiveSaveError(format!(
            "角色 #{} 未在存档中找到",
            actor_id
        )));
    }
    Ok(())
}

// ── 单元测试 ──
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
        assert!(
            !backups.is_empty(),
            "backup should be created on second save"
        );
    }
}
