//! 存档完整性检查与修复工具模块。
//!
//! 提供存档文件格式检测、完整性校验、信息查看、批量扫描和修复功能。
//! 所有检测逻辑在 core 层独立实现，不依赖具体引擎 crate。

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::types::SaveSummary;

/// 完整性检查结果
#[derive(Debug, Clone)]
pub struct IntegrityResult {
    /// 存档文件的绝对路径
    pub file_path: String,
    /// 识别的格式名称，如 "RPG Maker MV/MZ"
    pub format_name: String,
    /// 引擎标识符，如 "rpg_mv"
    pub engine: String,
    /// 格式是否通过校验
    pub is_valid: bool,
    /// 文件大小（字节）
    pub file_size: u64,
    /// 文件修改时间（ISO 8601 格式）
    pub modified: String,
    /// 格式/逻辑错误列表
    pub errors: Vec<String>,
    /// 数据逻辑警告列表
    pub warnings: Vec<String>,
    /// 成功解析后的摘要信息
    pub summary: Option<SaveSummary>,
    /// 发现的字段数量
    pub field_count: usize,
}

/// 存档文件快速信息（轻量级，不深入解析）
#[derive(Debug, Clone)]
pub struct SaveFileInfo {
    /// 文件路径
    pub file_path: String,
    /// 格式名称
    pub format_name: String,
    /// 引擎标识符
    pub engine: String,
    /// 文件大小（字节）
    pub file_size: u64,
    /// 修改时间（ISO 8601）
    pub modified: String,
    /// 是否为有效存档
    pub is_valid: bool,
    /// 简要错误信息
    pub error: Option<String>,
}

/// 存档修复结果
#[derive(Debug, Clone)]
pub struct RepairResult {
    /// 修复是否成功
    pub success: bool,
    /// 修复后文件的路径
    pub repaired_path: Option<String>,
    /// 原始文件中的错误列表
    pub original_errors: Vec<String>,
    /// 修复后残留的错误
    pub remaining_errors: Vec<String>,
}

/// 检测到的存档格式枚举（文件级检测，非目录级）
#[derive(Debug, Clone, PartialEq)]
enum SaveFormat {
    /// RPG Maker MV/MZ (.rpgsave / .rmmzsave)
    RpgMaker { is_mz: bool },
    /// Ren'Py (.save)
    RenPy,
    /// Unreal Engine (.sav)
    Unreal,
    /// 通用 JSON (.json)
    Generic,
    /// 无法识别
    Unknown,
}

impl SaveFormat {
    fn from_path(filepath: &str) -> Self {
        let path = Path::new(filepath);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "rpgsave" => SaveFormat::RpgMaker { is_mz: false },
            "rmmzsave" => SaveFormat::RpgMaker { is_mz: true },
            "save" => {
                if let Ok(data) = fs::read(filepath) {
                    if data.len() >= 4 && &data[0..4] == b"GVAS" {
                        return SaveFormat::Unreal;
                    }
                    let cursor = std::io::Cursor::new(data.clone());
                    if zip::ZipArchive::new(cursor).is_ok() {
                        return SaveFormat::RenPy;
                    }
                }
                SaveFormat::RpgMaker { is_mz: false }
            }
            "sav" => SaveFormat::Unreal,
            "json" => SaveFormat::Generic,
            _ => SaveFormat::Unknown,
        }
    }

    fn name(&self) -> &str {
        match self {
            SaveFormat::RpgMaker { is_mz: true } => "RPG Maker MZ",
            SaveFormat::RpgMaker { is_mz: false } => "RPG Maker MV",
            SaveFormat::RenPy => "Ren'Py",
            SaveFormat::Unreal => "Unreal Engine (GVAS)",
            SaveFormat::Generic => "通用 JSON",
            SaveFormat::Unknown => "未知格式",
        }
    }

    fn engine(&self) -> &str {
        match self {
            SaveFormat::RpgMaker { is_mz: true } => "rpg_mz",
            SaveFormat::RpgMaker { is_mz: false } => "rpg_mv",
            SaveFormat::RenPy => "renpy",
            SaveFormat::Unreal => "unreal",
            SaveFormat::Generic => "generic",
            SaveFormat::Unknown => "unknown",
        }
    }
}

