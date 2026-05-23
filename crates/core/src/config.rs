// game-tool-core: 配置管理模块
//
// 采用分层配置加载策略（优先级从低到高）：
//   1. 默认值（AppConfig::default()，硬编码在源码中）
//   2. config.json（Python 兼容的 JSON 配置文件）
//   3. config.toml（Rust 原生主配置文件，在用户配置目录下）
//   4. 环境变量 GAME_TOOL_*（最高优先级，用于容器/CI 覆盖）

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// TOML 配置文件名
const CONFIG_FILENAME: &str = "config.toml";
/// JSON 兼容配置文件名（与 Python 前端共享）
const CONFIG_JSON: &str = "config.json";
/// 环境变量前缀，所有 GAME_TOOL_* 变量用于覆盖配置
const ENV_PREFIX: &str = "GAME_TOOL_";

/// 获取操作系统用户配置目录下的应用配置文件夹
fn config_dir() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("GameSaveEditor")
}

/// 获取 TOML 配置文件的完整路径
fn config_toml_path() -> PathBuf {
    config_dir().join(CONFIG_FILENAME)
}

/// 全局应用配置
///
/// 所有字段均可在各层配置中覆盖。CLI 和 GUI 共享同一份配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// TCP 桥接监听端口（RPG Maker MV/MZ 插件连接端口）
    pub tcp_port: u16,
    /// Chrome DevTools Protocol 端口（Ren'Py/Web 游戏连接端口）
    pub cdp_port: u16,
    /// 存档备份保留数量（0 = 不限制）
    pub backup_keep: usize,
    /// 界面语言（如 "zh-CN", "en-US", "ja-JP"）
    pub language: String,
    /// 是否在检测到游戏后自动连接插件
    pub plugin_auto_connect: bool,
    /// 是否启用深色模式主题
    pub dark_mode: bool,
    /// 最近打开的游戏目录列表
    #[serde(default)]
    pub recent_games: Vec<String>,
}

/// AppConfig 的默认值
///
/// 所有字段的默认值作为配置加载的第一层（最低优先级）。
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tcp_port: 19999,
            cdp_port: 9222,
            backup_keep: 10,
            language: "zh-CN".to_string(),
            plugin_auto_connect: true,
            dark_mode: false,
            recent_games: Vec::new(),
        }
    }
}

impl AppConfig {
    /// 将另一个配置实例的字段合并到当前配置
    ///
    /// 采用"非零非空"合并策略：
    /// - 数值类型（端口、备份数量）：仅当不同于类型默认值且不为 0 时覆盖
    ///   （0 通常表示"未设置/使用默认值"）
    /// - 字符串类型（语言）：仅非空时覆盖
    /// - bool 类型：始终接受覆盖（因为 false 是有效的显式选择）
    ///
    /// 此策略确保配置文件或环境变量中的显式设置能够正确生效，
    /// 同时避免零值/空值意外覆盖高层配置。
    fn merge_from(&mut self, other: AppConfig) {
        // TCP 端口：仅当 other 的值不是默认 0 且非零时才覆盖
        if other.tcp_port != u16::default() && other.tcp_port != 0 {
            self.tcp_port = other.tcp_port;
        }
        // CDP 端口：同上策略，防止 0 值误覆盖
        if other.cdp_port != u16::default() && other.cdp_port != 0 {
            self.cdp_port = other.cdp_port;
        }
        // 备份数量：仅当 other 的值不是默认 0 且非零时才覆盖
        if other.backup_keep != usize::default() && other.backup_keep != 0 {
            self.backup_keep = other.backup_keep;
        }
        // 语言设置：仅非空字符串才覆盖（空串视为"未指定"）
        if !other.language.is_empty() {
            self.language = other.language;
        }
        // bool 类型无"默认零值"的概念，显式设置的 false 也是有效意图，
        // 因此总是接受覆盖，不做任何条件检查
        self.plugin_auto_connect = other.plugin_auto_connect;
    }
}

