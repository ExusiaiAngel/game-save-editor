//! TCP 行协议连接 + WebSocket 基础工具
//!
//! RPG Maker TCP 桥接和 Ren'Py TCP 桥接共用此模块。

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

/// TCP 行协议连接
///
/// 封装 connect / send_line / recv_line，供各引擎桥接复用。
pub struct TcpLineConnection {
    stream: Option<TcpStream>,
    reader: Option<BufReader<TcpStream>>,
}

impl TcpLineConnection {
    /// 连接到指定地址（如 "127.0.0.1:19999"）
    pub fn connect(addr: &str) -> Result<Self, std::io::Error> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(false)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self {
            stream: Some(stream),
            reader: Some(reader),
        })
    }

    /// 发送一行文本（自动追加 \n）
    pub fn send_line(&mut self, line: &str) -> Result<(), std::io::Error> {
        match self.stream {
            Some(ref mut stream) => {
                let mut buf = line.as_bytes().to_vec();
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

    /// 读取一行文本（去除尾部换行符）
    #[must_use]
    pub fn recv_line(&mut self) -> Result<String, std::io::Error> {
        if let Some(ref mut reader) = self.reader {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            Ok(line.trim_end_matches(|c| c == '\n' || c == '\r').to_string())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "not connected",
            ))
        }
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn disconnect(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
        self.reader = None;
    }
}
