//! LZ-String 压缩/解压（封装 lz-str crate）
//!
//! 与 JS 版 lz-string 1.4.4 兼容，用于 RPG Maker MV 存档解析。
//! 底层使用 `lz_str` crate（Rust 端 lz-string 移植）。

use thiserror::Error;

/// LZ-String 操作错误
#[derive(Error, Debug)]
pub enum LzStringError {
    /// 解压失败：数据格式无效或损坏
    #[error("LZ-String 解压失败：数据无效或损坏")]
    DecompressFailed,

    /// UTF-16 解码失败：解压结果不是有效的 UTF-16
    #[error("UTF-16 解码失败：{0}")]
    Utf16DecodeError(#[from] std::string::FromUtf16Error),
}

/// 从 Base64 编码的 LZ-String 解压为原始 JSON 字符串
///
/// # 参数
/// - `input`: Base64 编码的 lz-string 压缩数据
///
/// # 返回
/// - `Ok(String)`: 解压后的原始字符串
/// - `Err(LzStringError)`: 解压失败
pub fn decompress_from_base64(input: &str) -> Result<String, LzStringError> {
    if input.is_empty() {
        return Ok(String::new());
    }

    let decoded_u16 =
        lz_str::decompress_from_base64(input).ok_or(LzStringError::DecompressFailed)?;

    // Vec<u16> → String（lz-string 内部使用 UTF-16 编码）
    let result = String::from_utf16(&decoded_u16)?;
    Ok(result)
}

/// 将字符串压缩为 Base64 编码的 LZ-String 格式
///
/// # 参数
/// - `input`: 待压缩的原始字符串（通常是 JSON）
///
/// # 返回
/// - `Ok(String)`: Base64 编码的压缩数据
/// - `Err(LzStringError)`: 压缩失败（当前不会发生，但保留 Result 签名以便向后兼容）
pub fn compress_to_base64(input: &str) -> Result<String, LzStringError> {
    if input.is_empty() {
        // lz_str::compress_to_base64("") 返回空字符串，但 Python lzstring 返回 "Q==="
        // 为了与 Python/JS 行为一致，我们也返回空字符串
        // 注意：decompress_from_base64("") 也会返回 Ok("")，形成往返一致性
        return Ok(String::new());
    }

    let mut compressed = lz_str::compress_to_base64(input);
    // lz_str crate 的 base64 编码比 Python lzstring 多一个尾随 '='
    // 去除以匹配 Python golden 文件格式，保证字节级兼容性
    if compressed.ends_with('=') {
        compressed.pop();
    }
    Ok(compressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── 基本功能 ─────────────────────────────────────────

    #[test]
    fn test_roundtrip_simple() {
        let input = r#"{"id":1,"value":42,"str":"hello"}"#;
        let compressed = compress_to_base64(input).unwrap();
        let decompressed = decompress_from_base64(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_roundtrip_unicode() {
        let input = "你好世界! こんにちは! 🔥测试";
        let compressed = compress_to_base64(input).unwrap();
        let decompressed = decompress_from_base64(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_roundtrip_special_chars() {
        let input = "+/==\n\r\t\0$@!#%^&*()";
        let compressed = compress_to_base64(input).unwrap();
        let decompressed = decompress_from_base64(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }

    // ─── 边界测试 ─────────────────────────────────────────

    #[test]
    fn test_empty_string() {
        // 空字符串压缩 → 解压往返
        let compressed = compress_to_base64("").unwrap();
        let decompressed = decompress_from_base64(&compressed).unwrap();
        assert_eq!(decompressed, "");
    }

    #[test]
    fn test_decompress_empty_input() {
        // 对空字符串解压应返回空字符串
        let result = decompress_from_base64("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_decompress_invalid_base64() {
        // 无效 base64 应返回错误
        let result = decompress_from_base64("!!!invalid!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_garbage() {
        // 垃圾数据解压应返回错误
        let result = decompress_from_base64("AAAA");
        assert!(result.is_err());
    }

    #[test]
    fn test_large_string() {
        // 大字符串（约 100KB）
        let input = "x".repeat(100_000);
        let compressed = compress_to_base64(&input).unwrap();
        let decompressed = decompress_from_base64(&compressed).unwrap();
        assert_eq!(decompressed.len(), input.len());
        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_very_large_string() {
        // 超长字符串（约 1MB）
        let input = "The quick brown fox jumps over the lazy dog. ".repeat(23_000);
        assert!(input.len() > 1_000_000);
        let compressed = compress_to_base64(&input).unwrap();
        let decompressed = decompress_from_base64(&compressed).unwrap();
        assert_eq!(decompressed.len(), input.len());
        assert_eq!(decompressed, input);
    }

    // ─── JSON 对象 ────────────────────────────────────────

    #[test]
    fn test_json_roundtrip() {
        let input =
            r#"{"gameVersion":"1.0.0","party":[1,2,3],"actors":{"1":{"name":"Alice","hp":100}}}"#;
        let compressed = compress_to_base64(input).unwrap();
        let decompressed = decompress_from_base64(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }
}
