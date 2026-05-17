"""Ren'Py 插件注入器

将 TCP 桥接插件安装到 Ren'Py 游戏的 game/python-packages/tcp_bridge/ 目录。
插件代码内嵌于此模块中，不依赖外部文件（EXE 打包兼容）。

安装后需重启游戏生效。
"""
import os
import shutil


PLUGIN_DIR_NAME = "tcp_bridge"

# ── 内嵌的 TCP 桥接插件代码 ──
# 此字符串会被写入 game/python-packages/tcp_bridge/__init__.py
# 在 Ren'Py 游戏进程中作为 Python 模块自动加载

PLUGIN_CODE = r'''
"""Ren'Py TCP 桥接插件 — 注入到游戏中的 Python 代码

此文件会被复制到 Ren'Py 游戏的 game/python-packages/tcp_bridge/__init__.py
Ren'Py 启动时自动加载，在后台线程开启 TCP 服务器。
外部客户端（game_tool）通过 TCP 连接读写游戏实时数据。

用法:
  游戏开发者: 将 tcp_bridge/ 目录放入 game/python-packages/
  game_tool: 通过「注入插件」按钮自动安装
"""

import json
import socket
import threading
import sys

# ── 配置 ──────────────────────────────────────────

HOST = "127.0.0.1"
PORT = 19999
BUFFER_SIZE = 65536


# ── TCP 桥接服务器 ────────────────────────────────

class GameBridgeServer:
    """Ren'Py 游戏状态 TCP 桥接服务器

    在 Ren'Py 进程中启动，监听 localhost:19999。
    接收 JSON 命令，通过 Ren'Py store API 读写游戏状态。
    """

    def __init__(self, host=HOST, port=PORT):
        self.host = host
        self.port = port
        self.running = False
        self._sock = None

    def start(self):
        """在后台线程启动 TCP 服务器"""
        self.running = True
        thread = threading.Thread(target=self._run, name="tcp-bridge", daemon=True)
        thread.start()
        print(f"[TCP Bridge] 已启动: {self.host}:{self.port}")
        return thread

    def stop(self):
        """停止服务器"""
        self.running = False
        if self._sock:
            try:
                self._sock.close()
            except Exception:
                pass

    def _run(self):
        self._sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            self._sock.bind((self.host, self.port))
            self._sock.listen(1)
            self._sock.settimeout(1.0)
        except OSError as e:
            print(f"[TCP Bridge] 绑定失败 (端口可能被占用): {e}")
            self.running = False
            return

        while self.running:
            try:
                conn, addr = self._sock.accept()
                self._handle_client(conn)
            except socket.timeout:
                continue
            except OSError:
                if self.running:
                    break

        try:
            self._sock.close()
        except Exception:
            pass

    def _handle_client(self, conn):
        """处理单个客户端连接"""
        try:
            data = b""
            while True:
                chunk = conn.recv(BUFFER_SIZE)
                if not chunk:
                    break
                data += chunk
                if len(chunk) < BUFFER_SIZE:
                    break

            text = data.decode("utf-8")
            request = json.loads(text)
            response = self._dispatch(request)
            conn.sendall(json.dumps(response, ensure_ascii=False).encode("utf-8"))
        except Exception as e:
            try:
                conn.sendall(json.dumps({"error": str(e)}).encode("utf-8"))
            except Exception:
                pass
        finally:
            try:
                conn.close()
            except Exception:
                pass

    def _dispatch(self, request: dict) -> dict:
        action = request.get("action", "")
        handler = getattr(self, f"_cmd_{action}", None)
        if handler is None:
            return {"error": f"未知命令: {action}"}
        try:
            return handler(request)
        except Exception as e:
            return {"error": str(e)}

    def _get_store(self):
        """获取 Ren'Py store 模块"""
        try:
            import store as renpy_store
            return renpy_store
        except ImportError:
            pass
        try:
            import renpy
            return renpy.store
        except (ImportError, AttributeError):
            pass
        return None

    def _cmd_ping(self, _req):
        return {"ok": True, "engine": "renpy"}

    def _cmd_get_state(self, _req):
        """获取完整游戏状态快照"""
        store = self._get_store()
        if store is None:
            return {"error": "无法访问 Ren'Py store"}

        state = {"engine": "renpy"}
        for name in dir(store):
            if name.startswith("_") or name.startswith("__"):
                continue
            if name in ("renpy", "sys", "os", "math", "random", "json",
                        "collections", "itertools", "functools", "datetime",
                        "config", "persistent", "preferences"):
                continue
            try:
                val = getattr(store, name)
                if val is None:
                    continue
                if isinstance(val, (int, float, str, bool)):
                    state[name] = val
                elif isinstance(val, (list, tuple)) and len(val) < 100:
                    state[name] = list(val)[:50]
                elif isinstance(val, dict) and len(val) < 100:
                    simple = {str(k): str(v)[:100] for k, v in list(val.items())[:20]
                              if isinstance(k, (str, int)) and isinstance(v, (int, float, str, bool))}
                    if simple:
                        state[name] = simple
            except Exception:
                continue

        state["_var_count"] = len(state) - 1
        return state

    def _cmd_get_var(self, req):
        """读取指定变量值"""
        store = self._get_store()
        if store is None:
            return {"error": "无法访问 Ren'Py store"}
        name = req.get("name", "")
        if not name:
            return {"error": "缺少参数: name"}
        try:
            val = getattr(store, name)
            if isinstance(val, (int, float, str, bool, list, dict, type(None))):
                return {"value": val}
            return {"value": str(val), "_type": type(val).__name__}
        except AttributeError:
            return {"value": None}

    def _cmd_set_var(self, req):
        """写入变量值"""
        store = self._get_store()
        if store is None:
            return {"error": "无法访问 Ren'Py store"}
        name = req.get("name", "")
        value = req.get("value")
        if not name:
            return {"error": "缺少参数: name"}
        try:
            setattr(store, name, value)
            return {"ok": True}
        except Exception as e:
            return {"error": str(e)}

    def _cmd_eval(self, req):
        """执行 Python 表达式"""
        code = req.get("code", "")
        if not code:
            return {"error": "缺少参数: code"}
        try:
            store = self._get_store()
            safe_globals = {"store": store, "__builtins__": {}}
            result = eval(code, safe_globals)
            if isinstance(result, (int, float, str, bool, list, dict, type(None))):
                return {"value": result}
            return {"value": str(result)}
        except Exception as e:
            return {"error": str(e)}


# ── 自动启动 ─────────────────────────────────────

_bridge_server = None

def _auto_start():
    """Ren'Py 导入此包时自动启动桥接服务器"""
    global _bridge_server
    if _bridge_server is None:
        _bridge_server = GameBridgeServer()
        _bridge_server.start()


try:
    _auto_start()
except Exception as e:
    print(f"[TCP Bridge] 启动失败: {e}")
'''


