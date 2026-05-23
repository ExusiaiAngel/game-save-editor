//! Base64 编码/解码工具
//!
//! Ren'Py ZIP 存档和 Unreal GVAS 存档共用此模块。

pub fn encode(data: &[u8]) -> String {
    let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b = |i: usize| chunk.get(i).copied().unwrap_or(0) as u32;
        let n = (b(0) << 16) | (b(1) << 8) | b(2);
        result.push(chars[((n >> 18) & 0x3F) as usize] as char);
        result.push(chars[((n >> 12) & 0x3F) as usize] as char);
        result.push(if chunk.len() > 1 {
            chars[((n >> 6) & 0x3F) as usize]
        } else {
            b'='
        } as char);
        result.push(if chunk.len() > 2 {
            chars[(n & 0x3F) as usize]
        } else {
            b'='
        } as char);
    }
    result
}

pub fn decode(input: &str) -> Option<Vec<u8>> {
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
