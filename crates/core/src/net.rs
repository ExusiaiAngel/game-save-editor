//! TCP 行协议连接工具
//!
//! 封装基于换行符的 TCP 文本协议连接，供各引擎桥接器复用。
//! RPG Maker TCP 桥接和 Ren'Py TCP 桥接共用此模块。

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

/// TCP 行协议连接
///
/// 封装 connect / send_line / recv_line 的基本操作，
/// 提供基于换行符 `\n` 分隔的文本协议通道。
/// 所有引擎桥接器通过此类型与游戏进程通信。
pub struct TcpLineConnection {
    /// TCP 流（用于发送数据）
    stream: Option<TcpStream>,
    /// 带缓冲的读取器（用于接收行数据）
    reader: Option<BufReader<TcpStream>>,
}

impl TcpLineConnection {
    /// 连接到指定地址并配置连接参数
    ///
    /// # 参数
    /// - `addr`: 目标地址，格式如 "127.0.0.1:19999"
    ///
    /// # 连接配置
    /// - 阻塞模式（非非阻塞）
    /// - 禁用 Nagle 算法（`set_nodelay(true)`）以减少延迟
    /// - 读取超时 30 秒，写入超时 10 秒
    pub fn connect(addr: &str) -> Result<Self, std::io::Error> {
        // 建立 TCP 连接
        let stream = TcpStream::connect(addr)?;
        // 设为阻塞模式
        stream.set_nonblocking(false)?;
        // 禁用 Nagle 算法以降低延迟（游戏修改场景对实时性要求高）
        stream.set_nodelay(true)?;
        // 设置超时防止无限阻塞
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        // 克隆一个独立的流句柄供 BufReader 使用
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self {
            stream: Some(stream),
            reader: Some(reader),
        })
    }

    /// 发送一行文本（自动追加 `\n` 换行符）
    ///
    /// # 参数
    /// - `line`: 待发送的文本（不含换行符）
    ///
    /// # 协议约定
    /// 所有命令以换行符 `\n` 终止，与 RPG Maker 插件的行协议一致。
    pub fn send_line(&mut self, line: &str) -> Result<(), std::io::Error> {
        match self.stream {
            Some(ref mut stream) => {
                let mut buf = line.as_bytes().to_vec();
                // 追加换行符作为消息终止标记
                buf.push(b'\n');
                stream.write_all(&buf)?;
                stream.flush()?;
                Ok(())
            }
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "not connected",
            )),
        }
    }

    /// 读取一行文本（自动去除尾部换行符 `\n` 和 `\r`）
    ///
    /// # 返回
    /// 去除尾部换行符后的纯文本行。
    pub fn recv_line(&mut self) -> Result<String, std::io::Error> {
        if let Some(ref mut reader) = self.reader {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            // 去除尾部的 \n 和 \r\n
            Ok(line.trim_end_matches(['\n', '\r']).to_string())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "not connected",
            ))
        }
    }

    /// 检查当前是否处于已连接状态
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// 断开连接并释放资源
    ///
    /// 关闭 TCP 流的读写两端，并清空读取器。
    pub fn disconnect(&mut self) {
        if let Some(stream) = self.stream.take() {
            // 优雅关闭：同时关闭读写两端
            let _ = stream.shutdown(Shutdown::Both);
        }
        self.reader = None;
    }
}
