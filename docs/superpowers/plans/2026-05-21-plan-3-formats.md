# Plan 3: Ren'Py + Unreal + Generic Format Handlers

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement SaveFormat trait for Ren'Py (.save ZIP), Unreal (.sav GVAS), and Generic JSON (.json flatten) format handlers.

**Dependency:** Plan 1 (core traits) + Plan 2 (RpgMakerFormat as reference pattern).

---

## File Map

| Action | Path |
|--------|------|
| CREATE | `crates/engines/renpy/src/format.rs` |
| CREATE | `crates/engines/unreal/src/format.rs` |
| CREATE | `crates/engines/generic/src/format.rs` |
| MODIFY | `crates/engines/renpy/src/lib.rs` |
| MODIFY | `crates/engines/unreal/src/lib.rs` |
| MODIFY | `crates/engines/generic/src/lib.rs` |

---

### Task 1: Ren'Py Format Handler

- [ ] **Step 1: Update `crates/engines/renpy/src/lib.rs`**

```rust
// game-tool-renpy: Ren'Py 引擎支持

pub mod format;
// pub mod bridge;    -- Plan 4
// pub mod injector;  -- Plan 4
```

- [ ] **Step 2: Write `crates/engines/renpy/src/format.rs`**

```rust
//! Ren'Py 存档格式处理器 (.save ZIP)

use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use serde_json::Value;
use zip::read::ZipArchive;
use zip::write::{SimpleFileOptions, ZipWriter};
use game_tool_core::{
    ISaveFormat, ModifiableField, SaveSummary,
    error::GameToolError,
    backup,
};

pub struct RenPyFormat;

impl RenPyFormat {
    pub fn new() -> Self { Self }
}

impl ISaveFormat for RenPyFormat {
    fn name(&self) -> &str { "Ren'Py" }
    fn extensions(&self) -> Vec<String> { vec![".save".into()] }
    fn engine_type(&self) -> &str { "renpy" }
    fn magic_bytes(&self) -> Option<&[u8]> { Some(b"PK\x03\x04") }

    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        let file = fs::File::open(filepath)
            .map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| GameToolError::ArchiveLoadError(format!("ZIP 打开失败: {}", e)))?;

        let mut meta = Value::Null;
        let mut extra_info = String::new();
        let mut log_bytes = Vec::new();
        let mut screenshot = Vec::new();
        let mut renpy_version = String::new();

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)
                .map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
            let name = entry.name().to_string();
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok();

            match name.as_str() {
                "json" => {
                    meta = serde_json::from_slice(&buf)
                        .map_err(|e| GameToolError::ArchiveLoadError(format!("JSON 解析: {}", e)))?;
                }
                "extra_info" => extra_info = String::from_utf8_lossy(&buf).to_string(),
                "log" => log_bytes = buf,
                "screenshot.png" => screenshot = buf,
                "renpy_version" => renpy_version = String::from_utf8_lossy(&buf).trim().to_string(),
                _ => {}
            }
        }

        if meta.is_null() {
            return Err(GameToolError::ArchiveLoadError("ZIP 中缺少 json 条目".into()));
        }

        let mut data = serde_json::Map::new();
        data.insert("_format".into(), Value::String("renpy".into()));
        data.insert("_meta".into(), meta);
        data.insert("_extra_info".into(), Value::String(extra_info));
        data.insert("_log".into(), Value::String(base64_encode(&log_bytes)));
        data.insert("_screenshot".into(), Value::String(base64_encode(&screenshot)));
        data.insert("_renpy_version".into(), Value::String(renpy_version));

        Ok(Value::Object(data))
    }

    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);

        let meta = data.get("_meta").cloned().unwrap_or(Value::Null);
        let extra_info = data.get("_extra_info").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let log_b64 = data.get("_log").and_then(|v| v.as_str()).unwrap_or("");
        let screenshot_b64 = data.get("_screenshot").and_then(|v| v.as_str()).unwrap_or("");
        let renpy_version = data.get("_renpy_version").and_then(|v| v.as_str()).unwrap_or("");

        let mut buf = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut buf);
            let options = SimpleFileOptions::default();

            writer.start_file("json", options).map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
            serde_json::to_writer(&mut writer, &meta)
                .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;

            writer.start_file("extra_info", options).map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
            std::io::Write::write_all(&mut writer, extra_info.as_bytes()).ok();

            if !log_b64.is_empty() {
                if let Some(decoded) = base64_decode(log_b64) {
                    writer.start_file("log", options).map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
                    std::io::Write::write_all(&mut writer, &decoded).ok();
                }
            }
            if !screenshot_b64.is_empty() {
                if let Some(decoded) = base64_decode(screenshot_b64) {
                    writer.start_file("screenshot.png", options).map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
                    std::io::Write::write_all(&mut writer, &decoded).ok();
                }
            }
            if !renpy_version.is_empty() {
                writer.start_file("renpy_version", options).map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
                std::io::Write::write_all(&mut writer, renpy_version.as_bytes()).ok();
            }

            writer.finish().map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        }

        fs::write(path, buf.into_inner())
            .map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        Ok(())
    }

    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["game/saves", "game/save", "saves"] {
            let d = dir.join(sub);
            if d.is_dir() { return Some(d.to_string_lossy().to_string()); }
        }
        None
    }

    fn get_summary(&self, data: &Value) -> SaveSummary {
        let meta = data.get("_meta");
        let save_name = meta.and_then(|m| m.get("_save_name")).and_then(|v| v.as_str()).unwrap_or("");
        let version = data.get("_renpy_version").and_then(|v| v.as_str()).unwrap_or("");
        let has_screenshot = data.get("_screenshot").and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false);

        SaveSummary {
            gold: 0,
            party_size: 0,
            item_count: 0,
            save_count: 0,
            play_time: 0,
            members: if save_name.is_empty() { vec![] } else { vec![save_name.to_string()] },
            extra: {
                let mut m = HashMap::new();
                m.insert("version".into(), Value::String(version.to_string()));
                m.insert("has_screenshot".into(), Value::Bool(has_screenshot));
                m
            },
        }
    }

    fn scan_fields(&self, data: &Value, _game_dir: &str) -> Vec<ModifiableField> {
        let mut fields = Vec::new();
        let extra_info = data.get("_extra_info").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let save_name = data.get("_meta").and_then(|m| m.get("_save_name")).and_then(|v| v.as_str()).unwrap_or("");
        let version = data.get("_renpy_version").and_then(|v| v.as_str()).unwrap_or("");

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
            _ => {}
        }
        Ok(())
    }
}

fn base64_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b = |i| chunk.get(i).copied().unwrap_or(0) as u32;
        let n = (b(0) << 16) | (b(1) << 8) | b(2);
        let pad = if chunk.len() < 3 { 3 - chunk.len() } else { 0 };
        result.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        result.push(if chunk.len() > 1 { CHARS[((n >> 6) & 0x3F) as usize] } else { b'=' } as char);
        result.push(if chunk.len() > 2 { CHARS[(n & 0x3F) as usize] } else { b'=' } as char);
    }
    result
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let input = input.trim_end_matches('=');
    let mut result = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for c in input.chars() {
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_test_data() -> Value {
        let mut meta = serde_json::Map::new();
        meta.insert("_save_name".into(), Value::String("Quick Save".into()));
        meta.insert("_renpy_version".into(), Value::String("8.0.3".into()));

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
        let data = make_test_data();
        let summary = fmt.get_summary(&data);
        assert!(summary.members.contains(&"Quick Save".to_string()));
    }

    #[test]
    fn test_scan_fields() {
        let fmt = RenPyFormat::new();
        let data = make_test_data();
        let fields = fmt.scan_fields(&data, "");
        assert!(fields.iter().any(|f| f.field_id == "renpy_save_name"));
        assert!(fields.iter().any(|f| f.field_id == "renpy_extra_info"));
    }

    #[test]
    fn test_apply_field_extra_info() {
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
        assert!(found.unwrap().ends_with("game/saves"));
    }
}
```

