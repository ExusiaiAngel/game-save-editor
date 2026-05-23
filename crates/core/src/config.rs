// game-tool-core: 配置管理模块
//
// 分层配置加载策略（优先级从低到高）：
//   1. 默认值
//   2. config.json (Python 兼容 JSON)
//   3. config.toml (Rust 主配置文件)
//   4. 环境变量 GAME_TOOL_* (最高优先级)

use serde::{Deserialize, Serialize};
use std::path::Path;

const CONFIG_TOML: &str = "config.toml";
const CONFIG_JSON: &str = "config.json";
const ENV_PREFIX: &str = "GAME_TOOL_";

/// 全局应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub tcp_port: u16,
    pub cdp_port: u16,
    pub backup_keep: usize,
    pub language: String,
    pub plugin_auto_connect: bool,
    pub dark_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tcp_port: 19999,
            cdp_port: 9222,
            backup_keep: 10,
            language: "zh-CN".to_string(),
            plugin_auto_connect: true,
            dark_mode: true,
        }
    }
}

/// 合并另一个配置实例（非默认值字段覆盖当前值）
impl AppConfig {
    fn merge_from(&mut self, other: AppConfig) {
        // 仅当字段值不等于其默认值时覆盖，确保文件/环境变量能覆盖
        if other.tcp_port != u16::default() && other.tcp_port != 0 {
            self.tcp_port = other.tcp_port;
        }
        if other.cdp_port != u16::default() && other.cdp_port != 0 {
            self.cdp_port = other.cdp_port;
        }
        if other.backup_keep != usize::default() && other.backup_keep != 0 {
            self.backup_keep = other.backup_keep;
        }
        if !other.language.is_empty() {
            self.language = other.language;
        }
        // bool 类型特殊处理：总是接受文件/环境变量的值
        self.plugin_auto_connect = other.plugin_auto_connect;
    }
}

/// 加载配置，优先级：默认值 < config.json < config.toml < 环境变量 GAME_TOOL_*
pub fn load_config() -> Result<AppConfig, anyhow::Error> {
    let mut config = AppConfig::default();

    // Layer 2: config.json (Python 兼容)
    if let Ok(content) = std::fs::read_to_string(CONFIG_JSON) {
        if let Ok(json_config) = serde_json::from_str::<AppConfig>(&content) {
            config.merge_from(json_config);
        }
    }

    // Layer 3: config.toml (主配置文件)
    if let Ok(content) = std::fs::read_to_string(CONFIG_TOML) {
        if let Ok(toml_config) = toml::from_str::<AppConfig>(&content) {
            config.merge_from(toml_config);
        }
    }

    // Layer 4: 环境变量 GAME_TOOL_* (最高优先级)
    apply_env_vars(&mut config);

    Ok(config)
}

/// 从指定路径加载 TOML 配置文件（环境变量仍可覆盖）
pub fn load_config_from(path: impl AsRef<Path>) -> Result<AppConfig, anyhow::Error> {
    let mut config = AppConfig::default();

    let content = std::fs::read_to_string(path.as_ref())?;
    if let Ok(toml_config) = toml::from_str::<AppConfig>(&content) {
        config.merge_from(toml_config);
    }

    // 环境变量仍然覆盖
    apply_env_vars(&mut config);

    Ok(config)
}

/// 保存配置到 config.toml
pub fn save_config(config: &AppConfig) -> Result<(), anyhow::Error> {
    let toml_str = toml::to_string_pretty(config)?;
    std::fs::write(CONFIG_TOML, toml_str)?;
    Ok(())
}

/// 保存配置到指定路径
pub fn save_config_to(config: &AppConfig, path: impl AsRef<Path>) -> Result<(), anyhow::Error> {
    let toml_str = toml::to_string_pretty(config)?;
    std::fs::write(path.as_ref(), toml_str)?;
    Ok(())
}

/// 将配置导出为 JSON（供 Python 前端或其他工具读取）
pub fn export_config_json(config: &AppConfig) -> Result<String, anyhow::Error> {
    Ok(serde_json::to_string_pretty(config)?)
}

/// 从 JSON 字符串导入配置
pub fn import_config_json(json_str: &str) -> Result<AppConfig, anyhow::Error> {
    Ok(serde_json::from_str(json_str)?)
}

