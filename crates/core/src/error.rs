//! 错误类型定义

/// 游戏工具核心错误类型
#[derive(thiserror::Error, Debug)]
pub enum GameToolError {
    #[error("无法加载存档文件: {0}")]
    ArchiveLoadError(String),

    #[error("无法保存存档文件: {0}")]
    ArchiveSaveError(String),

    #[error("无法识别的存档格式: {0}")]
    FormatDetectError(String),

    #[error("游戏连接失败: {0}")]
    BridgeConnectError(String),

    #[error("写入游戏数据失败: {0}")]
    BridgeCommandError(String),

    #[error("插件注入失败: {0}")]
    PluginInjectError(String),

    #[error("游戏进程检测失败: {0}")]
    GameDetectError(String),

    #[error("I/O 错误: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON 序列化错误: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("ZIP 压缩错误: {0}")]
    ZipError(#[from] zip::result::ZipError),
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
