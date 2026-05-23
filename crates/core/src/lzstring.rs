//! LZ-String 压缩/解压（封装 lz-str crate）
//!
//! 提供与 JS 版 lz-string 1.4.4 兼容的压缩/解压功能。
//! 主要用于 RPG Maker MV 存档的 JSON 数据解压和重新压缩。
//! 底层使用 `lz_str` crate（Rust 端 lz-string 算法的完整移植）。

use thiserror::Error;

/// LZ-String 压缩/解压操作可能产生的错误
#[derive(Error, Debug)]
pub enum LzStringError {
    /// 解压失败：输入数据格式无效或已损坏
    #[error("LZ-String 解压失败：数据无效或损坏")]
    DecompressFailed,

    /// UTF-16 解码失败：解压结果不是有效的 UTF-16 字节序列
    /// LZ-String 内部使用 UTF-16 编码，解压后需转换为 Rust 的 UTF-8 String
    #[error("UTF-16 解码失败：{0}")]
    Utf16DecodeError(#[from] std::string::FromUtf16Error),
}

/// 从 Base64 编码的 LZ-String 数据解压为原始字符串
///
/// RPG Maker MV 的存档文件（.rpgsave）是 Base64 编码的 LZ-String 压缩数据，
/// 解压后得到 JSON 格式的存档内容。
///
/// # 参数
/// - `input`: Base64 编码的 LZ-String 压缩数据（即 .rpgsave 文件内容）
///
/// # 返回
/// - `Ok(String)`: 解压后的原始 JSON 字符串
/// - `Err(LzStringError)`: 解压失败（数据损坏或 Base64 无效）
///
/// # 内部流程
/// 1. Base64 解码 → Vec<u16>
/// 2. LZ-String 解压算法 → Vec<u16>
/// 3. UTF-16 → UTF-8 转换 → String
pub fn decompress_from_base64(input: &str) -> Result<String, LzStringError> {
    // 空输入直接返回空字符串
    if input.is_empty() {
        return Ok(String::new());
    }

    // 调用 lz_str crate 进行 Base64 LZ-String 解压
    let decoded_u16 =
        lz_str::decompress_from_base64(input).ok_or(LzStringError::DecompressFailed)?;

    // Vec<u16> → String 转换
    // LZ-String 算法内部使用 UTF-16 编码存储字符，
    // 所以解压得到的是 u16 数组，需要转换为 Rust 的 UTF-8 String
    let result = String::from_utf16(&decoded_u16)?;
    Ok(result)
}

/// 将字符串压缩为 Base64 编码的 LZ-String 格式
///
/// 与 `decompress_from_base64` 互为逆操作，用于修改存档后
/// 重新压缩写回 .rpgsave 文件。
///
/// # 参数
/// - `input`: 待压缩的原始字符串（通常是序列化后的 JSON）
///
/// # 返回
/// - `Ok(String)`: Base64 编码的 LZ-String 压缩数据
/// - `Err(LzStringError)`: 压缩失败（当前实现不会发生此错误，
///   但保留 `Result` 签名以便将来扩展和向后兼容）
///
/// # 兼容性处理
/// `lz_str` crate 的 Base64 输出比 Python `lzstring` 库多一个尾随 `=`，
/// 此处去除以确保与 Python golden 测试文件的字节级兼容。
pub fn compress_to_base64(input: &str) -> Result<String, LzStringError> {
    if input.is_empty() {
        // lz_str::compress_to_base64("") 返回空字符串
        // Python lzstring 也返回空字符串（而非 "Q==="）
        // 注意 decompress_from_base64("") 同样返回 Ok("")，
        // 形成完整的往返一致性
        return Ok(String::new());
    }

    let mut compressed = lz_str::compress_to_base64(input);
    // lz_str crate 的 Base64 输出末尾多一个 '='，去掉以匹配 Python 库行为
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
