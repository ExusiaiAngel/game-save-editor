//! Ren'Py 存档格式处理器 (.save ZIP)

use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use game_tool_core::{backup, error::GameToolError, ISaveFormat, ModifiableField, SaveSummary};
use serde_json::Value;
use zip::read::ZipArchive;
use zip::write::{SimpleFileOptions, ZipWriter};

pub struct RenPyFormat;

impl Default for RenPyFormat {
    fn default() -> Self {
        Self
    }
}

impl RenPyFormat {
    pub fn new() -> Self {
        Self
    }
}

impl ISaveFormat for RenPyFormat {
    fn name(&self) -> &str {
        "Ren'Py"
    }
    fn extensions(&self) -> Vec<String> {
        vec![".save".into()]
    }
    fn engine_type(&self) -> &str {
        "renpy"
    }
    fn magic_bytes(&self) -> Option<&[u8]> {
        Some(b"PK\x03\x04")
    }

    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        let file =
            fs::File::open(filepath).map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("ZIP 打开失败: {}", e)))?;

        let mut meta = Value::Null;
        let mut extra_info = String::new();
        let mut log_bytes = Vec::new();
        let mut screenshot = Vec::new();
        let mut renpy_version = String::new();

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
            let name = entry.name().to_string();
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok();

            match name.as_str() {
                "json" => {
                    meta = serde_json::from_slice(&buf).map_err(|e| {
                        GameToolError::ArchiveLoadError(format!("JSON 解析: {}", e))
                    })?;
                }
                "extra_info" => extra_info = String::from_utf8_lossy(&buf).to_string(),
                "log" => log_bytes = buf,
                "screenshot.png" => screenshot = buf,
                "renpy_version" => renpy_version = String::from_utf8_lossy(&buf).trim().to_string(),
                _ => {}
            }
        }

        if meta.is_null() {
            return Err(GameToolError::ArchiveLoadError(
                "ZIP 中缺少 json 条目".into(),
            ));
        }

        let mut data = serde_json::Map::new();
        data.insert("_format".into(), Value::String("renpy".into()));
        data.insert("_meta".into(), meta);
        data.insert("_extra_info".into(), Value::String(extra_info));
        data.insert(
            "_log".into(),
            Value::String(game_tool_core::base64::encode(&log_bytes)),
        );
        data.insert(
            "_screenshot".into(),
            Value::String(game_tool_core::base64::encode(&screenshot)),
        );
        data.insert("_renpy_version".into(), Value::String(renpy_version));

        Ok(Value::Object(data))
    }

    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);

        let meta = data.get("_meta").cloned().unwrap_or(Value::Null);
        let extra_info = data
            .get("_extra_info")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let log_b64 = data.get("_log").and_then(|v| v.as_str()).unwrap_or("");
        let screenshot_b64 = data
            .get("_screenshot")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let renpy_version = data
            .get("_renpy_version")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut buf = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut buf);
            let options = SimpleFileOptions::default();

            writer
                .start_file("json", options)
                .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
            serde_json::to_writer(&mut writer, &meta)
                .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;

            writer
                .start_file("extra_info", options)
                .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
            std::io::Write::write_all(&mut writer, extra_info.as_bytes()).ok();

            if !log_b64.is_empty() {
                if let Some(decoded) = game_tool_core::base64::decode(log_b64) {
                    writer
                        .start_file("log", options)
                        .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
                    std::io::Write::write_all(&mut writer, &decoded).ok();
                }
            }
            if !screenshot_b64.is_empty() {
                if let Some(decoded) = game_tool_core::base64::decode(screenshot_b64) {
                    writer
                        .start_file("screenshot.png", options)
                        .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
                    std::io::Write::write_all(&mut writer, &decoded).ok();
                }
            }
            if !renpy_version.is_empty() {
                writer
                    .start_file("renpy_version", options)
                    .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
                std::io::Write::write_all(&mut writer, renpy_version.as_bytes()).ok();
            }

            writer
                .finish()
                .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        }

        fs::write(path, buf.into_inner())
            .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        Ok(())
    }

    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["game/saves", "game/save", "saves"] {
            let d = dir.join(sub);
            if d.is_dir() {
                return Some(d.to_string_lossy().to_string());
            }
        }
        None
    }

    fn get_summary(&self, data: &Value) -> SaveSummary {
        let meta = data.get("_meta");
        let save_name = meta
            .and_then(|m| m.get("_save_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let version = data
            .get("_renpy_version")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let has_screenshot = data
            .get("_screenshot")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false);

        SaveSummary {
            members: if save_name.is_empty() {
                vec![]
            } else {
                vec![save_name.to_string()]
            },
            extra: {
                let mut m = HashMap::new();
                m.insert("version".into(), Value::String(version.to_string()));
                m.insert("has_screenshot".into(), Value::Bool(has_screenshot));
                m
            },
            ..Default::default()
        }
    }

    fn scan_fields(&self, data: &Value, _game_dir: &str) -> Vec<ModifiableField> {
        let mut fields = Vec::new();
        let extra_info = data
            .get("_extra_info")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let save_name = data
            .get("_meta")
            .and_then(|m| m.get("_save_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let version = data
            .get("_renpy_version")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        fields.push(ModifiableField {
            category: "meta".into(),
            field_id: "renpy_save_name".into(),
            display_name: "存档名称".into(),
            field_type: "str".into(),
            save_value: Value::String(save_name.to_string()),
            ..Default::default()
        });
        fields.push(ModifiableField {
            category: "meta".into(),
            field_id: "renpy_extra_info".into(),
            display_name: "Extra Info".into(),
            field_type: "str".into(),
            save_value: Value::String(extra_info),
            ..Default::default()
        });
        fields.push(ModifiableField {
            category: "meta".into(),
            field_id: "renpy_version".into(),
            display_name: "Ren'Py 版本".into(),
            field_type: "str".into(),
            save_value: Value::String(version.to_string()),
            ..Default::default()
        });

        // Recursively extract all leaf values from _meta (store variables)
        if let Some(meta) = data.get("_meta").and_then(|v| v.as_object()) {
            for (key, value) in meta {
                if key.starts_with('_') {
                    continue;
                }
                collect_meta_leaves(&mut fields, key, value, "");
            }
        }

        fields
    }

    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError> {
        match field.field_id.as_str() {
            "renpy_extra_info" => {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("_extra_info".into(), field.save_value.clone());
                }
            }
            "renpy_save_name" => {
                if let Some(meta) = data.pointer_mut("/_meta") {
                    if let Some(obj) = meta.as_object_mut() {
                        obj.insert("_save_name".into(), field.save_value.clone());
                    }
                }
            }
            fid if fid.starts_with("renpy_meta.") => {
                let path = &fid["renpy_meta.".len()..];
                set_nested_meta(data, path, &field.save_value);
            }
            _ => {}
        }
        Ok(())
    }
}

fn collect_meta_leaves(fields: &mut Vec<ModifiableField>, key: &str, value: &Value, prefix: &str) {
    let path = if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", prefix, key)
    };
    match value {
        Value::Object(inner) => {
            for (k, v) in inner {
                if !k.starts_with('_') {
                    collect_meta_leaves(fields, k, v, &path);
                }
            }
        }
        Value::Array(_) => {}
        leaf => {
            let field_type = match leaf {
                Value::Bool(_) => "bool",
                Value::Number(n) => {
                    if n.is_f64() {
                        "float"
                    } else {
                        "int"
                    }
                }
                Value::String(_) => "str",
                _ => "str",
            };
            fields.push(ModifiableField {
                category: "store".into(),
                field_id: format!("renpy_meta.{}", path),
                display_name: path.clone(),
                field_type: field_type.into(),
                save_value: leaf.clone(),
                ..Default::default()
            });
        }
    }
}

