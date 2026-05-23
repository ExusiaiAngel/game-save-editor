//! 自定义 Base64 编码/解码工具
//!
//! 纯 Rust 实现的 Base64 编解码，无外部依赖。
//! Ren'Py ZIP 存档和 Unreal GVAS 存档共用此模块进行字节数据编解码。

/// 将字节数组编码为标准 Base64 字符串
///
/// # 编码规则
/// 每 3 字节一组，拆分为 4 个 6-bit 索引，
/// 映射到标准 Base64 字符表 `A-Z a-z 0-9 + /`，
/// 不足 3 字节时以 `=` 填充至 4 的倍数。
pub fn encode(data: &[u8]) -> String {
    // 标准 Base64 字符表（64 个可打印字符）
    let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    // ── 按 3 字节分组进行 Base64 编码 ──
    // Base64 的编码原理：每 3 个字节（24 bit）拆分为 4 个 6-bit 索引，
    // 每个索引映射到 Base64 字符表中的一个字符。
    // 若最后一组不足 3 字节，以 '=' 填充缺失位置到 4 字符输出。
    for chunk in data.chunks(3) {
        // 辅助闭包：安全获取第 i 个字节，超出范围时填充 0
        // 这实现了 RFC 4648 §4 规定的零填充语义
        let b = |i: usize| chunk.get(i).copied().unwrap_or(0) as u32;
        // 将 3 个字节拼接为一个 24-bit 无符号整数：
        // [b0       ][b1       ][b2       ] ← 3 个输入字节
        // [b0<<16   ][b1<<8    ][b2       ] ← 移位后合并为 n
        let n = (b(0) << 16) | (b(1) << 8) | b(2);

        // 从 24-bit 整数中依次提取 4 个 6-bit 值：
        // 左起第 1 个 6-bit（bit 23~18）：n >> 18，然后 & 0x3F 取低 6 位
        result.push(chars[((n >> 18) & 0x3F) as usize] as char);
        // 左起第 2 个 6-bit（bit 17~12）：n >> 12
        result.push(chars[((n >> 12) & 0x3F) as usize] as char);
        // 左起第 3 个 6-bit（bit 11~6）：n >> 6
        // 注意：若 chunk 只有 1 字节，则 b(1)=0、b(2)=0，此值全为零，
        // 按 Base64 规范应输出 '=' 而非 'A'（即零的编码）
        result.push(if chunk.len() > 1 {
            chars[((n >> 6) & 0x3F) as usize]
        } else {
            b'='
        } as char);
        // 左起第 4 个 6-bit（bit 5~0）：n 的低 6 位
        // 若 chunk 不足 3 字节（1 或 2 字节），此位置也输出 '=' 填充
        result.push(if chunk.len() > 2 {
            chars[(n & 0x3F) as usize]
        } else {
            b'='
        } as char);
    }
    result
}

/// 将标准 Base64 字符串解码为原始字节数组
///
/// # 解码流程
/// 1. 去除尾部的 `=` 填充符
/// 2. 逐字符解析为 6-bit 值
/// 3. 每积累 8 bit 输出一个字节
///
/// # 返回
/// - `Some(Vec<u8>)`: 解码成功
/// - `None`: 输入包含非 Base64 字符
pub fn decode(input: &str) -> Option<Vec<u8>> {
    // ── 解码流程：逐字符处理 ──
    // Base64 解码是编码的逆过程：每 4 个 Base64 字符还原为 3 个原始字节。
    // 这里采用"位累积"式实现（而非按 4 字符分组）：
    //
    // 1. 每个 Base64 字符映射为 6-bit 值
    // 2. 将 6-bit 不断左移追加到 buf 累积缓冲区
    // 3. 每当缓冲区积累满 >= 8 bit，就提取高 8 bit 作为一个输出字节
    // 4. 保留剩余未满 8 bit 的位到下一次循环继续累积
    //
    // 这种实现方式更简洁：无需处理分组边界，自动处理尾部不足 3 字节的情况，
    // 并且天然支持去除了 '=' 填充符的输入。

    // 去除尾部填充符 '='（填充位对解码结果无影响，去除后简化实现）
    let input = input.trim_end_matches('=');
    let mut result = Vec::new();
    let mut buf = 0u32; // 位累积缓冲区：暂存未输出的编码位
    let mut bits = 0; // 当前缓冲区中已累积的有效位数（0~32）

    for c in input.chars() {
        // ── 字符 → 6-bit 数值映射表 ──
        // 映射规则（RFC 4648 §4）：
        //   A-Z  →  0-25
        //   a-z  → 26-51
        //   0-9  → 52-61
        //   +    → 62
        //   /    → 63
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,               // 大写字母：0-25
            'a'..='z' => c as u32 - 'a' as u32 + 26,          // 小写字母：26-51
            '0'..='9' => c as u32 - '0' as u32 + 52,          // 数字：52-61
            '+' => 62,                                         // 加号：62
            '/' => 63,                                         // 斜杠：63
            _ => return None,                                  // 非法字符 → 解码失败
        };
        // 将新解析的 6-bit 值追加到缓冲区末尾（左移 6 位空出位置，然后按位或合并）
        buf = (buf << 6) | val;
        bits += 6;
        // 缓冲区积累满 8 bit 时，提取最高 8 bit 作为一个输出字节
        if bits >= 8 {
            bits -= 8; // 消耗 8 bit
            // 取 buf 的高 8 bit（即最早进入缓冲区的 8 bit）作为输出字节
            result.push((buf >> bits) as u8);
            // 掩码保留低 bits 位（尚未输出的剩余位），清空高位
            buf &= (1 << bits) - 1;
        }
    }
    Some(result)
}