/// 获取存档文件的快速信息
pub fn get_save_info(filepath: &str) -> SaveFileInfo {
    let meta = fs::metadata(filepath);
    let file_size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified = meta
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| {
            let secs = d.as_secs();
            chrono::DateTime::from_timestamp(secs as i64, 0)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let format = SaveFormat::from_path(filepath);
    let (is_valid, error) = match &format {
        SaveFormat::Unknown => (false, Some("无法识别的存档格式".to_string())),
        _ => {
            if file_size == 0 {
                (false, Some("文件为空".to_string()))
            } else {
                (true, None)
            }
        }
    };

    SaveFileInfo {
        file_path: filepath.to_string(),
        format_name: format.name().to_string(),
        engine: format.engine().to_string(),
        file_size,
        modified,
        is_valid,
        error,
    }
}

/// 对单个存档文件执行完整性检查
pub fn check_save_integrity(filepath: &str) -> IntegrityResult {
    let info = get_save_info(filepath);
    let format = SaveFormat::from_path(filepath);
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut summary: Option<SaveSummary> = None;
    let mut field_count = 0;

    if let Some(ref e) = info.error {
        errors.push(e.clone());
    }

    if format == SaveFormat::Unknown {
        return IntegrityResult {
            file_path: info.file_path,
            format_name: info.format_name,
            engine: info.engine,
            is_valid: false,
            file_size: info.file_size,
            modified: info.modified,
            errors,
            warnings,
            summary: None,
            field_count: 0,
        };
    }

    let data = fs::read(filepath);
    match data {
        Err(e) => {
            errors.push(format!("读取文件失败: {}", e));
        }
        Ok(data) => {
            match &format {
                SaveFormat::RpgMaker { .. } => {
                    check_rpgmaker(&data, &mut errors, &mut warnings, &mut summary, &mut field_count);
                }
                SaveFormat::RenPy => {
                    check_renpy(&data, &mut errors, &mut warnings, &mut summary, &mut field_count);
                }
                SaveFormat::Unreal => {
                    check_unreal(&data, &mut errors, &mut warnings, &mut summary, &mut field_count);
                }
                SaveFormat::Generic => {
                    check_generic(&data, &mut errors, &mut warnings, &mut summary, &mut field_count);
                }
                SaveFormat::Unknown => {}
            }
        }
    }

    IntegrityResult {
        file_path: info.file_path,
        format_name: info.format_name,
        engine: info.engine,
        is_valid: errors.is_empty(),
        file_size: info.file_size,
        modified: info.modified,
        errors,
        warnings,
        summary,
        field_count,
    }
}

/// 校验 RPG Maker 存档（Base64 -> LZString -> JSON）
fn check_rpgmaker(
    data: &[u8],
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
    summary: &mut Option<SaveSummary>,
    field_count: &mut usize,
) {
    let text = match std::str::from_utf8(data) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            errors.push(format!("文件不是有效的 UTF-8 文本: {}", e));
            return;
        }
    };

    if text.is_empty() {
        errors.push("存档文件为空".to_string());
        return;
    }

    let json_str = match crate::lzstring::decompress_from_base64(&text) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("LZString 解压失败: {}", e));
            return;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("JSON 解析失败: {}", e));
            return;
        }
    };

    if !parsed.is_object() {
        errors.push("存档根元素不是 JSON 对象".to_string());
        return;
    }

    let obj = parsed.as_object().unwrap();
    *field_count = count_json_fields(&parsed, 0);

    match obj.get("party") {
        None => errors.push("缺少 party 字段".to_string()),
        Some(party) => {
            if let Some(gold) = party.get("_gold") {
                let g = gold.as_i64().unwrap_or(-1);
                if g < 0 {
                    errors.push(format!("金币为负数: {}", g));
                }
                if g > 99_999_999 {
                    warnings.push(format!("金币异常大: {}", g));
                }
                *summary = Some(SaveSummary {
                    gold: g as i32,
                    ..Default::default()
                });
            }
            if let Some(actors) = party.get("_actors").and_then(|v| v.as_array()) {
                for actor in actors {
                    if let (Some(hp), Some(mp), Some(lv)) = (
                        actor.get("_hp").and_then(|v| v.as_i64()),
                        actor.get("_mp").and_then(|v| v.as_i64()),
                        actor.get("_level").and_then(|v| v.as_i64()),
                    ) {
                        if hp < 0 || hp > 9999 {
                            warnings.push(format!("角色 HP 异常: {}", hp));
                        }
                        if mp < 0 || mp > 9999 {
                            warnings.push(format!("角色 MP 异常: {}", mp));
                        }
                        if lv < 1 || lv > 99 {
                            warnings.push(format!("角色等级异常: {}", lv));
                        }
                    }
                }
            }
        }
    }

    if let Some(switches) = obj.get("switches") {
        if !switches.is_array() {
            if switches.is_object() {
                let inner = switches.get("_data").unwrap_or(switches);
                if let Some(arr) = inner.get("@a").and_then(|v| v.as_object()) {
                    for v in arr.values() {
                        if v.is_number() && v.as_i64().map_or(true, |n| n != 0 && n != 1) {
                            warnings.push("开关值包含非 0/1 的数值".to_string());
                            break;
                        }
                    }
                }
            }
        } else if let Some(arr) = switches.as_array() {
            for v in arr {
                if !v.is_boolean() && !v.is_number() {
                    errors.push(format!("开关值类型错误: {:?}", v));
                    break;
                }
            }
        }
    } else {
        warnings.push("缺少 switches 字段".to_string());
    }

    if let Some(variables) = obj.get("variables") {
        if !variables.is_array() {
            if variables.is_object() {
                let inner = variables.get("_data").unwrap_or(variables);
                if let Some(arr) = inner.get("@a").and_then(|v| v.as_object()) {
                    for v in arr.values() {
                        if !v.is_number() {
                            warnings.push("变量值包含非数值类型".to_string());
                            break;
                        }
                    }
                }
            }
        } else if let Some(arr) = variables.as_array() {
            for v in arr {
                if !v.is_number() {
                    errors.push(format!("变量值类型错误: {:?}", v));
                    break;
                }
            }
        }
    }
}

