//! Unreal Engine GVAS 存档格式处理器

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use game_tool_core::{backup, error::GameToolError, ISaveFormat, ModifiableField, SaveSummary};
use serde_json::Value;

const MAGIC: &[u8] = b"GVAS";

pub struct UnrealGVASFormat;

impl Default for UnrealGVASFormat {
    fn default() -> Self {
        Self
    }
}

impl UnrealGVASFormat {
    pub fn new() -> Self {
        Self
    }
}

impl ISaveFormat for UnrealGVASFormat {
    fn name(&self) -> &str {
        "Unreal Engine (GVAS)"
    }
    fn extensions(&self) -> Vec<String> {
        vec![".sav".into()]
    }
    fn engine_type(&self) -> &str {
        "unreal"
    }
    fn magic_bytes(&self) -> Option<&[u8]> {
        Some(MAGIC)
    }

    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        let raw = fs::read(filepath).map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
        if raw.len() < 4 || &raw[0..4] != MAGIC {
            return Err(GameToolError::ArchiveLoadError("无效的 GVAS 格式".into()));
        }

        let header = Self::parse_header(&raw);
        let data_offset = header
            .get("_data_offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(36) as usize;
        let props = Self::extract_properties(&raw, data_offset);

        let mut data = serde_json::Map::new();
        data.insert("_format".into(), Value::String("unreal_gvas".into()));
        data.insert(
            "_raw".into(),
            Value::String(game_tool_core::base64::encode(&raw)),
        );
        data.insert("_header".into(), Value::Object(header));
        data.insert("_props".into(), Value::Object(props));

        Ok(Value::Object(data))
    }

    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);

        let raw_bytes = data
            .get("_raw")
            .and_then(|v| v.as_str())
            .and_then(game_tool_core::base64::decode)
            .ok_or_else(|| GameToolError::ArchiveSaveError("缺少 _raw 数据".into()))?;

        let data_offset = data
            .get("_header")
            .and_then(|h| h.get("_data_offset"))
            .and_then(|v| v.as_u64())
            .unwrap_or(36) as usize;

        let props = data
            .get("_props")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        let props_binary = Self::serialize_properties(&props);
        let original_props_end = Self::find_original_props_end(&raw_bytes, data_offset);

        let mut output = Vec::with_capacity(
            data_offset + props_binary.len() + raw_bytes.len().saturating_sub(original_props_end),
        );
        output.extend_from_slice(&raw_bytes[..data_offset]);
        output.extend_from_slice(&props_binary);
        if original_props_end < raw_bytes.len() {
            output.extend_from_slice(&raw_bytes[original_props_end..]);
        }

        fs::write(path, &output).map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        Ok(())
    }

    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["Saved/SaveGames", "Saved", "SaveGames"] {
            let d = dir.join(sub);
            if d.is_dir() {
                return Some(d.to_string_lossy().to_string());
            }
        }
        None
    }

    fn get_summary(&self, data: &Value) -> SaveSummary {
        let props = data.get("_props");
        let gold = props
            .and_then(|p| p.get("Gold"))
            .or_else(|| props.and_then(|p| p.get("Money")))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let play_time = props
            .and_then(|p| p.get("PlayTime"))
            .or_else(|| props.and_then(|p| p.get("RealTimeSeconds")))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as i32;
        let prop_count = props
            .and_then(|p| p.as_object())
            .map(|m| m.len() as i32)
            .unwrap_or(0);

        SaveSummary {
            gold,
            play_time,
            item_count: prop_count,
            ..Default::default()
        }
    }

    fn scan_fields(&self, data: &Value, _game_dir: &str) -> Vec<ModifiableField> {
        let mut fields = Vec::new();
        if let Some(props) = data.get("_props").and_then(|v| v.as_object()) {
            for (key, value) in props {
                let field_type = match value {
                    Value::Bool(_) => "bool",
                    Value::Number(n) if n.is_f64() => "float",
                    Value::Number(_) => "int",
                    Value::String(_) => "str",
                    _ => "str",
                };
                let display_name = KNOWN_NAMES
                    .get(key.as_str())
                    .copied()
                    .unwrap_or(key.as_str());
                fields.push(ModifiableField {
                    category: "gvas".into(),
                    field_id: format!("gvas_{}", key),
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
        let key = field
            .field_id
            .strip_prefix("gvas_")
            .unwrap_or(&field.field_id)
            .to_string();
        if let Some(props) = data.pointer_mut("/_props") {
            if let Some(obj) = props.as_object_mut() {
                obj.insert(key, field.save_value.clone());
            }
        }
        Ok(())
    }
}

impl UnrealGVASFormat {
    fn parse_header(raw: &[u8]) -> serde_json::Map<String, Value> {
        let mut header = serde_json::Map::new();
        if raw.len() < 36 {
            return header;
        }

        let sv = i32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
        let pv = i32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]);
        let em = u16::from_le_bytes([raw[12], raw[13]]);
        let e_min = u16::from_le_bytes([raw[14], raw[15]]);
        let ep = u16::from_le_bytes([raw[16], raw[17]]);
        let eb = u32::from_le_bytes([raw[18], raw[19], raw[20], raw[21]]);
        let branch = Self::read_cstring(&raw[22..38]);
        let cfv = i32::from_le_bytes([raw[38], raw[39], raw[40], raw[41]]);
        let custom_count = i32::from_le_bytes([raw[42], raw[43], raw[44], raw[45]]) as usize;

        let mut offset = 46;
        for _ in 0..custom_count.min(64) {
            if offset + 16 > raw.len() {
                break;
            }
            offset += 16;
        }
        if offset + 4 > raw.len() {
            return header;
        }
        let type_len = i32::from_le_bytes([
            raw[offset],
            raw[offset + 1],
            raw[offset + 2],
            raw[offset + 3],
        ]) as usize;
        offset += 4;
        let save_type = if offset + type_len <= raw.len() {
            String::from_utf8_lossy(&raw[offset..offset + type_len])
                .trim_end_matches('\0')
                .to_string()
        } else {
            String::new()
        };
        offset += type_len;

        header.insert("_saveGameVersion".into(), Value::Number(sv.into()));
        header.insert("_packageVersion".into(), Value::Number(pv.into()));
        header.insert("_engineMajor".into(), Value::Number(em.into()));
        header.insert("_engineMinor".into(), Value::Number(e_min.into()));
        header.insert("_enginePatch".into(), Value::Number(ep.into()));
        header.insert("_engineBuild".into(), Value::Number(eb.into()));
        header.insert("_branch".into(), Value::String(branch));
        header.insert("_customFormatVersion".into(), Value::Number(cfv.into()));
        header.insert("_saveGameType".into(), Value::String(save_type));
        header.insert("_data_offset".into(), Value::Number((offset as u64).into()));
        header
    }

    fn extract_properties(raw: &[u8], start: usize) -> serde_json::Map<String, Value> {
        let mut props = serde_json::Map::new();
        let mut offset = start.min(raw.len());
        while offset + 4 < raw.len() {
            let name_end = raw[offset..].iter().position(|&b| b == 0);
            let name = match name_end {
                Some(len) if len > 0 => {
                    String::from_utf8_lossy(&raw[offset..offset + len]).to_string()
                }
                _ => break,
            };
            offset += name.len() + 1;
            if offset >= raw.len() {
                break;
            }

            match raw[offset] {
                0x02 => {
                    // IntProperty
                    if offset + 9 <= raw.len() {
                        let val = i64::from_le_bytes(
                            raw[offset + 1..offset + 9].try_into().unwrap_or([0; 8]),
                        );
                        props.insert(name, Value::Number(val.into()));
                        offset += 9;
                    } else {
                        break;
                    }
                }
                0x03 => {
                    // FloatProperty
                    if offset + 5 <= raw.len() {
                        let val = f32::from_le_bytes(
                            raw[offset + 1..offset + 5].try_into().unwrap_or([0; 4]),
                        );
                        if let Some(n) = serde_json::Number::from_f64(val as f64) {
                            props.insert(name, Value::Number(n));
                        }
                        offset += 5;
                    } else {
                        break;
                    }
                }
                0x04 => {
                    // StrProperty
                    if offset + 5 <= raw.len() {
                        let len = i32::from_le_bytes(
                            raw[offset + 1..offset + 5].try_into().unwrap_or([0; 4]),
                        ) as usize;
                        offset += 5;
                        if offset + len <= raw.len() {
                            let s = String::from_utf8_lossy(&raw[offset..offset + len])
                                .trim_end_matches('\0')
                                .to_string();
                            props.insert(name, Value::String(s));
                            offset += if len > 0 { len } else { 1 };
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                0x08 => {
                    // BoolProperty
                    if offset + 2 <= raw.len() {
                        props.insert(name, Value::Bool(raw[offset + 1] != 0));
                        offset += 2;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        props
    }

    fn read_cstring(data: &[u8]) -> String {
        let pos = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        String::from_utf8_lossy(&data[..pos])
            .trim_end_matches('\0')
            .to_string()
    }

    fn serialize_properties(props: &serde_json::Map<String, Value>) -> Vec<u8> {
        let mut buf = Vec::new();
        for (name, value) in props {
            buf.extend_from_slice(name.as_bytes());
            buf.push(0);
            match value {
                Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        if f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                            buf.push(0x02);
                            buf.extend_from_slice(&(f as i64).to_le_bytes());
                        } else {
                            buf.push(0x03);
                            buf.extend_from_slice(&(f as f32).to_le_bytes());
                        }
                    }
                }
                Value::String(s) => {
                    buf.push(0x04);
                    let bytes = s.as_bytes();
                    buf.extend_from_slice(&(bytes.len() as i32).to_le_bytes());
                    buf.extend_from_slice(bytes);
                }
                Value::Bool(b) => {
                    buf.push(0x08);
                    buf.push(if *b { 1 } else { 0 });
                }
                _ => {}
            }
        }
        buf
    }

    fn find_original_props_end(raw: &[u8], start: usize) -> usize {
        let mut offset = start.min(raw.len());
        while offset + 4 < raw.len() {
            let name_end = raw[offset..].iter().position(|&b| b == 0);
            match name_end {
                Some(len) if len > 0 => {
                    offset += len + 1;
                }
                _ => break,
            }
            if offset >= raw.len() {
                break;
            }
            match raw[offset] {
                0x02 => offset += 9,
                0x03 => offset += 5,
                0x04 => {
                    if offset + 5 > raw.len() {
                        break;
                    }
                    let len = i32::from_le_bytes(
                        raw[offset + 1..offset + 5].try_into().unwrap_or([0; 4]),
                    ) as usize;
                    offset += 5 + len.max(1);
                }
                0x08 => offset += 2,
                _ => break,
            }
        }
        offset
    }
}

static KNOWN_NAMES: std::sync::LazyLock<HashMap<&str, &str>> = std::sync::LazyLock::new(|| {
    HashMap::from([
        ("Gold", "金币"),
        ("Money", "金钱"),
        ("Health", "生命值"),
        ("HP", "HP"),
        ("MaxHealth", "最大生命"),
        ("Level", "等级"),
        ("Experience", "经验值"),
        ("PlayTime", "游戏时间"),
        ("RealTimeSeconds", "实时秒数"),
        ("PlayerName", "玩家名"),
        ("SaveSlotName", "存档槽名"),
    ])
});

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_gvas() -> Vec<u8> {
        let mut data = vec![b'G', b'V', b'A', b'S'];
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"++UE5+Release\0\0\0");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&(0u32).to_le_bytes());
        data
    }

    #[test]
    fn test_extensions() {
        let fmt = UnrealGVASFormat::new();
        assert!(fmt.extensions().contains(&".sav".to_string()));
    }

    #[test]
    fn test_magic_bytes() {
        let fmt = UnrealGVASFormat::new();
        assert_eq!(fmt.magic_bytes(), Some(b"GVAS".as_ref()));
    }

    #[test]
    fn test_load_minimal_gvas() {
        let fmt = UnrealGVASFormat::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sav");
        std::fs::write(&path, &make_minimal_gvas()).unwrap();
        let data = fmt.load(&path.to_string_lossy()).unwrap();
        assert_eq!(data["_format"], "unreal_gvas");
    }

    #[test]
    fn test_find_data_dir() {
        let fmt = UnrealGVASFormat::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Saved/SaveGames")).unwrap();
        assert!(fmt.find_data_dir(&dir.path().to_string_lossy()).is_some());
    }
}
