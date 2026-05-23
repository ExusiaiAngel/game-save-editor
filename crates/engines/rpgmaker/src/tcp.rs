//! RPG Maker TCP 桥接（文本命令协议）。
//!
//! 通过 TCP 连接与 RPG Maker 游戏进程中的 NW.js 插件通信，
//! 使用纯文本命令格式实现游戏状态的实时读写。

use std::collections::HashMap;
use std::sync::Mutex;

use game_tool_core::net::TcpLineConnection;
use game_tool_core::{BridgeCommand, GameBridge, GameState, GameToolError};
use serde_json::Value;

/// RPG Maker TCP 桥接客户端。
///
/// 与 NW.js 游戏中的 JavaScript 插件（GameBridgeServer.js）通信，
/// 使用换行分隔的文本命令格式（如 `get_state`、`set_gold 9999` 等）。
pub struct RpgMakerTcpBridge {
    /// TCP 连接（Mutex 保护，跨线程访问）
    conn: Mutex<Option<TcpLineConnection>>,
    /// 目标主机地址
    host: String,
    /// 目标端口
    port: u16,
}

impl RpgMakerTcpBridge {
    /// 创建新的 RPG Maker TCP 桥接实例。
    ///
    /// 需调用 `connect()` 后才可用于通信。
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            conn: Mutex::new(None),
            host: host.to_string(),
            port,
        }
    }

    /// 向游戏端发送文本命令并接收响应。
    ///
    /// 命令为纯文本格式，以换行符分隔。响应也为纯文本行。
    fn send_cmd(&self, cmd: &str) -> Result<String, GameToolError> {
        let mut guard = self
            .conn
            .lock()
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))?;
        let conn = guard
            .as_mut()
            .ok_or_else(|| GameToolError::BridgeConnectError("未连接".into()))?;
        // 发送命令并等待响应
        conn.send_line(cmd)
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        conn.recv_line()
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))
    }

    /// 解析 `STATE:...` 格式的响应为 `GameState`。
    ///
    /// 响应格式: `STATE:{ "gold": 500, "switches": {...}, ... }`
    fn parse_state_response(resp: &str) -> Result<GameState, GameToolError> {
        let json_str = resp
            .strip_prefix("STATE:")
            .ok_or_else(|| GameToolError::BridgeCommandError("无效响应".into()))?;
        let raw: Value = serde_json::from_str(json_str)
            .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
        let mut extensions = HashMap::new();
        // 将各项数据放入 extensions
        if let Some(sw) = raw.get("switches") {
            extensions.insert("switches".into(), sw.clone());
        }
        if let Some(vars) = raw.get("variables") {
            extensions.insert("variables".into(), vars.clone());
        }
        if let Some(party) = raw.get("party") {
            extensions.insert("party".into(), party.clone());
        }
        if let Some(items) = raw.get("items") {
            extensions.insert("items".into(), items.clone());
        }
        if let Some(ss) = raw.get("selfSwitches") {
            extensions.insert("selfSwitches".into(), ss.clone());
        }
        // 金币单独提取为数字值
        extensions.insert(
            "gold".into(),
            Value::Number(raw.get("gold").and_then(|v| v.as_i64()).unwrap_or(0).into()),
        );
        Ok(GameState {
            engine: "rpg_maker".into(),
            map_name: raw
                .get("mapName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            play_time: raw
                .get("playtime")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            save_count: raw.get("saveCount").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            extensions,
        })
    }
}

