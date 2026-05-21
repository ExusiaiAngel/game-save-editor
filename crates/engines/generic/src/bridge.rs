//! 通用内存桥接（Windows API 内存扫描 + R/W）

use serde_json::Value;
use game_tool_core::{GameBridge, BridgeCommand};
use game_tool_core::error::GameToolError;

pub struct GenericMemoryBridge;

impl Default for GenericMemoryBridge {
    fn default() -> Self { Self }
}

impl GenericMemoryBridge {
    pub fn new() -> Self { Self }
}

impl GameBridge for GenericMemoryBridge {
    fn connect(&mut self) -> Result<(), GameToolError> {
        Err(GameToolError::BridgeConnectError("通用内存桥接尚未实现".into()))
    }

    fn disconnect(&mut self) {}
    fn is_connected(&self) -> bool { false }

    fn execute(&mut self, _cmd: &BridgeCommand) -> Result<Value, GameToolError> {
        Err(GameToolError::BridgeCommandError("通用内存桥接尚未实现".into()))
    }

    fn engine_name(&self) -> &str { "generic_memory" }
    fn priority(&self) -> i32 { 90 }
}
