//! 桥接线程管理模块：负责实时连接的建立、通信与结果回收。
//!
//! 架构说明：
//! - 每个实时连接运行在独立的线程中，通过 mpsc 通道与 GUI 主线程通信
//! - GUI 主线程通过 cmd_tx 发送 BridgeJob，桥接线程通过 result_tx 返回 BridgeResult
//! - 所有桥接操作在子线程中执行，避免阻塞 GUI 渲染

use crate::factory::create_bridge;
use crate::state::{BridgeJob, BridgeResult, ConnectionStatus, RealtimeConnection};
use game_tool_core::MemoryCommand;
use game_tool_core::detector::EngineType;
use std::sync::mpsc;
use std::thread;

/// 启动一个独立的桥接线程，返回与 GUI 通信的通道结构。
///
/// 工作流程：
/// 1. 创建两个 mpsc 通道（命令通道 cmd 和结果通道 result）
/// 2. spawn 一个新线程，在其中循环接收命令：
///    - Connect → 根据引擎类型创建网桥并连接
///    - Disconnect → 断开并清理网桥
///    - Execute(cmd) → 通过网桥执行命令并返回结果
///    - 通道关闭 → 退出线程
/// 3. 使用 catch_unwind 捕获线程 panic，防止子线程崩溃影响主线程
pub fn spawn_bridge_thread(
    engine_clone: EngineType,
    host: String,
    port: u16,
) -> RealtimeConnection {
    // 命令通道：GUI → 桥接线程
    let (cmd_tx, cmd_rx) = mpsc::channel::<BridgeJob>();
    // 结果通道：桥接线程 → GUI
    let (result_tx, result_rx) = mpsc::channel::<BridgeResult>();

    let host_clone = host.clone();
    thread::spawn(move || {
        // 用 catch_unwind 包裹整个循环，防止线程 panic 导致进程崩溃
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut bridge: Option<Box<dyn game_tool_core::GameBridge>> = None;

            loop {
                match cmd_rx.recv() {
                    Ok(BridgeJob::Connect) => {
                        let h = host_clone.clone();
                        if let Some(mut b) = create_bridge(&engine_clone, &h, port) {
                            match b.connect() {
                                Ok(()) => {
                                    bridge = Some(b);
                                    let _ = result_tx.send(BridgeResult::Connected);
                                }
                                Err(e) => {
                                    let _ = result_tx.send(BridgeResult::Error(e.to_string()));
                                }
                            }
                        } else {
                            let _ =
                                result_tx.send(BridgeResult::Error("该引擎不支持实时连接".into()));
                        }
                    }
                    Ok(BridgeJob::Disconnect) => {
                        // 断开现有连接并清理网桥对象
                        if let Some(ref mut b) = bridge {
                            b.disconnect();
                        }
                        bridge = None;
                        let _ = result_tx.send(BridgeResult::Disconnected);
                    }
                    Ok(BridgeJob::Execute(cmd)) => match &mut bridge {
                        Some(b) => match b.execute(&cmd) {
                            Ok(val) => {
                                let _ = result_tx.send(BridgeResult::CommandResult(val));
                            }
                            Err(e) => {
                                let _ = result_tx.send(BridgeResult::Error(e.to_string()));
                            }
                        },
                        None => {
                            let _ = result_tx.send(BridgeResult::Error("未连接".into()));
                        }
                    },
                    Ok(BridgeJob::MemoryCommand(mem_cmd)) => match &mut bridge {
                        Some(b) => match b.handle_memory_command(&mem_cmd) {
                            Ok(val) => {
                                // 根据命令类型路由到不同的 BridgeResult 变体
                                let result = match &mem_cmd {
                                    MemoryCommand::Attach(_) => BridgeResult::Attached,
                                    MemoryCommand::FirstScan { .. } | MemoryCommand::NextScan { .. } => {
                                        BridgeResult::ScanResult(val)
                                    }
                                    MemoryCommand::SeedFromSave(_) | MemoryCommand::CrossValidate { .. } => {
                                        BridgeResult::SeedResult(val)
                                    }
                                    _ => BridgeResult::CommandResult(val),
                                };
                                let _ = result_tx.send(result);
                            }
                            Err(e) => {
                                let _ = result_tx.send(BridgeResult::Error(e.to_string()));
                            }
                        },
                        None => {
                            let _ = result_tx.send(BridgeResult::Error("未连接".into()));
                        }
                    },
                    Err(_) => {
                        // 通道已关闭（GUI 端断开），清理资源并退出
                        if let Some(ref mut b) = bridge {
                            b.disconnect();
                        }
                        let _ = result_tx.send(BridgeResult::Disconnected);
                        break;
                    }
                }
            }
        }));

        // 线程 panic 后的兜底处理
        if let Err(_panic) = result {
            let _ = result_tx.send(BridgeResult::Error("连接异常断开".into()));
            let _ = result_tx.send(BridgeResult::Disconnected);
        }
    });

    RealtimeConnection {
        cmd_tx,
        result_rx,
        status: ConnectionStatus::Disconnected,
    }
}