impl GameBridge for RpgMakerTcpBridge {
    /// 建立 TCP 连接
    fn connect(&mut self) -> Result<(), GameToolError> {
        let conn = TcpLineConnection::connect(&format!("{}:{}", self.host, self.port))
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))?;
        *self
            .conn
            .lock()
            .map_err(|e| GameToolError::BridgeConnectError(e.to_string()))? = Some(conn);
        Ok(())
    }

    /// 断开连接（先发送 `close` 命令通知对端）
    fn disconnect(&mut self) {
        if let Ok(mut guard) = self.conn.lock() {
            if let Some(ref mut conn) = *guard {
                let _ = conn.send_line("close");
                conn.disconnect();
            }
            *guard = None;
        }
    }

    /// 检查连接状态
    fn is_connected(&self) -> bool {
        self.conn
            .lock()
            .map(|g| g.as_ref().is_some_and(|c| c.is_connected()))
            .unwrap_or(false)
    }

    /// 执行桥接命令。
    ///
    /// 支持的命令类型：
    /// - **ReadAll**: `get_state` → 解析为完整 `GameState`
    /// - **ReadField**: 按 `field_id` 从 `get_state` 响应中提取值
    /// - **WriteField**: 根据字段类型发送对应的 set 命令
    ///
    /// 支持的 set 命令：
    /// - `set_gold <amount>`
    /// - `set_switch <id> <0|1>`
    /// - `set_variable <id> <value>`
    /// - `set_hp <id> <value>` / `set_mp <id> <value>` / `set_level <id> <value>`
    /// - `set_item <id> <count>`
    /// - `set_self_switch <key> <0|1>`
    fn execute(&mut self, cmd: &BridgeCommand) -> Result<Value, GameToolError> {
        match cmd {
            BridgeCommand::ReadAll => {
                let resp = self.send_cmd("get_state")?;
                let state = Self::parse_state_response(&resp)?;
                serde_json::to_value(state)
                    .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))
            }
            BridgeCommand::ReadField(field_id) => {
                // 复用 get_state 响应，提取目标字段
                let resp = self.send_cmd("get_state")?;
                let json_str = resp
                    .strip_prefix("STATE:")
                    .ok_or_else(|| GameToolError::BridgeCommandError("无效响应".into()))?;
                let raw: Value = serde_json::from_str(json_str)
                    .map_err(|e| GameToolError::BridgeCommandError(e.to_string()))?;
                // 根据 field_id 前缀选择读取路径
                if field_id == "gold" {
                    Ok(raw.get("gold").cloned().unwrap_or(Value::Null))
                } else if let Some(id_str) = field_id.strip_prefix("switch_") {
                    let id = id_str.to_string();
                    Ok(raw
                        .get("switches")
                        .and_then(|s| s.get(&id))
                        .cloned()
                        .unwrap_or(Value::Null))
                } else if let Some(id_str) = field_id.strip_prefix("var_") {
                    let id = id_str.to_string();
                    Ok(raw
                        .get("variables")
                        .and_then(|v| v.get(&id))
                        .cloned()
                        .unwrap_or(Value::Null))
                } else {
                    Ok(Value::Null)
                }
            }
            BridgeCommand::WriteField(field_id, value) => {
                // 检查响应是否以 "OK" 开头
                let check_ok = |resp: &str| {
                    if resp.starts_with("OK") {
                        Ok(Value::String("ok".into()))
                    } else {
                        Err(GameToolError::BridgeCommandError(resp.into()))
                    }
                };
                // 根据 field_id 分发到对应的 set 命令
                if field_id == "gold" {
                    let n = value.as_i64().unwrap_or(0);
                    check_ok(&self.send_cmd(&format!("set_gold {}", n))?)
                } else if let Some(id_str) = field_id.strip_prefix("switch_") {
                    let id: i32 = id_str.parse().unwrap_or(0);
                    let v = if value.as_bool().unwrap_or(false) {
                        1
                    } else {
                        0
                    };
                    check_ok(&self.send_cmd(&format!("set_switch {} {}", id, v))?)
                } else if let Some(id_str) = field_id.strip_prefix("var_") {
                    let id: i32 = id_str.parse().unwrap_or(0);
                    let v = value.as_i64().unwrap_or(0);
                    check_ok(&self.send_cmd(&format!("set_variable {} {}", id, v))?)
                } else if let Some(rest) = field_id.strip_prefix("actor_") {
                    // 解析 actor_<id>_<stat> 格式
                    let parts: Vec<&str> = rest.splitn(2, '_').collect();
                    if parts.len() == 2 {
                        let id: i32 = parts[0].parse().unwrap_or(0);
                        let v = value.as_i64().unwrap_or(0);
                        let cmd = match parts[1] {
                            "hp" => format!("set_hp {} {}", id, v),
                            "mp" => format!("set_mp {} {}", id, v),
                            "level" => format!("set_level {} {}", id, v),
                            _ => {
                                return Err(GameToolError::BridgeCommandError(format!(
                                    "未知属性: {}",
                                    parts[1]
                                )))
                            }
                        };
                        check_ok(&self.send_cmd(&cmd)?)
                    } else {
                        Err(GameToolError::BridgeCommandError("无效actor字段".into()))
                    }
                } else if let Some(id_str) = field_id.strip_prefix("item_") {
                    let id: i32 = id_str.parse().unwrap_or(0);
                    let count = value.as_i64().unwrap_or(0);
                    check_ok(&self.send_cmd(&format!("set_item {} {}", id, count))?)
                } else if let Some(key) = field_id.strip_prefix("ss_") {
                    let v = if value.as_bool().unwrap_or(false) {
                        1
                    } else {
                        0
                    };
                    check_ok(&self.send_cmd(&format!("set_self_switch {} {}", key, v))?)
                } else {
                    Err(GameToolError::BridgeCommandError(format!(
                        "不支持的字段: {}",
                        field_id
                    )))
                }
            }
        }
    }

    fn engine_name(&self) -> &str {
        "rpg_maker_tcp"
    }
    fn priority(&self) -> i32 {
        10
    }
}

