//! Game Save Editor — Rust + egui 版本
//!
//! 多功能游戏存档编辑器，支持 RPG Maker MV/MZ, Ren'Py, Unreal Engine, 通用 JSON。

mod registry;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_log::LogTracer::init().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    tracing::info!("Game Save Editor 启动 (Rust)");

    let _formats = registry::FormatRegistry::default();
    tracing::info!(
        "已注册格式处理器: {}",
        _formats.list_formats().join(", ")
    );

    // TODO: egui GUI (后续 Plan 实施)
    tracing::info!("运行中...");
}
