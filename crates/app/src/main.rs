//! Game Save Editor — CLI 测试工具

mod registry;

use std::path::Path;
use tracing_subscriber::EnvFilter;
use game_tool_core::ISaveFormat;
use game_tool_core::GameBridge;

#[derive(rust_embed::RustEmbed)]
#[folder = "../../profiles"]
struct ProfilesAsset;

fn print_usage() {
    println!("用法: GameSaveEditor --game-dir <路径> [--tcp] [--port <端口>]");
    println!();
    println!("示例:");
    println!("  GameSaveEditor --game-dir \"D:\\Games\\MyRPGMaker\"");
    println!("  GameSaveEditor --game-dir \"D:\\Games\\MyRPGMaker\" --tcp");
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init()
        .ok();

    let args: Vec<String> = std::env::args().collect();
    let game_dir = args.iter().position(|a| a == "--game-dir")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string());
    let do_tcp = args.contains(&"--tcp".to_string());
    let port: u16 = args.iter().position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(19999);

    println!("=== Game Save Editor v{} ===\n", env!("CARGO_PKG_VERSION"));

    match &game_dir {
        Some(dir) => {
            println!("游戏目录: {}", dir);

            let engine = game_tool_detector::detect_by_filesystem(dir);
            println!("检测引擎: {:?}", engine);

            let config = game_tool_rpgmaker::gamedata::scan_game_directory(dir);
            if config.data_loaded {
                println!("游戏标题: {}", config.game_title);
                println!("货币单位: {}", config.currency_unit);
                println!("开关数量: {}", config.switch_names.len());
                println!("变量数量: {}", config.variable_names.len());
                println!("角色数量: {}", config.actor_names.len());
                println!("物品数量: {}", config.item_names.len());
            }

            let save_dir = find_save_dir(dir);
            println!("\n存档目录: {}", save_dir.display());
            if let Some(save_path) = find_latest_save(&save_dir) {
                println!("存档文件: {}", save_path.display());
                let fmt = game_tool_rpgmaker::format::RpgMakerFormat::new();
                match fmt.load(&save_path.to_string_lossy()) {
                    Ok(data) => {
                        let summary = fmt.get_summary(&data);
                        println!();
                        println!("--- 存档摘要 ---");
                        println!("金币:    {}", summary.gold);
                        println!("队伍:    {} 人", summary.party_size);
                        println!("物品:    {} 种", summary.item_count);
                        println!("存档次数: {}", summary.save_count);
                        println!("游戏时间: {}秒", summary.play_time);
                        let members: Vec<_> = summary.members.iter()
                            .filter(|m| !m.is_empty())
                            .collect();
                        if !members.is_empty() {
                            println!("队员:    {}", members.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
                        }

                        println!("\n--- 可修改字段 (前20条) ---");
                        let fields = fmt.scan_fields(&data, dir);
                        for (i, f) in fields.iter().take(20).enumerate() {
                            println!(
                                "  {:3}. [{:12}] {:30} => {}",
                                i + 1, f.category, f.display_name, f.save_value
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
                println!("\n--- TCP 桥接测试 (端口 {}) ---", port);
                let mut bridge = game_tool_rpgmaker::tcp::RpgMakerTcpBridge::new("127.0.0.1", port);
                match bridge.connect() {
                    Ok(()) => {
                        println!("TCP 连接成功!");
                        match bridge.execute(&game_tool_core::BridgeCommand::ReadAll) {
                            Ok(state) => println!("实时状态: {}", state),
                            Err(e) => println!("读取状态失败: {}", e),
                        }
                        bridge.disconnect();
                    }
                    Err(e) => println!("TCP 连接失败: {} (游戏是否正在运行且插件已注入?)", e),
                }
            }
        }
        None => {
            println!("内嵌配置: {} 个 profiles", ProfilesAsset::iter().count());
            println!();
            print_usage();
            println!();
            println!("按 Enter 键退出...");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
    }
}

fn find_save_dir(game_dir: &str) -> std::path::PathBuf {
    let base = Path::new(game_dir);
    for sub in &["www/save", "www/Save", "save", "Save"] {
        let d = base.join(sub);
        if d.is_dir() {
            return d;
        }
    }
    base.join("www/save")
}

fn find_latest_save(save_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut saves: Vec<_> = std::fs::read_dir(save_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            (name.ends_with(".rpgsave") || name.ends_with(".rmmzsave"))
                && !name.contains(".bak.")
                && name != "config.rpgsave"
                && name != "global.rpgsave"
        })
        .map(|e| e.path())
        .collect();
    saves.sort_by_key(|p| {
        std::fs::metadata(p).and_then(|m| m.modified()).ok()
    });
    saves.pop()
}