// ── 插件注入 ──────────────────────────────────────────────────────────

use std::fs;
use std::path::{Path, PathBuf};

/// 插件文件名
const PLUGIN_FILENAME: &str = "GameBridgeServer.js";
/// plugins.js 文件路径
const PLUGINS_JS: &str = "js/plugins.js";

/// GameBridgeServer.js 插件源码（NW.js TCP 服务器）。
///
/// 在 RPG Maker NW.js 运行时中创建 TCP 服务器，
/// 暴露游戏对象（`$gameParty`、`$gameSwitches`、`$gameVariables` 等）
/// 供外部工具通过文本命令访问。
///
/// 支持的命令：
/// - `ping` → `PONG`
/// - `get_state` → `STATE:{"gold":..., "switches":{...}, ...}`
/// - `set_gold <n>` → `OK`
/// - `set_switch <id> <0|1>` → `OK`
/// - `set_variable <id> <n>` → `OK`
/// - `set_hp <id> <n>` / `set_mp <id> <n>` / `set_level <id> <n>` → `OK`
/// - `set_item <id> <n>` → `OK`
/// - `set_self_switch <key> <0|1>` → `OK`
pub const PLUGIN_SOURCE: &str = r#"
(function() {
try {
var net = require('net');
var server = net.createServer(function(socket) {
    socket.setEncoding('utf8');
    var buffer = '';
    socket.on('data', function(data) {
        buffer += data;
        var newline = buffer.indexOf('\n');
        while (newline >= 0) {
            var cmd = buffer.substring(0, newline).trim();
            buffer = buffer.substring(newline + 1);
            try { handleCommand(socket, cmd); } catch(e) {}
            newline = buffer.indexOf('\n');
        }
    });
});
function handleCommand(socket, cmd) {
    var parts = cmd.split(' ');
    var action = parts[0];
    if (action === 'ping') { socket.write('PONG\n'); return; }
    if (action === 'get_state') {
        var state = {
            gold: $gameParty ? $gameParty._gold : 0,
            steps: $gameParty ? $gameParty._steps : 0,
            switches: {},
            variables: {},
            party: (function() {
                var members = [];
                if ($gameParty) {
                    for (var i = 0; i < $gameParty.members().length; i++) {
                        var a = $gameParty.members()[i];
                        members.push({_actorId: a._actorId, _hp: a._hp, _mp: a._mp, _level: a._level, _name: a._name});
                    }
                }
                return members;
            })(),
            items: (function() {
                var r = {};
                if ($gameParty && $gameParty._items)
                    for (var k in $gameParty._items)
                        if ($gameParty._items.hasOwnProperty(k) && $gameParty._items[k] > 0)
                            r[k] = $gameParty._items[k];
                return r;
            })(),
            selfSwitches: {},
            mapName: $gameMap ? $gameMap.displayName() : '',
            playtime: $gameSystem ? $gameSystem.playtimeText() : '',
            saveCount: $gameSystem ? $gameSystem.saveCount() : 0
        };
        for (var i = 1; i < $dataSystem.switches.length; i++) state.switches[i] = $gameSwitches.value(i);
        for (var i = 1; i < $dataSystem.variables.length; i++) state.variables[i] = $gameVariables.value(i);
        if ($gameSelfSwitches && $gameSelfSwitches._data) {
            for (var key in $gameSelfSwitches._data)
                if ($gameSelfSwitches._data.hasOwnProperty(key))
                    state.selfSwitches[key] = $gameSelfSwitches._data[key] === true;
        }
        socket.write('STATE:' + JSON.stringify(state) + '\n');
        return;
    }
    if (action === 'set_gold') { $gameParty._gold = parseInt(parts[1]); socket.write('OK\n'); return; }
    if (action === 'set_switch') { $gameSwitches.setValue(parseInt(parts[1]), parts[2] === '1'); socket.write('OK\n'); return; }
    if (action === 'set_variable') { $gameVariables.setValue(parseInt(parts[1]), parseInt(parts[2])); socket.write('OK\n'); return; }
    if (action === 'set_hp') { $gameActors.actor(parseInt(parts[1]))._hp = parseInt(parts[2]); socket.write('OK\n'); return; }
    if (action === 'set_mp') { $gameActors.actor(parseInt(parts[1]))._mp = parseInt(parts[2]); socket.write('OK\n'); return; }
    if (action === 'set_level') { $gameActors.actor(parseInt(parts[1]))._level = parseInt(parts[2]); socket.write('OK\n'); return; }
    if (action === 'set_item') { $gameParty._items[parseInt(parts[1])] = parseInt(parts[2]); socket.write('OK\n'); return; }
    if (action === 'set_self_switch') { var ssKey = parts.slice(1, -1).join(' '); $gameSelfSwitches.setValue(ssKey, parts[parts.length-1] === '1'); socket.write('OK\n'); return; }
    socket.write('ERR\n');
}
server.listen(__PORT__, '127.0.0.1');
} catch(e) {}
})();
"#;