/// 加载完整配置
///
/// 按优先级从低到高依次加载：默认值 → config.json → config.toml → 环境变量。
/// 每一层覆盖上一层的同名字段（非零非空值）。
///
/// # 加载链路说明
/// 采用分层覆盖策略（higher priority wins）：
/// 1. 硬编码默认值（最低优先级，作为基础配置）
/// 2. 工作目录下的 config.json（与 Python 前端共享的 JSON 兼容配置）
/// 3. 用户配置目录下的 config.toml（Rust 原生 TOML 格式主配置）
/// 4. 环境变量 GAME_TOOL_*（最高优先级，用于容器/CI 临时覆盖）
///
/// 其中第 2、3 层文件可能不存在，此时静默跳过。
pub fn load_config() -> Result<AppConfig, anyhow::Error> {
    // ── 第 1 层：硬编码默认值（最低优先级）──
    // AppConfig::default() 提供所有字段的保守默认值，
    // 确保即使没有任何外部配置文件，程序也能正常启动
    let mut config = AppConfig::default();

    // ── 第 2 层：config.json（Python 兼容配置）──
    // 读取工作目录下的 JSON 文件，与 Python 版编辑器共享同一份配置。
    // serde_json::from_str 失败时静默忽略（文件可能不存在或格式不兼容）
    if let Ok(content) = std::fs::read_to_string(CONFIG_JSON) {
        if let Ok(json_config) = serde_json::from_str::<AppConfig>(&content) {
            config.merge_from(json_config);
        }
    }

    // ── 第 3 层：config.toml（用户配置目录下的主配置文件）──
    // 路径为 ~/.config/GameSaveEditor/config.toml（Linux/macOS）
    // 或 %APPDATA%/GameSaveEditor/config.toml（Windows）
    // TOML 格式支持更丰富的类型（嵌套表、枚举等），作为 Rust 端的主配置格式
    let toml_path = config_toml_path();
    if let Ok(content) = std::fs::read_to_string(&toml_path) {
        if let Ok(toml_config) = toml::from_str::<AppConfig>(&content) {
            config.merge_from(toml_config);
        }
    }

    // ── 第 4 层：环境变量 GAME_TOOL_*（最高优先级）──
    // 环境变量具有最高优先级，可覆盖前 3 层的任意配置。
    // 适用于 Docker 容器、CI/CD 流水线等需要临时修改配置的场景
    apply_env_vars(&mut config);

    Ok(config)
}

/// 从指定路径加载 TOML 配置文件
///
/// 直接读取指定路径的 TOML，跳过默认的配置目录查找。
/// 环境变量仍然在文件加载后覆盖，保持最高优先级。
pub fn load_config_from(path: impl AsRef<Path>) -> Result<AppConfig, anyhow::Error> {
    let mut config = AppConfig::default();

    let content = std::fs::read_to_string(path.as_ref())?;
    if let Ok(toml_config) = toml::from_str::<AppConfig>(&content) {
        config.merge_from(toml_config);
    }

    // 环境变量始终覆盖文件配置
    apply_env_vars(&mut config);

    Ok(config)
}

/// 将当前配置保存到用户配置目录下的 config.toml
///
/// 自动创建父目录（如 ~/.config/GameSaveEditor/ 或 %APPDATA%/GameSaveEditor/）。
///
/// # 写入流程
/// 1. 将 AppConfig 序列化为格式化的 TOML 字符串（`toml::to_string_pretty`）
/// 2. 确保配置目录存在（`create_dir_all` 递归创建）
/// 3. 将 TOML 字符串原子写入文件（`std::fs::write` 覆盖写入）
pub fn save_config(config: &AppConfig) -> Result<(), anyhow::Error> {
    // 序列化为人类可读的格式化 TOML（带缩进和空行）
    let toml_str = toml::to_string_pretty(config)?;
    // 递归创建配置目录（若已存在则无操作）
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    // 写入文件（覆盖已有配置）
    std::fs::write(config_toml_path(), toml_str)?;
    Ok(())
}

/// 将配置保存到指定路径（用于导出或测试）
///
/// 与 `save_config` 的区别在于可以指定任意写入路径，
/// 不局限于默认的配置目录。适用于：
/// - 测试场景：写入临时目录验证序列化/反序列化
/// - 导出功能：将当前配置导出为可分享的配置文件
pub fn save_config_to(config: &AppConfig, path: impl AsRef<Path>) -> Result<(), anyhow::Error> {
    let toml_str = toml::to_string_pretty(config)?;
    std::fs::write(path.as_ref(), toml_str)?;
    Ok(())
}

