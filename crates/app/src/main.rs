//! Game Save Editor — Rust 版本
//!
//! 多功能游戏存档编辑器，支持 RPG Maker MV/MZ, Ren'Py, Unreal Engine, 通用 JSON。

mod registry;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    tracing::info!("Game Save Editor 启动 (Rust)");

    let formats = registry::FormatRegistry::default();
    tracing::info!(
        "已注册格式处理器: {}",
        formats.list_formats().join(", ")
    );

    tracing::info!("运行中...");
}
