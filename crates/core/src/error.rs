//! 统一错误类型定义
//!
//! 使用 `thiserror` crate 定义游戏工具核心库的所有错误类型，
//! 提供清晰的错误信息和自动派生（From）转换。

/// 游戏工具核心错误枚举
///
/// 覆盖存档操作、格式检测、游戏连接、插件注入、
/// 进程检测等所有核心模块的失败场景。
/// 同时支持从标准 I/O、JSON、ZIP 错误的自动转换。
#[derive(thiserror::Error, Debug)]
pub enum GameToolError {
    /// 加载存档文件失败：文件不存在、损坏或格式解析错误
    #[error("无法加载存档文件: {0}")]
    ArchiveLoadError(String),

    /// 保存存档文件失败：磁盘写入、权限或序列化错误
    #[error("无法保存存档文件: {0}")]
    ArchiveSaveError(String),

    /// 存档格式无法识别：不支持的加密/压缩/引擎格式
    #[error("无法识别的存档格式: {0}")]
    FormatDetectError(String),

    /// 与游戏进程建立连接失败：TCP 端口未监听或进程不存在
    #[error("游戏连接失败: {0}")]
    BridgeConnectError(String),

    /// 向游戏进程发送命令失败：写入超时或协议格式错误
    #[error("写入游戏数据失败: {0}")]
    BridgeCommandError(String),

    /// 插件注入失败：目标进程拒绝注入或注入逻辑错误
    #[error("插件注入失败: {0}")]
    PluginInjectError(String),

    /// 游戏进程检测失败：未找到匹配的进程
    #[error("游戏进程检测失败: {0}")]
    GameDetectError(String),

    /// 标准 I/O 错误（文件读写、网络操作等）
    #[error("I/O 错误: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON 序列化/反序列化错误
    #[error("JSON 序列化错误: {0}")]
    JsonError(#[from] serde_json::Error),

    /// ZIP 压缩/解压错误（用于 Ren'Py 存档等）
    #[error("ZIP 压缩错误: {0}")]
    ZipError(#[from] zip::result::ZipError),

    /// 内存桥通用错误
    #[error("内存桥错误: {0}")]
    BridgeError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_load_error_display() {
        let err = GameToolError::ArchiveLoadError("save.dat".into());
        let msg = err.to_string();
        assert!(msg.contains("无法加载存档文件"));
        assert!(msg.contains("save.dat"));
    }

    #[test]
    fn test_archive_save_error_display() {
        let err = GameToolError::ArchiveSaveError("save.dat".into());
        let msg = err.to_string();
        assert!(msg.contains("无法保存存档文件"));
    }

    #[test]
    fn test_format_detect_error_display() {
        let err = GameToolError::FormatDetectError("未知格式".into());
        let msg = err.to_string();
        assert!(msg.contains("无法识别的存档格式"));
    }

    #[test]
    fn test_bridge_connect_error_display() {
        let err = GameToolError::BridgeConnectError("127.0.0.1:8080".into());
        let msg = err.to_string();
        assert!(msg.contains("游戏连接失败"));
    }

    #[test]
    fn test_bridge_command_error_display() {
        let err = GameToolError::BridgeCommandError("写入超时".into());
        let msg = err.to_string();
        assert!(msg.contains("写入游戏数据失败"));
    }

    #[test]
    fn test_plugin_inject_error_display() {
        let err = GameToolError::PluginInjectError("注入被拒绝".into());
        let msg = err.to_string();
        assert!(msg.contains("插件注入失败"));
    }

    #[test]
    fn test_game_detect_error_display() {
        let err = GameToolError::GameDetectError("未找到进程".into());
        let msg = err.to_string();
        assert!(msg.contains("游戏进程检测失败"));
    }

    #[test]
    fn test_io_error_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件未找到");
        let err: GameToolError = io_err.into();
        let msg = err.to_string();
        assert!(msg.contains("I/O 错误"));
    }

    #[test]
    fn test_json_error_from() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err: GameToolError = json_err.into();
        let msg = err.to_string();
        assert!(msg.contains("JSON 序列化错误"));
    }
}