/// 应用 GAME_TOOL_* 环境变量覆盖配置
fn apply_env_vars(config: &mut AppConfig) {
    use std::env;

    if let Ok(val) = env::var(format!("{}TCP_PORT", ENV_PREFIX)) {
        if let Ok(parsed) = val.parse::<u16>() {
            config.tcp_port = parsed;
        }
    }
    if let Ok(val) = env::var(format!("{}CDP_PORT", ENV_PREFIX)) {
        if let Ok(parsed) = val.parse::<u16>() {
            config.cdp_port = parsed;
        }
    }
    if let Ok(val) = env::var(format!("{}BACKUP_KEEP", ENV_PREFIX)) {
        if let Ok(parsed) = val.parse::<usize>() {
            config.backup_keep = parsed;
        }
    }
    if let Ok(val) = env::var(format!("{}LANGUAGE", ENV_PREFIX)) {
        if !val.is_empty() {
            config.language = val;
        }
    }
    if let Ok(val) = env::var(format!("{}PLUGIN_AUTO_CONNECT", ENV_PREFIX)) {
        match val.to_lowercase().as_str() {
            "true" | "1" | "yes" => config.plugin_auto_connect = true,
            "false" | "0" | "no" => config.plugin_auto_connect = false,
            _ => {}
        }
    }
}

// ── 测试 ──────────────────────────────────────────────

/// 注：涉及环境变量的测试（GAME_TOOL_*）在并行测试中会有竞态，
/// 因此移入单独的测试函数 test_env_override_full_chain，
/// 该函数在一个测试内完成完整的优先级链路验证。
/// 运行方式：cargo test -p game-tool-core -- config:: --test-threads=1

#[cfg(test)]
mod tests {
    use super::*;

    // ── 默认值测试 ────────────────────────────────────

    #[test]
    fn test_default_values() {
        let config = AppConfig::default();
        assert_eq!(config.tcp_port, 19999);
        assert_eq!(config.cdp_port, 9222);
        assert_eq!(config.backup_keep, 10);
        assert_eq!(config.language, "zh-CN");
        assert!(config.plugin_auto_connect);
    }

    // ── 序列化/反序列化 ────────────────────────────────