/// 检查插件是否已安装。
///
/// 验证两个条件：
/// 1. `GameBridgeServer.js` 文件存在
/// 2. `plugins.js` 中包含该插件的引用
pub fn is_plugin_installed(game_dir: &str) -> bool {
    // 检查 GameBridgeServer.js 插件文件是否存在于目标目录
    let plugin_path = find_plugin_file(game_dir);
    if !plugin_path.is_file() {
        return false;
    }
    // 检查 plugins.js 配置中是否已注册该插件
    let plugins_js = find_plugins_js(game_dir);
    if !plugins_js.is_file() {
        return false;
    }
    // 读取 plugins.js 内容并搜索插件文件名
    if let Ok(content) = fs::read_to_string(&plugins_js) {
        content.contains(PLUGIN_FILENAME)
    } else {
        false
    }
}

/// 注入插件到游戏目录。
///
/// 操作流程：
/// 1. 创建 `www/js/plugins/GameBridgeServer.js`（替换 `__PORT__` 为实际端口）
/// 2. 修改 `www/js/plugins.js`，在插件数组中追加新条目
/// 3. 修改前自动备份 `plugins.js` 为 `plugins.js.bak`
///
/// 如果修改 `plugins.js` 失败，自动从备份还原。
pub fn inject_plugin(game_dir: &str, port: u16) -> Result<(), String> {
    let www_dir = Path::new(game_dir).join("www");
    let js_plugins = www_dir.join("js/plugins");
    fs::create_dir_all(&js_plugins).map_err(|e| e.to_string())?;

    // 写入插件文件（替换端口占位符）
    let plugin_path = js_plugins.join(PLUGIN_FILENAME);
    let source_with_port = PLUGIN_SOURCE.replace("__PORT__", &port.to_string());
    fs::write(&plugin_path, &source_with_port).map_err(|e| e.to_string())?;

    // 修改 plugins.js
    let plugins_js = www_dir.join(PLUGINS_JS);
    if plugins_js.is_file() {
        let content = fs::read_to_string(&plugins_js).map_err(|e| e.to_string())?;
        if !content.contains(PLUGIN_FILENAME) {
            // 修改前备份
            let bak = plugins_js.with_extension("js.bak");
            if !bak.exists() {
                fs::copy(&plugins_js, &bak).map_err(|e| e.to_string())?;
            }

            let result = modify_plugins_js(&plugins_js, &content);
            if let Err(e) = result {
                // 失败时自动还原备份
                if bak.exists() {
                    let _ = fs::copy(&bak, &plugins_js);
                }
                return Err(format!("修改 plugins.js 失败: {}，已自动还原备份", e));
            }
        }
    }

    Ok(())
}