- [ ] **Step 3: Verify**

```powershell
cargo test -p game-tool-renpy
```

Expected: 6 tests pass.

---

### Task 2: Unreal GVAS Format Handler

- [ ] **Step 1: Update `crates/engines/unreal/src/lib.rs`**

```rust
// game-tool-unreal: Unreal Engine 支持

pub mod format;
// pub mod bridge;  -- Plan 4
```

- [ ] **Step 2: Write `crates/engines/unreal/src/format.rs`**

```rust
//! Unreal Engine GVAS 存档格式处理器

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::Value;
use game_tool_core::{
    ISaveFormat, ModifiableField, SaveSummary,
    error::GameToolError,
    backup,
};

const MAGIC: &[u8] = b"GVAS";

pub struct UnrealGVASFormat;

impl UnrealGVASFormat {
    pub fn new() -> Self { Self }
}

impl ISaveFormat for UnrealGVASFormat {
    fn name(&self) -> &str { "Unreal Engine (GVAS)" }
    fn extensions(&self) -> Vec<String> { vec![".sav".into()] }
    fn engine_type(&self) -> &str { "unreal" }
    fn magic_bytes(&self) -> Option<&[u8]> { Some(MAGIC) }

    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        let raw = fs::read(filepath)
            .map_err(|e| GameToolError::ArchiveLoadError(e.to_string()))?;
        if raw.len() < 4 || &raw[0..4] != MAGIC {
            return Err(GameToolError::ArchiveLoadError("无效的 GVAS 格式".into()));
        }

        let header = Self::parse_header(&raw);
        let props = Self::extract_properties(&raw, header.get("_data_offset").and_then(|v| v.as_u64()).unwrap_or(36) as usize);

        let mut data = serde_json::Map::new();
        data.insert("_format".into(), Value::String("unreal_gvas".into()));
        data.insert("_raw".into(), Value::String(base64_encode_simple(&raw)));
        data.insert("_header".into(), Value::Object(header));
        data.insert("_props".into(), Value::Object(props));

        Ok(Value::Object(data))
    }

    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        let path = Path::new(filepath);
        let _ = backup::save_backup(path, 10);
        let raw_b64 = data.get("_raw").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(raw) = base64_decode_simple(raw_b64) {
            fs::write(path, &raw).map_err(|e| GameToolError::ArchiveSaveError(e.to_string()))?;
        }
        Ok(())
    }

    fn find_data_dir(&self, game_dir: &str) -> Option<String> {
        let dir = Path::new(game_dir);
        for sub in &["Saved/SaveGames", "Saved", "SaveGames"] {
            let d = dir.join(sub);
            if d.is_dir() { return Some(d.to_string_lossy().to_string()); }
        }
        None
    }

    fn get_summary(&self, data: &Value) -> SaveSummary {
        let props = data.get("_props");
        let gold = props.and_then(|p| p.get("Gold")).or_else(|| props.and_then(|p| p.get("Money")))
            .and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let play_time = props.and_then(|p| p.get("PlayTime")).or_else(|| props.and_then(|p| p.get("RealTimeSeconds")))
            .and_then(|v| v.as_f64()).unwrap_or(0.0) as i32;
        let prop_count = props.and_then(|p| p.as_object()).map(|m| m.len() as i32).unwrap_or(0);

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
                let display_name = KNOWN_NAMES.get(key.as_str()).cloned().unwrap_or_else(|| key.clone());
                fields.push(ModifiableField {
                    category: "gvas".into(),
                    field_id: format!("gvas_{}", key),
                    display_name,
                    field_type: field_type.into(),
                    save_value: value.clone(),
                    ..Default::default()
                });
            }
        }
        fields
    }

    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError> {
        let key = field.field_id.strip_prefix("gvas_").unwrap_or(&field.field_id).to_string();
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
        if raw.len() < 36 { return header; }

        let save_game_version = i32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
        let package_version = i32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]);
        let engine_major = u16::from_le_bytes([raw[12], raw[13]]);
        let engine_minor = u16::from_le_bytes([raw[14], raw[15]]);
        let engine_patch = u16::from_le_bytes([raw[16], raw[17]]);
        let engine_build = u32::from_le_bytes([raw[18], raw[19], raw[20], raw[21]]);
        let branch = Self::read_cstring(&raw[22..38]);
        let custom_format_version = i32::from_le_bytes([raw[38], raw[39], raw[40], raw[41]]);
        let custom_count = i32::from_le_bytes([raw[42], raw[43], raw[44], raw[45]]) as usize;

        let mut offset = 46;
        let mut custom_versions = Vec::new();
        for _ in 0..custom_count.min(64) {
            if offset + 16 > raw.len() { break; }
            let guid = format!("{:02x?}", &raw[offset..offset+16]);
            custom_versions.push(Value::String(guid));
            offset += 16;
        }
        let save_game_type_len = i32::from_le_bytes([raw[offset], raw[offset+1], raw[offset+2], raw[offset+3]]) as usize;
        offset += 4;
        let save_game_type = if offset + save_game_type_len <= raw.len() {
            String::from_utf8_lossy(&raw[offset..offset+save_game_type_len]).trim_end_matches('\0').to_string()
        } else { String::new() };
        offset += save_game_type_len;

        header.insert("_saveGameVersion".into(), Value::Number(save_game_version.into()));
        header.insert("_packageVersion".into(), Value::Number(package_version.into()));
        header.insert("_engineMajor".into(), Value::Number(engine_major.into()));
        header.insert("_engineMinor".into(), Value::Number(engine_minor.into()));
        header.insert("_enginePatch".into(), Value::Number(engine_patch.into()));
        header.insert("_engineBuild".into(), Value::Number(engine_build.into()));
        header.insert("_branch".into(), Value::String(branch));
        header.insert("_customFormatVersion".into(), Value::Number(custom_format_version.into()));
        header.insert("_saveGameType".into(), Value::String(save_game_type));
        header.insert("_data_offset".into(), Value::Number((offset as u64).into()));

        header
    }

    fn extract_properties(raw: &[u8], _start_offset: usize) -> serde_json::Map<String, Value> {
        let mut props = serde_json::Map::new();
        let mut offset = _start_offset.min(raw.len());
        while offset + 4 < raw.len() {
            let name_end = raw[offset..].iter().position(|&b| b == 0);
            let name = match name_end {
                Some(len) if len > 0 => String::from_utf8_lossy(&raw[offset..offset+len]).to_string(),
                _ => break,
            };
            offset += name.len() + 1;
            if offset >= raw.len() { break; }

            match raw[offset] {
                0x02 => { // IntProperty
                    if offset + 9 <= raw.len() {
                        let val = i64::from_le_bytes(raw[offset+1..offset+9].try_into().unwrap_or([0; 8]));
                        props.insert(name, Value::Number(val.into()));
                        offset += 9;
                    }
                }
                0x03 => { // FloatProperty
                    if offset + 5 <= raw.len() {
                        let val = f32::from_le_bytes(raw[offset+1..offset+5].try_into().unwrap_or([0; 4]));
                        props.insert(name, Value::Number(serde_json::Number::from_f64(val as f64).unwrap_or(0.into())));
                        offset += 5;
                    }
                }
                0x04 => { // StrProperty
                    if offset + 5 <= raw.len() {
                        let len = i32::from_le_bytes(raw[offset+1..offset+5].try_into().unwrap_or([0; 4])) as usize;
                        offset += 5;
                        if offset + len <= raw.len() {
                            let s = String::from_utf8_lossy(&raw[offset..offset+len]).trim_end_matches('\0').to_string();
                            props.insert(name, Value::String(s));
                            offset += len;
                        }
                    }
                }
                0x08 => { // BoolProperty
                    if offset + 2 <= raw.len() {
                        let val = raw[offset+1] != 0;
                        props.insert(name, Value::Bool(val));
                        offset += 2;
                    }
                }
                _ => break,
            }
        }
        props
    }

    fn read_cstring(data: &[u8]) -> String {
        let null_pos = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        String::from_utf8_lossy(&data[..null_pos]).trim_end_matches('\0').to_string()
    }
}

static KNOWN_NAMES: once_cell::sync::Lazy<HashMap<&str, &str>> = once_cell::sync::Lazy::new(|| {
    HashMap::from([
        ("Gold", "金币"), ("Money", "金钱"), ("Health", "生命值"), ("HP", "HP"),
        ("MaxHealth", "最大生命"), ("Level", "等级"), ("Experience", "经验值"),
        ("PlayTime", "游戏时间"), ("RealTimeSeconds", "实时秒数"),
        ("PlayerName", "玩家名"), ("SaveSlotName", "存档槽名"),
    ])
});

fn base64_encode_simple(data: &[u8]) -> String {
    use std::fmt::Write;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b = |i| chunk.get(i).copied().unwrap_or(0) as u32;
        let n = (b(0) << 16) | (b(1) << 8) | b(2);
        result.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        result.push(if chunk.len() > 1 { CHARS[((n >> 6) & 0x3F) as usize] } else { b'=' } as char);
        result.push(if chunk.len() > 2 { CHARS[(n & 0x3F) as usize] } else { b'=' } as char);
    }
    result
}

fn base64_decode_simple(input: &str) -> Option<Vec<u8>> {
    let input = input.trim_end_matches('=');
    let mut result = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for c in input.chars() {
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 { bits -= 8; result.push((buf >> bits) as u8); buf &= (1 << bits) - 1; }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_gvas() -> Vec<u8> {
        let mut data = vec![b'G', b'V', b'A', b'S'];
        // SaveGameVersion (int32)
        data.extend_from_slice(&0u32.to_le_bytes());
        // PackageVersion (int32)
        data.extend_from_slice(&0u32.to_le_bytes());
        // EngineVersion: major(2) minor(2) patch(2) build(4)
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        // Branch (16 bytes)
        data.extend_from_slice(b"++UE5+Release\0\0\0");
        // CustomFormatVersion (int32)
        data.extend_from_slice(&0u32.to_le_bytes());
        // CustomFormatData count (int32) = 0
        data.extend_from_slice(&0u32.to_le_bytes());
        // SaveGameType length (int32) = 15
        data.extend_from_slice(&(15u32).to_le_bytes());
        // SaveGameType string
        data.extend_from_slice(b"/Script/SaveGame\0");
        // No properties follow
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
        assert!(data.get("_props").is_some());
    }

    #[test]
    fn test_find_data_dir() {
        let fmt = UnrealGVASFormat::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Saved/SaveGames")).unwrap();
        let found = fmt.find_data_dir(&dir.path().to_string_lossy());
        assert!(found.is_some());
    }
}
```

