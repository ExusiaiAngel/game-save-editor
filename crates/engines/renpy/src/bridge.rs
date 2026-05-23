//! Ren'Py TCP JSON 桥接模块。
//!
//! 通过 TCP 连接与 Ren'Py 游戏进程通信，执行读取/写入操作。
//! 包含桥接客户端（`RenPyBridge`）和 Python 插件注入代码。

use std::collections::HashMap;
use std::sync::Mutex;

use game_tool_core::net::TcpLineConnection;
use game_tool_core::{BridgeCommand, GameBridge, GameState, GameToolError};
use serde_json::{json, Value};

/// Ren'Py TCP JSON 桥接客户端。
///
/// 与游戏中运行的 Python TCP 服务器通信，通过 JSON 消息执行
/// `get_state`、`get_var`、`set_var`、`eval` 等操作。
/// 支持对 Ren'Py store 中的任意变量进行实时读写。
pub struct RenPyBridge {
    /// TCP 连接（Mutex 保护，跨线程访问）
    conn: Mutex<Option<TcpLineConnection>>,
    /// 目标主机地址
    host: String,
    /// 目标端口
    port: u16,
}

impl RenPyBridge {
    /// 创建新的 Ren'Py 桥接实例。
    ///
    /// 需调用 `connect()` 后才可用于通信。
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            conn: Mutex::new(None),
            host: host.to_string(),
            port,
        }
    }

    /// 向游戏端发送 JSON 请求并接收响应。
    ///
    /// JSON 消息格式: `{"action": "<action>", "params": {...}}`
    /// 响应对应的 JSON 对象。
    fn send_json(&self, action: &str, params: Value) -> Result<Value, GameToolError> {
        // 获取 Mutex 锁并检查连接状态
        let mut guard = self
            .conn
            .lock()
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))?;
        let conn = guard
            .as_mut()
            .ok_or_else(|| GameToolError::BridgeConnectError("未连接".into()))?;
        // 构造标准 JSON 请求消息
        let msg = json!({"action": action, "params": params});
        let msg_str = serde_json::to_string(&msg)
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        // 发送 JSON 行到 TCP 连接
        conn.send_line(&msg_str)
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        // 接收响应行并解析为 JSON
        let resp = conn
            .recv_line()
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        serde_json::from_str(&resp).map_err(|e| GameToolError::BridgeCommandError(e.to_string()))
    }
}

impl GameBridge for RenPyBridge {
    /// 建立 TCP 连接到指定主机和端口
    fn connect(&mut self) -> Result<(), GameToolError> {
        let conn = TcpLineConnection::connect(&format!("{}:{}", self.host, self.port))
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))?;
        *self
            .conn
            .lock()
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))? = Some(conn);
        Ok(())
    }

    /// 断开 TCP 连接
    fn disconnect(&mut self) {
        if let Ok(mut guard) = self.conn.lock() {
            if let Some(ref mut conn) = *guard {
                conn.disconnect();
            }
            *guard = None;
        }
    }

    /// 检查是否已建立连接
    fn is_connected(&self) -> bool {
        self.conn.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// 执行桥接命令。
    ///
    /// - `ReadAll`: 获取完整的游戏状态（store 变量），返回包含 `store` 扩展的 `GameState`
    /// - `ReadField`: 通过 `get_var` 读取单个 Ren'Py store 变量值
    /// - `WriteField`: 通过 `set_var` 设置单个 Ren'Py store 变量值
    fn execute(&mut self, cmd: &BridgeCommand) -> Result<Value, GameToolError> {
        match cmd {
            BridgeCommand::ReadAll => {
                // 请求完整的游戏状态（所有 store 变量的快照）
                let resp = self.send_json("get_state", json!({}))?;
                // 提取 store 对象，若缺失则使用空对象
                let store = resp
                    .get("store")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                let mut extensions = HashMap::new();
                extensions.insert("store".into(), store);
                // 包装为 GameState 结构并返回
                Ok(serde_json::to_value(GameState {
                    engine: "renpy".into(),
                    extensions,
                    ..Default::default()
                })
                .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?)
            }
            BridgeCommand::ReadField(field_id) => {
                // 将 field_id 映射为 Ren'Py store 变量名后读取
                let var_name = Self::field_to_var(field_id);
                let resp = self.send_json("get_var", json!({"name": var_name}))?;
                Ok(resp.get("value").cloned().unwrap_or(Value::Null))
            }
            BridgeCommand::WriteField(field_id, value) => {
                // 将 field_id 映射为 Ren'Py store 变量名后写入
                let var_name = Self::field_to_var(field_id);
                self.send_json("set_var", json!({"name": var_name, "value": value}))?;
                Ok(Value::String("ok".into()))
            }
        }
    }

    fn engine_name(&self) -> &str {
        "renpy"
    }
    fn priority(&self) -> i32 {
        50
    }
}

