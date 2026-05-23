//! Game Save Editor — CLI 测试工具

use game_tool_core::{BridgeCommand, GameBridge, ISaveFormat};
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

#[derive(rust_embed::RustEmbed)]
#[folder = "../../profiles"]
struct ProfilesAsset;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init()
        .ok();

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

            let save_dir = find_save_dir(dir);
            println!("\n存档目录: {}", save_dir.display());
            if let Some(save_path) = find_latest_save(&save_dir) {
                println!("存档文件: {}", save_path.display());
                let fmt = game_tool_rpgmaker::format::RpgMakerFormat::new();
                match fmt.load(&save_path.to_string_lossy()) {
                    Ok(data) => {
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
            println!("内嵌配置: {} 个 profiles", ProfilesAsset::iter().count());
            println!("\n用法: GameSaveEditor --game-dir <路径> [--tcp] [--port <端口>]");
            println!("示例: GameSaveEditor --game-dir \"D:\\Games\\MyRPGMaker\"\n");
            println!("按 Enter 键退出...");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
    }
}

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