/// 校验 Ren'Py 存档（ZIP 格式）
fn check_renpy(
    data: &[u8],
    errors: &mut Vec<String>,
    _warnings: &mut Vec<String>,
    _summary: &mut Option<SaveSummary>,
    _field_count: &mut usize,
) {
    let cursor = std::io::Cursor::new(data);
    match zip::ZipArchive::new(cursor) {
        Ok(archive) => {
            let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
            if names.is_empty() {
                errors.push("ZIP 归档内没有文件".to_string());
            }
        }
        Err(e) => {
            errors.push(format!("ZIP 解析失败: {}", e));
        }
    }
}

/// 校验 Unreal GVAS 存档
fn check_unreal(
    data: &[u8],
    errors: &mut Vec<String>,
    _warnings: &mut Vec<String>,
    _summary: &mut Option<SaveSummary>,
    field_count: &mut usize,
) {
    const MAGIC: &[u8] = b"GVAS";
    if data.len() < 4 {
        errors.push("文件太小，不是有效的 GVAS 格式".to_string());
        return;
    }
    if &data[0..4] != MAGIC {
        errors.push(format!(
            "魔术字节不匹配: 期望 'GVAS', 实际 '{:?}'",
            String::from_utf8_lossy(&data[0..4])
        ));
        return;
    }
    if data.len() >= 46 {
        let sv = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if sv < 0 {
            errors.push(format!("存档版本异常: {}", sv));
        }
    }
    let mut offset = 46;
    let mut prop_count = 0;
    while offset + 4 < data.len() {
        let name_end = data[offset..].iter().position(|&b| b == 0);
        match name_end {
            Some(len) if len > 0 => {
                offset += len + 1;
            }
            _ => break,
        }
        if offset >= data.len() {
            break;
        }
        match data[offset] {
            0x02 => {
                offset += 9;
                prop_count += 1;
            }
            0x03 => {
                offset += 5;
                prop_count += 1;
            }
            0x04 => {
                if offset + 5 > data.len() {
                    break;
                }
                let len = i32::from_le_bytes(
                    [data[offset + 1], data[offset + 2], data[offset + 3], data[offset + 4]],
                ) as usize;
                offset += 5 + len.max(1);
                prop_count += 1;
            }
            0x08 => {
                offset += 2;
                prop_count += 1;
            }
            _ => break,
        }
    }
    *field_count = prop_count;
}

/// 校验通用 JSON 存档
fn check_generic(
    data: &[u8],
    errors: &mut Vec<String>,
    _warnings: &mut Vec<String>,
    _summary: &mut Option<SaveSummary>,
    field_count: &mut usize,
) {
    let text = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("文件不是有效的 UTF-8 文本: {}", e));
            return;
        }
    };
    let parsed: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("JSON 解析失败: {}", e));
            return;
        }
    };
    *field_count = count_json_fields(&parsed, 0);
}