/// 修改 plugins.js 文件，追加 GameBridgeServer 插件条目。
///
/// 解析现有插件数组 `[{...}]`，追加新条目后写回。
fn modify_plugins_js(path: &Path, content: &str) -> Result<(), String> {
    // 定位数组的起始和结束位置
    let left = content.find('[')
        .ok_or_else(|| "plugins.js 格式不支持：找不到数组开始 '['".to_string())?;
    let right = content.rfind(']')
        .ok_or_else(|| "plugins.js 格式不支持：找不到数组结束 ']'".to_string())?;

    let prefix = &content[..=left];
    let suffix = &content[right..];
    let array_body = &content[left + 1..right];

    // 解析现有插件数组
    let mut plugins: Vec<serde_json::Value> =
        serde_json::from_str(&format!("[{}]", array_body))
        .map_err(|e| format!("plugins.js JSON 解析失败: {}", e))?;

    // 追加新插件条目
    let entry = serde_json::json!({
        "name": "GameBridgeServer",
        "status": true,
        "description": "TCP Bridge",
        "parameters": {}
    });
    plugins.push(entry);

    // 重新组装文件内容
    let entries: Vec<String> = plugins.iter()
        .map(|v| serde_json::to_string(v).unwrap_or_default())
        .collect();
    let new_content = format!("{}\n{}\n{}", prefix, entries.join(",\n"), suffix);
    fs::write(path, &new_content).map_err(|e| e.to_string())
}

/// 移除插件。
///
/// 删除 `GameBridgeServer.js` 并从备份文件还原 `plugins.js`。
pub fn remove_plugin(game_dir: &str) -> Result<(), String> {
    let plugin_path = find_plugin_file(game_dir);
    if plugin_path.is_file() {
        fs::remove_file(&plugin_path).map_err(|e| e.to_string())?;
    }
    // 从备份恢复 plugins.js
    let plugins_js = Path::new(game_dir).join("www/js/plugins.js");
    let plugins_js_bak = plugins_js.with_extension("js.bak");
    if plugins_js_bak.exists() {
        fs::copy(&plugins_js_bak, &plugins_js).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 获取插件文件路径: `{game_dir}/www/js/plugins/GameBridgeServer.js`
fn find_plugin_file(game_dir: &str) -> PathBuf {
    Path::new(game_dir)
        .join("www/js/plugins")
        .join(PLUGIN_FILENAME)
}

/// 获取 plugins.js 文件路径: `{game_dir}/www/js/plugins.js`
fn find_plugins_js(game_dir: &str) -> PathBuf {
    Path::new(game_dir).join("www/js/plugins.js")
}