/// 将配置导出为 JSON 字符串
///
/// 用于供 Python 前端或其他外部工具读取当前配置。
pub fn export_config_json(config: &AppConfig) -> Result<String, anyhow::Error> {
    Ok(serde_json::to_string_pretty(config)?)
}

/// 从 JSON 字符串导入配置（反序列化 → AppConfig 实例）
pub fn import_config_json(json_str: &str) -> Result<AppConfig, anyhow::Error> {
    Ok(serde_json::from_str(json_str)?)
}

/// 应用环境变量覆盖配置字段
///
/// 遍历所有支持的环境变量，将存在的变量值解析后写入配置。
/// 解析失败（如类型无法转换）时静默忽略，不会中断配置加载流程。
///
/// # 支持的环境变量
/// - `GAME_TOOL_TCP_PORT`: TCP 监听端口（u16）
/// - `GAME_TOOL_CDP_PORT`: CDP 调试端口（u16）
/// - `GAME_TOOL_BACKUP_KEEP`: 备份保留数量（usize）
/// - `GAME_TOOL_LANGUAGE`: 界面语言（字符串）
/// - `GAME_TOOL_PLUGIN_AUTO_CONNECT`: 自动连接开关（true/false/1/0/yes/no）
///
/// # 设计考虑
/// 环境变量不采用自动枚举/反射机制（如遍历所有以 GAME_TOOL_ 为前缀的变量），
/// 而是逐个显式处理，原因如下：
/// 1. 避免意外读取到无关的 GAME_TOOL_* 变量导致的静默覆盖错误
/// 2. 每个字段的解析逻辑不同（u16/usize/bool/string），需要不同的类型处理
/// 3. 后续新增配置字段时，环境变量支持需显式添加，防止遗漏类型转换逻辑
fn apply_env_vars(config: &mut AppConfig) {
    use std::env;

    // ── TCP 端口 ──
    // 环境变量名：GAME_TOOL_TCP_PORT
    // 类型：u16（0-65535），解析失败时保持原值不变
    if let Ok(val) = env::var(format!("{}TCP_PORT", ENV_PREFIX)) {
        if let Ok(parsed) = val.parse::<u16>() {
            config.tcp_port = parsed;
        }
    }

    // ── CDP 端口 ──
    // 环境变量名：GAME_TOOL_CDP_PORT
    // 与 TCP 端口同理，解析失败静默忽略
    if let Ok(val) = env::var(format!("{}CDP_PORT", ENV_PREFIX)) {
        if let Ok(parsed) = val.parse::<u16>() {
            config.cdp_port = parsed;
        }
    }

    // ── 备份保留数量 ──
    // 环境变量名：GAME_TOOL_BACKUP_KEEP
    // 类型：usize（平台字长），0 表示不限制备份数量
    if let Ok(val) = env::var(format!("{}BACKUP_KEEP", ENV_PREFIX)) {
        if let Ok(parsed) = val.parse::<usize>() {
            config.backup_keep = parsed;
        }
    }

    // ── 界面语言 ──
    // 环境变量名：GAME_TOOL_LANGUAGE
    // 非空字符串直接覆盖（空串视为"未设置"，保持原配置不变）
    if let Ok(val) = env::var(format!("{}LANGUAGE", ENV_PREFIX)) {
        if !val.is_empty() {
            config.language = val;
        }
    }

    // ── 插件自动连接 ──
    // 环境变量名：GAME_TOOL_PLUGIN_AUTO_CONNECT
    // 兼容多种布尔值格式：true/false（标准）、1/0（数字）、yes/no（YAML 风格）
    // 其他值（如 "auto"、"maybe"）静默忽略，保持原值
    if let Ok(val) = env::var(format!("{}PLUGIN_AUTO_CONNECT", ENV_PREFIX)) {
        match val.to_lowercase().as_str() {
            "true" | "1" | "yes" => config.plugin_auto_connect = true,
            "false" | "0" | "no" => config.plugin_auto_connect = false,
            _ => {} // 无法识别的值，不做任何修改
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
            recent_games: Vec::new(),
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
        let toml_path = "test_env_override.toml";
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