impl RenPyBridge {
    /// 将字段标识符映射为 Ren'Py 中的 store 变量名。
    ///
    /// 将外部工具使用的通用字段 ID 转换为 Ren'Py 实际 store 变量名。
    /// 特殊映射：
    /// - `gold` → `money`（常见 Ren'Py 项目用 money 表示金币）
    /// - `gold_alt` → `gold`（备用金币变量名）
    /// - `var_xxx` → `xxx`（去掉 `var_` 前缀，用于一般变量）
    fn field_to_var(field_id: &str) -> &str {
        match field_id {
            // 工具层统一用 "gold" 表示金币，但 Ren'Py 默认变量名是 money
            "gold" => "money",
            // 部分项目可能直接用 gold 作为变量名
            "gold_alt" => "gold",
            // 通用前缀映射：var_xxx → xxx
            s if s.starts_with("var_") => &s[4..],
            // 无前缀匹配时原样返回
            s => s,
        }
    }
}

// ── 插件注入 ──────────────────────────────────────────────────────────

use std::fs;
use std::path::{Path, PathBuf};

/// Ren'Py 桥接服务器插件源码（Python）。
///
/// 作为 Ren'Py 游戏内的 Python 模块运行，提供 TCP 服务器
/// 用于外部工具通过 JSON 命令读写游戏状态。
pub const PLUGIN_CODE: &str = r#"
import socket
import threading
import json

class GameBridgeServer:
    def __init__(self, host='127.0.0.1', port=19999):
        self.host = host
        self.port = port
        self.running = True
        self.server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            self.server.bind((host, port))
            self.server.listen(1)
            self.server.settimeout(1)
        except:
            pass

    def start(self):
        thread = threading.Thread(target=self._run, daemon=True)
        thread.start()

    def _run(self):
        while self.running:
            try:
                client, addr = self.server.accept()
                self._handle(client)
            except socket.timeout:
                continue
            except:
                break

    def _handle(self, client):
        client.settimeout(10)
        data = b''
        while True:
            try:
                chunk = client.recv(4096)
                if not chunk: break
                data += chunk
                if b'\n' in data: break
            except: break
        try:
            msg = json.loads(data.decode('utf-8').strip())
            action = msg.get('action', '')
            params = msg.get('params', {})
            if action == 'ping':
                client.send(json.dumps({'status':'ok'}).encode())
            elif action == 'get_state':
                import renpy
                store_vars = {k:v for k,v in renpy.store.__dict__.items() if not k.startswith('_')}
                client.send(json.dumps({'store':store_vars}).encode())
            elif action == 'get_var':
                import renpy
                name = params.get('name','')
                val = getattr(renpy.store, name, None)
                client.send(json.dumps({'value':val}).encode())
            elif action == 'set_var':
                import renpy
                name = params.get('name','')
                val = params.get('value')
                setattr(renpy.store, name, val)
                client.send(json.dumps({'status':'ok'}).encode())
            elif action == 'eval':
                code = params.get('code','')
                exec(code, {'store':renpy.store})
                client.send(json.dumps({'status':'ok'}).encode())
        except Exception as e:
            client.send(json.dumps({'error':str(e)}).encode())
        finally:
            client.close()

try:
    bridge = GameBridgeServer()
    bridge.start()
except:
    pass
"#;

/// 检查桥接插件是否已安装。
///
/// 验证 `game/python-packages/tcp_bridge/__init__.py` 文件存在于游戏目录中。
pub fn is_plugin_installed(game_dir: &str) -> bool {
    // 检查目标插件文件是否存在
    get_plugin_target(game_dir).is_file()
}

/// 注入桥接插件到游戏目录。
///
/// 在 `game/python-packages/tcp_bridge/` 下创建 `__init__.py`，
/// 内容为桥接服务器的 Python 源码。如果父目录不存在则自动创建。
pub fn inject_plugin(game_dir: &str) -> Result<(), String> {
    let target = get_plugin_target(game_dir);
    // 确保插件所在目录存在，不存在则递归创建
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // 写入 Python 桥接服务器源码
    fs::write(&target, PLUGIN_CODE).map_err(|e| e.to_string())?;
    Ok(())
}

/// 移除桥接插件文件。
///
/// 删除 `game/python-packages/tcp_bridge/` 目录（如果为目录）或文件。
/// 支持目录和文件两种形态的清理。
pub fn remove_plugin(game_dir: &str) -> Result<(), String> {
    let target = get_plugin_target(game_dir);
    // 根据目标类型选择不同的删除方式
    if target.is_dir() {
        // 递归删除整个插件包目录
        fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    } else if target.is_file() {
        // 删除单个插件文件
        fs::remove_file(&target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 获取插件目标文件路径。
///
/// 返回 `{game_dir}/game/python-packages/tcp_bridge/__init__.py`。
/// Ren'Py 会将 `python-packages` 中的包自动导入，便于桥接插件加载。
fn get_plugin_target(game_dir: &str) -> PathBuf {
    Path::new(game_dir).join("game/python-packages/tcp_bridge/__init__.py")
}