    #[test]
    fn test_serde_roundtrip_toml() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.tcp_port, deserialized.tcp_port);
        assert_eq!(config.language, deserialized.language);
    }

    #[test]
    fn test_serde_roundtrip_json() {
        let config = AppConfig::default();
        let json_str = export_config_json(&config).unwrap();
        let deserialized = import_config_json(&json_str).unwrap();
        assert_eq!(config.tcp_port, deserialized.tcp_port);
        assert_eq!(config.language, deserialized.language);
    }

    // ── 文件加载/保存 ────────────────────────────────

    #[test]
    fn test_toml_file_override() {
        let test_path = "test_config_override.toml";

        let test_toml = r#"
tcp_port = 12345
cdp_port = 7777
backup_keep = 5
language = "en-US"
plugin_auto_connect = false
"#;
        std::fs::write(test_path, test_toml).unwrap();

        // 使用 load_config_from 直接从文件加载（不经过 env）
        let config = AppConfig::default();
        let config = {
            let content = std::fs::read_to_string(test_path).unwrap();
            let mut cfg = config;
            if let Ok(toml_config) = toml::from_str::<AppConfig>(&content) {
                cfg.merge_from(toml_config);
            }
            cfg
        };

        assert_eq!(config.tcp_port, 12345);
        assert_eq!(config.cdp_port, 7777);
        assert_eq!(config.backup_keep, 5);
        assert_eq!(config.language, "en-US");
        assert!(!config.plugin_auto_connect);

        std::fs::remove_file(test_path).unwrap();
    }

    #[test]
    fn test_json_file_compatibility() {
        let json_path = "test_config_compat.json";

        let json_content = r#"{
  "tcp_port": 20000,
  "cdp_port": 9333,
  "backup_keep": 20,
  "language": "zh-CN",
  "plugin_auto_connect": true
}"#;
        std::fs::write(json_path, json_content).unwrap();

        // 直接 JSON 反序列化验证兼容性
        let config: AppConfig = serde_json::from_str(json_content).unwrap();
        assert_eq!(config.tcp_port, 20000);
        assert_eq!(config.cdp_port, 9333);

        std::fs::remove_file(json_path).unwrap();
    }

    #[test]
    fn test_save_and_load() {
        let test_path = "test_config_save_load.toml";

        let config = AppConfig {
            tcp_port: 3000,
            cdp_port: 4000,
            backup_keep: 3,
            language: "ja-JP".to_string(),
            plugin_auto_connect: false,
            dark_mode: false,
        };

        save_config_to(&config, test_path).unwrap();

        let loaded = {
            let content = std::fs::read_to_string(test_path).unwrap();
            let mut cfg = AppConfig::default();
            if let Ok(toml_config) = toml::from_str::<AppConfig>(&content) {
                cfg.merge_from(toml_config);
            }
            cfg
        };
        assert_eq!(loaded.tcp_port, 3000);
        assert_eq!(loaded.cdp_port, 4000);
        assert_eq!(loaded.backup_keep, 3);
        assert_eq!(loaded.language, "ja-JP");
        assert!(!loaded.plugin_auto_connect);

        std::fs::remove_file(test_path).unwrap();
    }

    // ── JSON 导入/导出 ────────────────────────────────

    #[test]
    fn test_json_export_import() {
        let config = AppConfig {
            tcp_port: 5555,
            ..Default::default()
        };

        let json = export_config_json(&config).unwrap();
        let imported = import_config_json(&json).unwrap();

        assert_eq!(imported.tcp_port, 5555);
        assert_eq!(imported.language, "zh-CN");
        assert!(imported.plugin_auto_connect);
    }

    // ── 完整优先级链路（单测内完成，避免 env 竞争） ──

    #[test]
    fn test_env_override_full_chain() {
        use std::env;

        // 清理历史环境变量
        unsafe {
            env::remove_var("GAME_TOOL_TCP_PORT");
            env::remove_var("GAME_TOOL_CDP_PORT");
            env::remove_var("GAME_TOOL_BACKUP_KEEP");
            env::remove_var("GAME_TOOL_LANGUAGE");
            env::remove_var("GAME_TOOL_PLUGIN_AUTO_CONNECT");
        }

        // 1. 默认值验证
        let mut config = AppConfig::default();
        assert_eq!(config.tcp_port, 19999);

        // 2. JSON 文件覆盖
        let json_path = CONFIG_JSON;
        std::fs::write(json_path, r#"{"tcp_port": 100, "cdp_port": 100}"#).unwrap();
        if let Ok(content) = std::fs::read_to_string(json_path) {
            if let Ok(json_config) = serde_json::from_str::<AppConfig>(&content) {
                config.merge_from(json_config);
            }
        }
        assert_eq!(config.tcp_port, 100);

        // 3. TOML 文件覆盖 JSON
        let toml_path = CONFIG_TOML;
        std::fs::write(toml_path, "tcp_port = 200\nlanguage = \"en\"").unwrap();
        if let Ok(content) = std::fs::read_to_string(toml_path) {
            if let Ok(toml_cfg) = toml::from_str::<AppConfig>(&content) {
                config.merge_from(toml_cfg);
            }
        }
        assert_eq!(config.tcp_port, 200);
        assert_eq!(config.language, "en");

        // 4. 环境变量覆盖 TOML
        unsafe {
            env::set_var("GAME_TOOL_CDP_PORT", "300");
        }
        apply_env_vars(&mut config);
        assert_eq!(config.cdp_port, 300);
        // TCP 端口仍来自 TOML（env 未设置 TCP_PORT）
        assert_eq!(config.tcp_port, 200);

        // 清理
        unsafe {
            env::remove_var("GAME_TOOL_CDP_PORT");
        }
        std::fs::remove_file(json_path).unwrap();
        std::fs::remove_file(toml_path).unwrap();
    }

    // ── apply_env_vars 单步验证 ──────────────────────

    #[test]
    fn test_apply_env_vars_single() {
        use std::env;

        unsafe {
            env::remove_var("GAME_TOOL_TCP_PORT");
            env::remove_var("GAME_TOOL_CDP_PORT");
            env::remove_var("GAME_TOOL_BACKUP_KEEP");
            env::remove_var("GAME_TOOL_LANGUAGE");
            env::remove_var("GAME_TOOL_PLUGIN_AUTO_CONNECT");
        }

        // 测试环境变量单独设置 TCP_PORT
        unsafe {
            env::set_var("GAME_TOOL_TCP_PORT", "9090");
        }
        let mut config = AppConfig::default();
        apply_env_vars(&mut config);
        assert_eq!(config.tcp_port, 9090);
        // 默认值未被影响
        assert_eq!(config.cdp_port, 9222);

        unsafe {
            env::remove_var("GAME_TOOL_TCP_PORT");
        }
    }

    #[test]
    fn test_apply_env_vars_overrides_file() {
        use std::env;

        unsafe {
            env::remove_var("GAME_TOOL_TCP_PORT");
            env::remove_var("GAME_TOOL_CDP_PORT");
            env::remove_var("GAME_TOOL_BACKUP_KEEP");
            env::remove_var("GAME_TOOL_LANGUAGE");
            env::remove_var("GAME_TOOL_PLUGIN_AUTO_CONNECT");
        }

        // 模拟从文件加载了 tcp_port=8888
        let mut config = AppConfig {
            tcp_port: 8888,
            ..Default::default()
        };

        // 环境变量覆盖
        unsafe {
            env::set_var("GAME_TOOL_TCP_PORT", "9999");
        }
        apply_env_vars(&mut config);
        assert_eq!(config.tcp_port, 9999);

        unsafe {
            env::remove_var("GAME_TOOL_TCP_PORT");
        }
    }
}