/// 从通道中非阻塞地提取所有待处理的结果
///
/// 使用 try_recv() 而非 recv()，确保不会阻塞 GUI 线程。
/// 每次 update() 调用时都会调用此函数，批量处理积累的结果。
pub fn drain_results(conn: &mut RealtimeConnection) -> Vec<BridgeResult> {
    let mut results = Vec::new();
    while let Ok(r) = conn.result_rx.try_recv() {
        results.push(r);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{BridgeJob, BridgeResult, ConnectionStatus, RealtimeConnection};
    use std::sync::mpsc;

    fn make_test_connection() -> RealtimeConnection {
        let (cmd_tx, _cmd_rx) = mpsc::channel();
        let (_result_tx, result_rx) = mpsc::channel();
        RealtimeConnection {
            cmd_tx,
            result_rx,
            status: ConnectionStatus::Disconnected,
        }
    }

    #[test]
    fn test_drain_results_empty() {
        let mut conn = make_test_connection();
        let results = drain_results(&mut conn);
        assert!(results.is_empty());
    }

    #[test]
    fn test_drain_results_single() {
        let (cmd_tx, _) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        result_tx.send(BridgeResult::Connected).unwrap();
        drop(result_tx);

        let mut conn = RealtimeConnection {
            cmd_tx,
            result_rx,
            status: ConnectionStatus::Disconnected,
        };
        let results = drain_results(&mut conn);
        assert_eq!(results.len(), 1);
        match &results[0] {
            BridgeResult::Connected => {}
            _ => panic!("Expected Connected"),
        }
    }

    #[test]
    fn test_drain_results_multiple() {
        let (cmd_tx, _) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        result_tx.send(BridgeResult::Connected).unwrap();
        result_tx
            .send(BridgeResult::CommandResult(serde_json::Value::String(
                "ok".into(),
            )))
            .unwrap();
        result_tx.send(BridgeResult::Disconnected).unwrap();
        drop(result_tx);

        let mut conn = RealtimeConnection {
            cmd_tx,
            result_rx,
            status: ConnectionStatus::Disconnected,
        };
        let results = drain_results(&mut conn);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_drain_results_error() {
        let (cmd_tx, _) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        result_tx
            .send(BridgeResult::Error("connection refused".into()))
            .unwrap();
        drop(result_tx);

        let mut conn = RealtimeConnection {
            cmd_tx,
            result_rx,
            status: ConnectionStatus::Disconnected,
        };
        let results = drain_results(&mut conn);
        assert_eq!(results.len(), 1);
        match &results[0] {
            BridgeResult::Error(e) => assert_eq!(e, "connection refused"),
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_spawn_bridge_thread_initial_status() {
        let conn = spawn_bridge_thread(
            game_tool_core::detector::EngineType::Unknown,
            "127.0.0.1".into(),
            19999,
        );
        assert_eq!(conn.status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_spawn_bridge_thread_supported_engine() {
        let conn = spawn_bridge_thread(
            game_tool_core::detector::EngineType::RpgMakerMv,
            "127.0.0.1".into(),
            19999,
        );
        assert_eq!(conn.status, ConnectionStatus::Disconnected);
        // Connection should be possible (bridge created but not connected yet)
        let _ = conn.cmd_tx.send(BridgeJob::Connect);
        // Don't wait for result since no real server — just verify no panic
    }
}
