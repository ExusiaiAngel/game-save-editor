//! game-tool-app: 应用入口

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    // 桥接 log crate 到 tracing
    tracing_log::LogTracer::init()
        .context("初始化 tracing-log 桥接失败")?;

    // 初始化 tracing-subscriber: JSON 格式 + env-filter
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("游戏存档编辑器启动");

    Ok(())
}