- [ ] **Step 3: Add `once_cell` to unreal Cargo.toml**

```toml
once_cell = "1"
```

- [ ] **Step 4: Verify**

```powershell
cargo test -p game-tool-unreal
```

---

### Task 3: Generic JSON Format Handler

- [ ] **Step 1: Update `crates/engines/generic/src/lib.rs`**

```rust
// game-tool-generic: 通用/Unity/Godot 引擎支持

pub mod format;
// pub mod bridge;  -- Plan 4
```

- [ ] **Step 2: Write `crates/engines/generic/src/format.rs`**

```rust
//! 通用 JSON 存档格式处理器
//! 适应任何 JSON 存档：嵌套结构扁平化 → 编辑 → 重建嵌套

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
        let flat = data.get("_flat").and_then(|v| v.as_object()).cloned().unwrap_or_default();
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
                let display_name = FIELD_NAME_MAP.get(key.as_str()).cloned().unwrap_or_else(|| key.clone());
                fields.push(ModifiableField {
                    category,
                    field_id: format!("json_{}", key),
                    display_name,
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
                let new_prefix = if prefix.is_empty() { key.clone() } else { format!("{}.{}", prefix, key) };
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
        let rest = if bracket_end + 1 < path.len() { &path[bracket_end+1..] } else { "" };
        if let Some(arr) = node.as_array_mut() {
            while arr.len() <= idx { arr.push(Value::Null); }
            if rest.is_empty() || rest == "." {
                arr[idx] = value;
            } else {
                let rest = rest.strip_prefix('.').unwrap_or(rest);
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
    if lower.contains("gold") || lower.contains("money") || lower.contains("金币") { return "gold".into(); }
    if lower.contains("hp") || lower.contains("health") { return "actor".into(); }
    "general".into()
}

use once_cell::sync::Lazy;
static FIELD_NAME_MAP: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
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
    fn test_flatten_roundtrip() {
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

        let original = r#"{"player":{"hp":100,"mp":50}}"#;
        std::fs::write(&save_path, original).unwrap();
        let data = fmt.load(&path_str).unwrap();
        fmt.save(&path_str, &data).unwrap();

        let loaded = fmt.load(&path_str).unwrap();
        assert_eq!(loaded["_flat"]["player.hp"], json!(100));
    }
}
```

- [ ] **Step 3: Add `once_cell` to generic Cargo.toml**

```toml
once_cell = "1"
```

- [ ] **Step 4: Verify**

```powershell
cargo test -p game-tool-generic
```

Expected: 5 tests pass.

---

### Task 4: Final Verification

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: all format tests pass, clippy clean.

```bash
git add -A && git commit -m "feat: implement Ren'Py, Unreal GVAS, Generic JSON format handlers"
```