# ── 公共 API ──────────────────────────────────────────

def get_plugin_content() -> str:
    """获取插件源代码内容（EXE 兼容，内嵌字符串）"""
    return PLUGIN_CODE


def is_plugin_installed(game_dir: str) -> bool:
    """检查 Ren'Py 游戏是否已安装 TCP 桥接插件"""
    plugin_dir = _get_plugin_target_dir(game_dir)
    if not plugin_dir:
        return False
    init_path = os.path.join(plugin_dir, "__init__.py")
    return os.path.isfile(init_path)


def inject_plugin(game_dir: str) -> bool:
    """将 TCP 桥接插件安装到 Ren'Py 游戏目录

    插件代码内嵌于此模块，不依赖外部文件（EXE 兼容）。

    Args:
        game_dir: 游戏根目录

    Returns:
        是否安装成功
    """
    plugin_dir = _get_plugin_target_dir(game_dir)
    if not plugin_dir:
        return False

    os.makedirs(plugin_dir, exist_ok=True)

    target = os.path.join(plugin_dir, "__init__.py")
    try:
        with open(target, "w", encoding="utf-8") as f:
            f.write(PLUGIN_CODE)
        return True
    except OSError as e:
        print(f"[Ren'Py Injector] 写入插件失败: {e}")
        return False


def remove_plugin(game_dir: str) -> bool:
    """从 Ren'Py 游戏目录移除 TCP 桥接插件"""
    plugin_dir = _get_plugin_target_dir(game_dir)
    if not plugin_dir or not os.path.isdir(plugin_dir):
        return True

    try:
        shutil.rmtree(plugin_dir)
        return True
    except OSError as e:
        print(f"[Ren'Py Injector] 移除插件失败: {e}")
        return False


def get_plugin_status_text(game_dir: str) -> str:
    """获取插件安装状态文本"""
    if is_plugin_installed(game_dir):
        return "✅ Ren'Py 桥接插件已安装"
    return "⚠ Ren'Py 桥接插件未安装"


def _get_plugin_target_dir(game_dir: str) -> str:
    """获取 Ren'Py 游戏中的插件安装目录"""
    if not game_dir or not os.path.isdir(game_dir):
        return ""

    game_subdir = os.path.join(game_dir, "game")
    if not os.path.isdir(game_subdir):
        if os.path.isfile(os.path.join(game_dir, "script.rpy")):
            game_subdir = game_dir
        else:
            return ""

    return os.path.join(game_subdir, "python-packages", PLUGIN_DIR_NAME)