fn set_nested_meta(data: &mut Value, dotted_path: &str, value: &Value) {
    let parts: Vec<&str> = dotted_path.split('.').collect();

    // Ensure _meta and all intermediate nodes exist
    {
        let obj = data.as_object_mut().unwrap();
        let meta = obj
            .entry("_meta".to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let mut current = meta;
        for i in 0..parts.len() - 1 {
            let part = parts[i];
            if !current.is_object() {
                *current = Value::Object(serde_json::Map::new());
            }
            let cur_obj = current.as_object_mut().unwrap();
            current = cur_obj
                .entry(part.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
        }
    }

    // Set the final leaf value
    let json_ptr = format!("/_meta/{}", parts.join("/"));
    if let Some(target) = data.pointer_mut(&json_ptr) {
        *target = value.clone();
    } else if let Some(obj) = data.pointer_mut("/_meta") {
        if let Some(inner) = obj.as_object_mut() {
            insert_nested_value(inner, &parts, value);
        }
    }
}

fn insert_nested_value(obj: &mut serde_json::Map<String, Value>, parts: &[&str], value: &Value) {
    if parts.len() == 1 {
        obj.insert(parts[0].to_string(), value.clone());
    } else {
        let entry = obj
            .entry(parts[0].to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(inner) = entry.as_object_mut() {
            insert_nested_value(inner, &parts[1..], value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_data() -> Value {
        let mut meta = serde_json::Map::new();
        meta.insert("_save_name".into(), Value::String("Quick Save".into()));

        let mut data = serde_json::Map::new();
        data.insert("_format".into(), Value::String("renpy".into()));
        data.insert("_meta".into(), Value::Object(meta));
        data.insert("_extra_info".into(), Value::String("测试存档".into()));
        data.insert("_log".into(), Value::String("".into()));
        data.insert("_screenshot".into(), Value::String("".into()));
        data.insert("_renpy_version".into(), Value::String("8.0.3".into()));
        Value::Object(data)
    }

    #[test]
    fn test_extensions() {
        let fmt = RenPyFormat::new();
        assert!(fmt.extensions().contains(&".save".to_string()));
    }

    #[test]
    fn test_magic_bytes() {
        let fmt = RenPyFormat::new();
        assert_eq!(fmt.magic_bytes(), Some(b"PK\x03\x04".as_ref()));
    }

    #[test]
    fn test_get_summary() {
        let fmt = RenPyFormat::new();
        let summary = fmt.get_summary(&make_test_data());
        assert!(summary.members.contains(&"Quick Save".to_string()));
    }

    #[test]
    fn test_scan_fields() {
        let fmt = RenPyFormat::new();
        let fields = fmt.scan_fields(&make_test_data(), "");
        assert!(fields.iter().any(|f| f.field_id == "renpy_save_name"));
        assert!(fields.iter().any(|f| f.field_id == "renpy_extra_info"));
    }

    #[test]
    fn test_apply_field() {
        let fmt = RenPyFormat::new();
        let mut data = make_test_data();
        let field = ModifiableField {
            category: "meta".into(),
            field_id: "renpy_extra_info".into(),
            display_name: "Extra".into(),
            field_type: "str".into(),
            save_value: Value::String("Changed".into()),
            ..Default::default()
        };
        fmt.apply_field(&mut data, &field).unwrap();
        assert_eq!(data["_extra_info"], Value::String("Changed".into()));
    }

    #[test]
    fn test_find_data_dir() {
        let fmt = RenPyFormat::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("game/saves")).unwrap();
        let found = fmt.find_data_dir(&dir.path().to_string_lossy());
        assert!(found.is_some());
    }
}