/// 递归计算 JSON 对象/数组中叶子字段的个数
fn count_json_fields(value: &serde_json::Value, depth: usize) -> usize {
    if depth > 20 {
        return 0;
    }
    match value {
        serde_json::Value::Object(obj) => {
            obj.values().map(|v| count_json_fields(v, depth + 1)).sum()
        }
        serde_json::Value::Array(arr) => {
            arr.iter().map(|v| count_json_fields(v, depth + 1)).sum()
        }
        _ => 1,
    }
}

/// 对指定目录中的所有存档文件执行批量完整性检查
pub fn batch_check_saves(dir: &str) -> Vec<IntegrityResult> {
    let mut results = Vec::new();
    let path = Path::new(dir);
    if !path.is_dir() {
        return results;
    }

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                continue;
            }
            let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if fname.contains(".bak.") || fname == "config.rpgsave" || fname == "global.rpgsave" {
                continue;
            }
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            let supported = matches!(ext, "rpgsave" | "rmmzsave" | "save" | "sav" | "json");
            if supported {
                results.push(check_save_integrity(&p.to_string_lossy()));
            }
        }
    }
    results
}

/// 尝试修复损坏的 RPG Maker 存档
pub fn attempt_repair(filepath: &str) -> RepairResult {
    let data = match fs::read(filepath) {
        Ok(d) => d,
        Err(e) => {
            return RepairResult {
                success: false,
                repaired_path: None,
                original_errors: vec![format!("读取文件失败: {}", e)],
                remaining_errors: vec![],
            };
        }
    };

    let text = String::from_utf8_lossy(&data).trim().to_string();
    let mut original_errors = Vec::new();

    // 策略1: 补齐 Base64 padding
    let fixed_text = repair_base64_padding(&text);
    if fixed_text != text {
        if let Ok(json_str) = crate::lzstring::decompress_from_base64(&fixed_text) {
            if let Ok(_parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                return write_repaired(filepath, &fixed_text);
            }
        }
    }

    // 策略2: 移除非法 Base64 字符后重试
    let cleaned = clean_base64(&text);
    if cleaned != text {
        if let Ok(json_str) = crate::lzstring::decompress_from_base64(&cleaned) {
            if let Ok(_parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                return write_repaired(filepath, &cleaned);
            }
        }
    }

    // 策略3: 解压后修复不完整 JSON
    if let Ok(mut json_str) = crate::lzstring::decompress_from_base64(&text) {
        let fixed_json = repair_json(&json_str);
        if fixed_json != json_str {
            json_str = fixed_json;
        }
        if serde_json::from_str::<serde_json::Value>(&json_str).is_ok() {
            match crate::lzstring::compress_to_base64(&json_str) {
                Ok(compressed) => return write_repaired(filepath, &compressed),
                Err(_) => {}
            }
        }
    }

    original_errors.push("所有修复策略均失败".to_string());
    RepairResult {
        success: false,
        repaired_path: None,
        original_errors,
        remaining_errors: vec![],
    }
}

/// 补齐缺失的 Base64 padding
fn repair_base64_padding(text: &str) -> String {
    let trimmed = text.trim_end_matches('=');
    let missing = (4 - (trimmed.len() % 4)) % 4;
    let mut fixed = trimmed.to_string();
    for _ in 0..missing {
        fixed.push('=');
    }
    fixed
}

/// 移除字符串中的非 Base64 字符
fn clean_base64(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '=')
        .collect()
}

/// 尝试修复不完整的 JSON 字符串
fn repair_json(json_str: &str) -> String {
    let mut fixed = json_str.to_string();
    let open_braces = fixed.matches('{').count().saturating_sub(fixed.matches('}').count());
    let open_brackets = fixed.matches('[').count().saturating_sub(fixed.matches(']').count());
    for _ in 0..open_brackets {
        fixed.push(']');
    }
    for _ in 0..open_braces {
        fixed.push('}');
    }
    fixed
}

