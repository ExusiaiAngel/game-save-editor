//! Unreal Engine 内存桥接（Windows API）
//!
//! 通过 ReadProcessMemory / WriteProcessMemory 读写游戏内存。

use serde_json::Value;
use game_tool_core::{GameBridge, BridgeCommand};
use game_tool_core::error::GameToolError;

pub struct UnrealMemoryBridge;

impl Default for UnrealMemoryBridge {
    fn default() -> Self { Self }
}

impl UnrealMemoryBridge {
    pub fn new() -> Self { Self }
}

impl GameBridge for UnrealMemoryBridge {
    fn connect(&mut self) -> Result<(), GameToolError> {
        Err(GameToolError::BridgeConnectError("Unreal 内存桥接尚未实现".into()))
    }

    fn disconnect(&mut self) {}
    fn is_connected(&self) -> bool { false }

    fn execute(&mut self, _cmd: &BridgeCommand) -> Result<Value, GameToolError> {
        Err(GameToolError::BridgeCommandError("Unreal 内存桥接尚未实现".into()))
    }

    fn engine_name(&self) -> &str { "unreal_memory" }
    fn priority(&self) -> i32 { 40 }
}
