"""Ren'Py TCP 桥接客户端 — 实现 IGameBridge 接口

连接 Ren'Py 游戏内运行的 TCP Bridge 插件，读写实时游戏数据。
通过 socket TCP 与游戏进程通信，无需外部依赖。
"""
import json
import socket
import time
from typing import Optional

from core.game_bridge import IGameBridge, GameState


class RenPyBridge(IGameBridge):
    """Ren'Py 游戏实时桥接器

    连接游戏内 TCP Bridge 插件 (localhost:19999)，
    通过 JSON 命令读写游戏变量。

    启动方式:
        1. 将插件注入游戏目录 (game/python-packages/tcp_bridge/)
        2. 启动游戏
        3. 连接: bridge = RenPyBridge(); bridge.connect()
    """

    DEFAULT_HOST = "127.0.0.1"
    DEFAULT_PORT = 19999
    TIMEOUT = 5.0

    def __init__(self, host: str = "", port: int = 0, **kwargs):
        self._host = host or self.DEFAULT_HOST
        self._port = port or self.DEFAULT_PORT
        self._connected = False
        self._sock: socket.socket | None = None

    @property
    def is_connected(self) -> bool:
        return self._connected

    @staticmethod
    def priority() -> int:
        return 50  # 低于 RPG Maker(10)，高于通用方案

    @staticmethod
    def engine_name() -> str:
        return "renpy"

    def check_available(self) -> bool:
        """检查 Ren'Py 桥接是否可用（尝试 TCP 连接）"""
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(0.5)
            sock.connect((self._host, self._port))
            sock.close()
            return True
        except (socket.error, OSError):
            return False

    # ── 连接管理 ──────────────────────────────────────

    def connect(self) -> tuple[bool, str]:
        """连接 Ren'Py 游戏 TCP 桥接"""
        if self._connected:
            return True, "renpy (已连接)"

        try:
            self._sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self._sock.settimeout(self.TIMEOUT)
            self._sock.connect((self._host, self._port))

            # 验证连接
            result = self._send_cmd({"action": "ping"})
            if result and result.get("ok"):
                self._connected = True
                return True, "renpy"
            else:
                self._sock.close()
                self._sock = None
                return False, "renpy (无响应)"
        except (socket.error, OSError, ConnectionRefusedError) as e:
            if self._sock:
                try:
                    self._sock.close()
                except Exception:
                    pass
                self._sock = None
            return False, f"renpy ({e})"

    def disconnect(self) -> None:
        """断开连接"""
        self._connected = False
        if self._sock:
            try:
                self._sock.close()
            except Exception:
                pass
            self._sock = None

    # ── TCP 通信 ──────────────────────────────────────

    def _send_cmd(self, cmd: dict) -> dict | None:
        """发送 JSON 命令并接收响应"""
        if not self._sock:
            return None
        try:
            data = json.dumps(cmd, ensure_ascii=False).encode("utf-8")
            self._sock.sendall(data)

            response = b""
            while True:
                chunk = self._sock.recv(65536)
                if not chunk:
                    break
                response += chunk
                if len(chunk) < 65536:
                    break

            return json.loads(response.decode("utf-8"))
        except (socket.timeout, ConnectionError, json.JSONDecodeError) as e:
            self._connected = False
            return {"error": str(e)}

    # ── 游戏状态 ──────────────────────────────────────

    def get_state(self) -> GameState | None:
        """获取 Ren'Py 游戏状态快照"""
        result = self._send_cmd({"action": "get_state"})
        if not result:
            return None

        state = GameState(engine="renpy", raw=result)

        # 将 Ren'Py store 变量映射到 GameState 字段
        # 常见变量名映射（不同游戏可能不同）
        GOLD_NAMES = ("gold", "money", "Gold", "Money", "金币", "gold_amount")
        HP_NAMES = ("hp", "health", "HP", "Health", "player_hp")
        VAR_NAMES = ("variable", "variables")

        # 提取金币
        for name in GOLD_NAMES:
            if name in result:
                try:
                    state.gold = int(result[name])
                except (ValueError, TypeError):
                    pass
                break

        # 提取其他信息
        state.switches = {k: v for k, v in result.items()
                          if isinstance(v, bool) and not k.startswith("_")}
        state.variables = {k: v for k, v in result.items()
                           if isinstance(v, (int, float)) and not k.startswith("_")}
        state.play_time = str(result.get("play_time", result.get("playtime", "")))
        state.map_name = str(result.get("scene", result.get("label", "")))

        return state

    def get_raw_state(self) -> dict | None:
        """获取原始状态字典"""
        return self._send_cmd({"action": "get_state"})

    # ── 通用变量读写 ──────────────────────────────────

    def get_variable(self, name: str):
        """读取 Ren'Py store 中的变量"""
        result = self._send_cmd({"action": "get_var", "name": name})
        if result:
            return result.get("value")
        return None

    def set_variable(self, var_id: int, value: int) -> bool:
        """写入 Ren'Py store 变量（按 name 查找）"""
        # var_id 在这里是"尝试写变量"，实际通过通用 set_var
        result = self._send_cmd({"action": "set_var", "name": str(var_id), "value": value})
        return result is not None and result.get("ok", False)

    def set_named_variable(self, name: str, value) -> bool:
        """按名称写入变量"""
        result = self._send_cmd({"action": "set_var", "name": name, "value": value})
        return result is not None and result.get("ok", False)

    def eval_code(self, code: str):
        """在游戏进程中执行 Python 表达式"""
        result = self._send_cmd({"action": "eval", "code": code})
        if result:
            return result.get("value")
        return None

    # ── IGameBridge 接口方法 ──────────────────────────

    def set_gold(self, amount: int) -> bool:
        # 尝试常见金币变量名
        for name in ("gold", "money", "Gold", "Money", "金币"):
            result = self._send_cmd({"action": "set_var", "name": name, "value": amount})
            if result and result.get("ok"):
                return True
        return False

    def set_switch(self, sw_id: int, value: bool) -> bool:
        return self.set_named_variable(str(sw_id), value)

    def set_actor_hp(self, actor_id: int, hp: int) -> bool:
        # Ren'Py 游戏通常用变量存储 HP，而非 RPG Maker 的角色表
        for name in (f"hp_{actor_id}", f"player_hp", "hp", "health", "Health"):
            result = self._send_cmd({"action": "set_var", "name": name, "value": hp})
            if result and result.get("ok"):
                return True
        return False

    def set_actor_mp(self, actor_id: int, mp: int) -> bool:
        for name in (f"mp_{actor_id}", f"player_mp", "mp", "mana", "Mana"):
            result = self._send_cmd({"action": "set_var", "name": name, "value": mp})
            if result and result.get("ok"):
                return True
        return False

    def set_item_count(self, item_id: int, count: int) -> bool:
        for name in (f"item_{item_id}", "item_count", "items"):
            result = self._send_cmd({"action": "set_var", "name": name, "value": count})
            if result and result.get("ok"):
                return True
        return False