/// 将修复后的数据写入 `{原名}_repaired.{ext}` 文件
fn write_repaired(original_path: &str, content: &str) -> RepairResult {
    let path = Path::new(original_path);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let parent = path.parent().unwrap_or(Path::new("."));
    let repaired_name = if ext.is_empty() {
        format!("{}_repaired", stem)
    } else {
        format!("{}_repaired.{}", stem, ext)
    };
    let repaired_path = parent.join(&repaired_name);
    match fs::write(&repaired_path, content) {
        Ok(()) => RepairResult {
            success: true,
            repaired_path: Some(repaired_path.to_string_lossy().to_string()),
            original_errors: vec![],
            remaining_errors: vec![],
        },
        Err(e) => RepairResult {
            success: false,
            repaired_path: None,
            original_errors: vec![],
            remaining_errors: vec![format!("写入修复文件失败: {}", e)],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── SaveFileInfo 测试 ──

    #[test]
    fn test_get_save_info_unknown_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.xyz");
        fs::write(&path, b"hello").unwrap();
        let info = get_save_info(&path.to_string_lossy());
        assert_eq!(info.format_name, "未知格式");
        assert!(!info.is_valid);
        assert!(info.error.is_some());
    }

    #[test]
    fn test_get_save_info_rpgsave() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("save.rpgsave");
        fs::write(&path, b"AA").unwrap();
        let info = get_save_info(&path.to_string_lossy());
        assert!(info.format_name.contains("RPG Maker"));
        assert_eq!(info.engine, "rpg_mv");
    }

    // ── Integrity 测试 ──

    #[test]
    fn test_check_rpgmaker_valid() {
        let dir = tempfile::tempdir().unwrap();
        let save = serde_json::json!({
            "party": {"_gold": 1000, "_actors": [{"_actorId":1,"_hp":100,"_mp":50,"_level":5}]},
            "switches": [false, true, false],
            "variables": [0, 42, 0]
        });
        let compressed = crate::lzstring::compress_to_base64(&save.to_string()).unwrap();
        let path = dir.path().join("save.rpgsave");
        fs::write(&path, &compressed).unwrap();
        let result = check_save_integrity(&path.to_string_lossy());
        assert!(result.is_valid, "应通过校验: {:?}", result.errors);
        assert_eq!(result.summary.as_ref().map(|s| s.gold), Some(1000));
    }

    #[test]
    fn test_check_rpgmaker_negative_gold() {
        let dir = tempfile::tempdir().unwrap();
        let save = serde_json::json!({"party": {"_gold": -500}});
        let compressed = crate::lzstring::compress_to_base64(&save.to_string()).unwrap();
        let path = dir.path().join("save.rpgsave");
        fs::write(&path, &compressed).unwrap();
        let result = check_save_integrity(&path.to_string_lossy());
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("负数")));
    }

    #[test]
    fn test_check_unreal_valid() {
        let dir = tempfile::tempdir().unwrap();
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
        data.extend_from_slice(&0u32.to_le_bytes());
        let path = dir.path().join("test.sav");
        fs::write(&path, &data).unwrap();
        let result = check_save_integrity(&path.to_string_lossy());
        assert!(result.is_valid);
        assert_eq!(result.format_name, "Unreal Engine (GVAS)");
    }

    #[test]
    fn test_check_invalid_gvas_magic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.sav");
        fs::write(&path, b"XXXX").unwrap();
        let result = check_save_integrity(&path.to_string_lossy());
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("魔术")));
    }

    // ── Repair 测试 ──

    #[test]
    fn test_repair_base64_padding() {
        assert_eq!(repair_base64_padding("abc"), "abc=");
        assert_eq!(repair_base64_padding("abcd"), "abcd");
        assert_eq!(repair_base64_padding("ab"), "ab==");
    }

    #[test]
    fn test_clean_base64() {
        assert_eq!(clean_base64("abc^def"), "abcdef");
        assert_eq!(clean_base64("a+b/c=d"), "a+b/c=d");
    }

    #[test]
    fn test_repair_json_unclosed() {
        let fixed = repair_json(r#"{"a":1"#);
        assert_eq!(fixed, r#"{"a":1}"#);
        let fixed2 = repair_json(r#"[1,2"#);
        assert_eq!(fixed2, r#"[1,2]"#);
    }

    #[test]
    fn test_attempt_repair_nonexistent_file() {
        let result = attempt_repair("nonexistent_file.rpgsave");
        assert!(!result.success);
        assert!(result.original_errors.iter().any(|e| e.contains("读取文件失败")));
    }

    // ── Batch 测试 ──

    #[test]
    fn test_batch_check_saves_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let results = batch_check_saves(&dir.path().to_string_lossy());
        assert!(results.is_empty());
    }
}
