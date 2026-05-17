"""游戏配置持久化 — 保存和加载游戏设置

在 profiles/ 目录下存储每个游戏的配置信息，
支持多游戏切换，自动恢复上次会话的设置。
"""

import json
import os
import time
from dataclasses import dataclass, field, asdict


# ── 数据类 ────────────────────────────────────────────

@dataclass
class GameProfile:
  """游戏配置档案"""
  profile_id: str = ""             # 唯一标识（基于游戏目录路径哈希）
  game_name: str = ""              # 游戏名称
  game_dir: str = ""               # 游戏根目录
  engine: str = "unknown"          # 引擎类型
  data_dir: str = ""               # 数据目录
  save_dir: str = ""               # 存档目录
  last_save_path: str = ""         # 上次加载的存档路径
  exe_path: str = ""               # 可执行文件路径
  plugin_installed: bool = False   # 插件是否已安装
  plugin_port: int = 19999         # TCP 桥接端口
  tcp_enabled: bool = True         # 是否启用 TCP 连接
  cdp_port: int = 9222             # CDP 调试端口
  auto_connect: bool = True        # 是否自动连接
  auto_scan: bool = True           # 是否自动扫描可修改项目
  created_at: float = 0.0          # 创建时间
  updated_at: float = 0.0          # 最后更新时间
  custom_fields: dict = field(default_factory=dict)  # 扩展字段


# ── 配置管理 ──────────────────────────────────────────

class ProfileManager:
  """游戏配置档案管理器"""

  def __init__(self, profiles_dir: str = ""):
    if not profiles_dir:
      profiles_dir = os.path.join(os.path.dirname(__file__), "..", "profiles")
    self._dir = os.path.abspath(profiles_dir)
    os.makedirs(self._dir, exist_ok=True)
    self._current: GameProfile | None = None

  @property
  def current(self) -> GameProfile | None:
    return self._current

  def _profile_path(self, profile_id: str) -> str:
    """获取配置文件的路径"""
    safe_name = "".join(c for c in profile_id if c.isalnum() or c in "_-.")
    return os.path.join(self._dir, f"{safe_name}.profile.json")

  def _make_id(self, game_dir: str) -> str:
    """基于游戏目录生成唯一 ID"""
    import hashlib
    return hashlib.md5(game_dir.encode("utf-8")).hexdigest()[:12]

  def load(self, game_dir: str) -> GameProfile | None:
    """加载指定游戏目录的配置档案"""
    pid = self._make_id(game_dir)
    path = self._profile_path(pid)
    if not os.path.isfile(path):
      return None
    try:
      with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)
      profile = GameProfile(**{k: v for k, v in data.items()
                               if k in GameProfile.__dataclass_fields__})
      # 恢复自定义字段
      for k, v in data.items():
        if k not in GameProfile.__dataclass_fields__:
          profile.custom_fields[k] = v
      self._current = profile
      return profile
    except Exception:
      return None

  def save(self, profile: GameProfile) -> bool:
    """保存配置档案"""
    if not profile.game_dir:
      return False
    profile.profile_id = self._make_id(profile.game_dir)
    profile.updated_at = time.time()
    if profile.created_at == 0:
      profile.created_at = profile.updated_at

    path = self._profile_path(profile.profile_id)
    try:
      data = asdict(profile)
      data.pop("custom_fields", None)
      # 合并自定义字段
      if profile.custom_fields:
        data.update(profile.custom_fields)
      with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
      self._current = profile
      return True
    except Exception:
      return False

  def create_from_game_info(self, game_info,
                            auto_connect: bool = True) -> GameProfile:
    """从 GameInfo 创建配置档案"""
    from core.game_detector import GameInfo
    profile = GameProfile(
      game_name=game_info.game_title or os.path.basename(game_info.game_dir),
      game_dir=game_info.game_dir,
      engine=game_info.engine,
      data_dir=game_info.data_dir,
      save_dir=game_info.save_dir,
      exe_path=game_info.exe_path,
      last_save_path=(game_info.save_files[0] if game_info.save_files else ""),
      auto_connect=auto_connect,
    )
    return profile

  def list_all(self) -> list[GameProfile]:
    """列出所有已保存的配置档案"""
    profiles = []
    if not os.path.isdir(self._dir):
      return profiles
    for fname in os.listdir(self._dir):
      if fname.endswith(".profile.json"):
        path = os.path.join(self._dir, fname)
        try:
          with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
          profile = GameProfile(**{k: v for k, v in data.items()
                                   if k in GameProfile.__dataclass_fields__})
          profiles.append(profile)
        except Exception:
          pass
    profiles.sort(key=lambda p: p.updated_at, reverse=True)
    return profiles

  def delete(self, game_dir: str) -> bool:
    """删除指定游戏的配置档案"""
    pid = self._make_id(game_dir)
    path = self._profile_path(pid)
    if os.path.isfile(path):
      try:
        os.remove(path)
        if self._current and self._current.game_dir == game_dir:
          self._current = None
        return True
      except OSError:
        pass
    return False

  def set_current_game(self, game_dir: str) -> GameProfile | None:
    """设置当前游戏并加载其配置"""
    profile = self.load(game_dir)
    if not profile:
      # 没有现存的配置，创建新的
      from core.game_detector import detect_game
      info = detect_game(game_dir=game_dir)
      if info:
        profile = self.create_from_game_info(info)
        self.save(profile)
    self._current = profile
    return profile

  def update_plugin_status(self, installed: bool):
    """更新当前配置的插件安装状态"""
    if self._current:
      self._current.plugin_installed = installed
      self.save(self._current)

  def update_last_save(self, save_path: str):
    """更新上次打开的存档路径"""
    if self._current:
      self._current.last_save_path = save_path
      self.save(self._current)


# ── 全局实例 ──────────────────────────────────────────

_profile_manager: ProfileManager | None = None


def get_profile_manager() -> ProfileManager:
  """获取全局配置管理器单例"""
  global _profile_manager
  if _profile_manager is None:
    _profile_manager = ProfileManager()
  return _profile_manager
