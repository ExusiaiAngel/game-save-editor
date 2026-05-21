//! Ren'Py 插件注入（TCP Bridge Python 模块）

use std::fs;
use std::path::{Path, PathBuf};

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

/// 检查插件是否已安装
pub fn is_plugin_installed(game_dir: &str) -> bool {
    get_plugin_target(game_dir).is_file()
}

/// 注入插件
pub fn inject_plugin(game_dir: &str) -> Result<(), String> {
    let target = get_plugin_target(game_dir);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&target, PLUGIN_CODE).map_err(|e| e.to_string())?;
    Ok(())
}

/// 移除插件
pub fn remove_plugin(game_dir: &str) -> Result<(), String> {
    let target = get_plugin_target(game_dir);
    if target.is_dir() {
        fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    } else if target.is_file() {
        fs::remove_file(&target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn get_plugin_target(game_dir: &str) -> PathBuf {
    Path::new(game_dir).join("game/python-packages/tcp_bridge/__init__.py")
}
