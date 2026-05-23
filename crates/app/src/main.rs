//! GameSaveEditor — CLI 命令行测试工具。
//!
//! 本二进制 crate 提供命令行接口，用于在不启动 GUI 的情况下测试存档读写、
//! 引擎检测、实时桥接连接等核心功能。主要用于开发调试和自动化测试。
//!
//! # 用法
//!
//! ```bash
//! GameSaveEditor --game-dir "D:\Games\MyRPGMaker" [--tcp] [--port <端口>]
//! ```
//!
//! - `--game-dir <路径>`：指定游戏目录路径（必需，否则显示帮助信息）
//! - `--tcp`：启用 TCP 桥接连接测试
//! - `--port <端口>`：指定 TCP 连接端口（默认 19999）
//!
//! # 功能
//!
//! 1. 检测游戏目录的引擎类型
//! 2. 扫描游戏配置（开关名、变量名、角色名、物品名）
//! 3. 搜索存档目录，加载最新存档
//! 4. 显示存档摘要（金币、队伍、物品、存档次数、游戏时长）
//! 5. 列出前 20 个可修改字段
//! 6. 可选：测试 TCP 桥接连接实时读取

use game_tool_core::{BridgeCommand, GameBridge, ISaveFormat};
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

/// 内嵌的配置文件资源集合（来自 `profiles/` 目录）
#[derive(rust_embed::RustEmbed)]
#[folder = "../../profiles"]
struct ProfilesAsset;

/// 应用程序入口：解析命令行参数，执行 CLI 测试流程
fn main() {
    // 初始化 tracing 日志，使用环境变量控制过滤级别
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init()
        .ok();

    // 解析命令行参数：--game-dir <路径>、--tcp、--port <端口>
    let args: Vec<String> = std::env::args().collect();
    let game_dir = args
        .iter()
        .position(|a| a == "--game-dir")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string());
    let do_tcp = args.contains(&"--tcp".to_string());
    let port: u16 = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(19999);

    println!("=== Game Save Editor v{} ===\n", env!("CARGO_PKG_VERSION"));

    match &game_dir {
        Some(dir) => {
            // ── 引擎检测与配置扫描 ──
            println!("游戏目录: {}", dir);
            let engine = game_tool_core::detector::detect_by_filesystem(dir);
            println!("检测引擎: {:?}", engine);

            let config = game_tool_rpgmaker::scanner::scan_game_directory(dir);
            if config.data_loaded {
                println!("游戏标题: {}", config.game_title);
                println!(
                    "开关/变量/角色/物品: {} / {} / {} / {}",
                    config.switch_names.len(),
                    config.variable_names.len(),
                    config.actor_names.len(),
                    config.item_names.len()
                );
            }

            // ── 存档目录搜索与最新存档加载 ──
            let save_dir = find_save_dir(dir);
            println!("\n存档目录: {}", save_dir.display());
            if let Some(save_path) = find_latest_save(&save_dir) {
                println!("存档文件: {}", save_path.display());
                let fmt = game_tool_rpgmaker::format::RpgMakerFormat::new();
                match fmt.load(&save_path.to_string_lossy()) {
                    Ok(data) => {
                        // 显示存档摘要信息
                        let summary = fmt.get_summary(&data);
                        println!("\n--- 存档摘要 ---");
                        println!(
                            "金币: {}  队伍: {}人  物品: {}种  次数: {}  时间: {}秒",
                            summary.gold,
                            summary.party_size,
                            summary.item_count,
                            summary.save_count,
                            summary.play_time
                        );
                        if !summary.members.iter().all(|m| m.is_empty()) {
                            println!(
                                "队员: {}",
                                summary
                                    .members
                                    .iter()
                                    .map(|s| s.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                        // 显示前 20 个可修改字段
                        println!("\n--- 可修改字段 (前20条) ---");
                        let fields = fmt.scan_fields(&data, dir);
                        for (i, f) in fields.iter().take(20).enumerate() {
                            println!(
                                "  {:3}. [{:12}] {:30} => {}",
                                i + 1,
                                f.category,
                                f.display_name,
                                f.save_value
                            );
                        }
                        println!("  ... 共 {} 个字段", fields.len());
                    }
                    Err(e) => println!("加载存档失败: {}", e),
                }
            } else {
                println!("未找到存档文件");
            }

            // ── 可选的 TCP 桥接连接测试 ──
            if do_tcp {
                println!("\n--- TCP 桥接 (端口 {}) ---", port);
                let mut bridge = game_tool_rpgmaker::tcp::RpgMakerTcpBridge::new("127.0.0.1", port);
                match bridge.connect() {
                    Ok(()) => {
                        println!("已连接!");
                        match bridge.execute(&BridgeCommand::ReadAll) {
                            Ok(state) => println!("实时状态: {}", state),
                            Err(e) => println!("读取失败: {}", e),
                        }
                        bridge.disconnect();
                    }
                    Err(e) => println!("连接失败: {}", e),
                }
            }
        }
        None => {
            // ── 无参数时显示帮助信息并等待退出 ──
            println!("内嵌配置: {} 个 profiles", ProfilesAsset::iter().count());
            println!("\n用法: GameSaveEditor --game-dir <路径> [--tcp] [--port <端口>]");
            println!("示例: GameSaveEditor --game-dir \"D:\\Games\\MyRPGMaker\"\n");
            println!("按 Enter 键退出...");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
    }
}

/// 在游戏目录中查找存档子目录。
///
/// 按优先级依次检查：
/// 1. www/save（RPG Maker MV 的默认存档目录）
/// 2. www/Save
/// 3. save
/// 4. Save
/// 如果均不存在，默认返回 www/save。
fn find_save_dir(game_dir: &str) -> PathBuf {
    let base = Path::new(game_dir);
    for sub in &["www/save", "www/Save", "save", "Save"] {
        let d = base.join(sub);
        if d.is_dir() {
            return d;
        }
    }
    base.join("www/save")
}

/// 在存档目录中查找最新的 RPG Maker 存档文件。
///
/// 筛选规则：以 .rpgsave 或 .rmmzsave 结尾，排除备份文件（含 .bak.）、
/// config.rpgsave 和 global.rpgsave。按修改时间排序，返回最新的一个。
fn find_latest_save(save_dir: &Path) -> Option<PathBuf> {
    let mut saves: Vec<_> = std::fs::read_dir(save_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            (n.ends_with(".rpgsave") || n.ends_with(".rmmzsave"))
                && !n.contains(".bak.")
                && n != "config.rpgsave"
                && n != "global.rpgsave"
        })
        .collect();
    saves.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    saves.pop()
}
