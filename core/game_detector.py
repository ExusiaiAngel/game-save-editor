"""通用游戏检测器 — 自动识别游戏类型、目录结构、存档位置

支持:
- RPG Maker MV (.rpgsave) / MZ (.rmmzsave)
- NW.js 封装的任意游戏
- 多种目录布局变体
"""

import json
import os
import re
from dataclasses import dataclass, field


# ── 数据类 ────────────────────────────────────────────

@dataclass
class GameInfo:
  """检测到的游戏信息"""
  game_dir: str = ""               # 游戏根目录
  game_title: str = ""             # 游戏标题
  engine: str = "unknown"          # "rpg_mv", "rpg_mz", "nwjs"
  is_nwjs: bool = False            # 是否 NW.js 封装
  data_dir: str = ""               # 数据目录（含 JSON 配置文件）
  save_dir: str = ""               # 存档目录
  www_dir: str = ""                # www 目录（NW.js 游戏）
  exe_path: str = ""               # 游戏可执行文件路径
  exe_name: str = "Game.exe"       # 可执行文件名
  package_json_path: str = ""      # package.json 路径
  save_files: list[str] = field(default_factory=list)  # 存档文件列表
  save_format: str = "rpgsave"     # "rpgsave" 或 "rmmzsave"
  detected_from: str = ""          # 检测来源: "save", "process", "dir"


# ── 主检测函数 ────────────────────────────────────────

def detect_game(save_path: str = "", process_info: dict | None = None,
                game_dir: str = "") -> GameInfo | None:
  """自动检测游戏信息

  优先级: process_info > save_path > game_dir

  Args:
    save_path: 存档文件路径
    process_info: 进程信息 {"exe_path": str, "pid": int}
    game_dir: 用户指定的游戏目录

  Returns:
    GameInfo 或 None
  """
  info = GameInfo()

  # 1. 从进程信息检测
  if process_info:
    exe_path = process_info.get("exe_path", "")
    if exe_path and os.path.isfile(exe_path):
      info.exe_path = exe_path
      info.game_dir = os.path.dirname(exe_path)
      info.exe_name = os.path.basename(exe_path)
      info.detected_from = "process"
      if _fill_game_info(info):
        return info

  # 2. 从存档路径反推游戏目录
  if save_path and os.path.isfile(save_path):
    info.detected_from = "save"
    info.game_dir = _infer_game_dir_from_save(save_path)
    if info.game_dir:
      info.save_files = [save_path]
      info.save_dir = os.path.dirname(save_path)
      ext = os.path.splitext(save_path)[1].lower()
      info.save_format = "rmmzsave" if ext == ".rmmzsave" else "rpgsave"
      if _fill_game_info(info):
        return info

  # 3. 从用户指定的游戏目录检测
  if game_dir and os.path.isdir(game_dir):
    info.detected_from = "dir"
    info.game_dir = game_dir
    if _fill_game_info(info):
      return info

  # 4. 尝试从保存路径向上搜索（非标准布局）
  if save_path and os.path.isfile(save_path):
    info = _brute_force_detect(save_path)
    if info:
      return info

  return None


def _fill_game_info(info: GameInfo) -> bool:
  """填充游戏信息的各个字段，返回是否找到有效游戏"""
  gd = info.game_dir
  if not gd or not os.path.isdir(gd):
    return False

  # 检测 NW.js
  info.is_nwjs = _is_nwjs_game(gd)

  # 查找 www 目录
  for sub in ["www", "html", "game"]:
    d = os.path.join(gd, sub)
    if os.path.isdir(d):
      info.www_dir = d
      break

  # 查找数据目录
  info.data_dir = _find_data_dir(gd, info.www_dir)

  # 查找 package.json
  for loc in [gd, info.www_dir]:
    if loc:
      pkg = os.path.join(loc, "package.json")
      if os.path.isfile(pkg):
        info.package_json_path = pkg
        break

  # 读取游戏标题
  info.game_title = _read_game_title(info)

  # 检测引擎类型
  info.engine = _detect_engine(info)

  # 查找可执行文件
  if not info.exe_path:
    info.exe_path = _find_exe(gd)
    if info.exe_path:
      info.exe_name = os.path.basename(info.exe_path)

  # 查找存档目录
  if not info.save_dir:
    info.save_dir = _find_save_dir(gd, info.www_dir)

  # 扫描存档文件
  if info.save_dir and not info.save_files:
    info.save_files = _scan_save_files(info.save_dir)

  # 至少要有数据目录或存档目录才认为有效
  return bool(info.data_dir or info.save_dir)


def _brute_force_detect(save_path: str) -> GameInfo | None:
  """从存档路径暴力向上搜索游戏根目录"""
  info = GameInfo(detected_from="save")
  info.save_files = [save_path]
  info.save_dir = os.path.dirname(save_path)
  ext = os.path.splitext(save_path)[1].lower()
  info.save_format = "rmmzsave" if ext == ".rmmzsave" else "rpgsave"

  # 向上搜索最多 5 层
  current = os.path.dirname(save_path)
  for _ in range(5):
    if _is_game_root(current):
      info.game_dir = current
      _fill_game_info(info)
      return info
    parent = os.path.dirname(current)
    if parent == current:
      break
    current = parent

  return None


# ── 引擎检测 ──────────────────────────────────────────

def _is_game_root(directory: str) -> bool:
  """判断目录是否为游戏根目录"""
  if not os.path.isdir(directory):
    return False
  # NW.js 特征: Game.exe / nw.dll / package.json
  has_nw = os.path.isfile(os.path.join(directory, "nw.dll"))
  has_pkg = os.path.isfile(os.path.join(directory, "package.json"))
  has_exe = any(
    os.path.isfile(os.path.join(directory, f))
    for f in os.listdir(directory)
    if f.lower().endswith(".exe")
  )
  # RPG Maker 特征: www/data/ 或 data/System.json
  has_data = (
    os.path.isfile(os.path.join(directory, "www", "data", "System.json")) or
    os.path.isfile(os.path.join(directory, "data", "System.json"))
  )
  return (has_nw or (has_pkg and has_exe) or has_data)


def _is_nwjs_game(game_dir: str) -> bool:
  """检测是否 NW.js 游戏"""
  return (
    os.path.isfile(os.path.join(game_dir, "nw.dll")) or
    os.path.isfile(os.path.join(game_dir, "nw_elf.dll"))
  )


def _detect_engine(info: GameInfo) -> str:
  """检测游戏引擎类型"""
  # 检查 RPG Maker 特征文件
  data_dir = info.data_dir
  if data_dir:
    has_system = os.path.isfile(os.path.join(data_dir, "System.json"))
    if has_system:
      # 检查是否为 MZ（MZ 的特有文件或结构）
      # MZ 通常使用 .rmmzsave，MV 使用 .rpgsave
      if info.save_format == "rmmzsave":
        return "rpg_mz"
      # 检查 data/ 中是否有 MZ 特有的文件结构
      if os.path.isfile(os.path.join(data_dir, "Traits.json")):
        return "rpg_mz"
      return "rpg_mv"

  # 纯 NW.js 游戏
  if info.is_nwjs:
    return "nwjs"

  return "unknown"


def _read_game_title(info: GameInfo) -> str:
  """从游戏配置中读取标题"""
  data_dir = info.data_dir
  if data_dir:
    sys_path = os.path.join(data_dir, "System.json")
    if os.path.isfile(sys_path):
      try:
        with open(sys_path, "r", encoding="utf-8") as f:
          sys_data = json.load(f)
        return sys_data.get("gameTitle", "")
      except Exception:
        pass

  # 从 index.html 标题读取
  if info.www_dir:
    html_path = os.path.join(info.www_dir, "index.html")
    if os.path.isfile(html_path):
      try:
        with open(html_path, "r", encoding="utf-8") as f:
          content = f.read()
        match = re.search(r"<title>(.+?)</title>", content)
        if match:
          return match.group(1).strip()
      except Exception:
        pass

  return os.path.basename(info.game_dir)


# ── 目录查找 ──────────────────────────────────────────

def _find_data_dir(game_dir: str, www_dir: str = "") -> str:
  """查找游戏数据目录"""
  candidates = []
  if www_dir:
    candidates.append(os.path.join(www_dir, "data"))
  candidates.extend([
    os.path.join(game_dir, "www", "data"),
    os.path.join(game_dir, "data"),
  ])
  for d in candidates:
    if os.path.isdir(d) and os.path.isfile(os.path.join(d, "System.json")):
      return d
  # 没有 System.json 也行，只要是 data 目录
  for d in candidates:
    if os.path.isdir(d):
      return d
  return ""


def _find_save_dir(game_dir: str, www_dir: str = "") -> str:
  """查找存档目录"""
  candidates = []
  if www_dir:
    candidates.extend([
      os.path.join(www_dir, "Save"),
      os.path.join(www_dir, "save"),
      os.path.join(www_dir, "saves"),
    ])
  candidates.extend([
    os.path.join(game_dir, "www", "Save"),
    os.path.join(game_dir, "www", "save"),
    os.path.join(game_dir, "Save"),
    os.path.join(game_dir, "save"),
    os.path.join(game_dir, "saves"),
  ])
  for d in candidates:
    if os.path.isdir(d):
      return d
  return ""


def _find_exe(game_dir: str) -> str:
  """查找游戏可执行文件"""
  # 常见 NW.js 游戏 EXE 名称
  common_names = ["Game.exe", "game.exe", "nw.exe", "RPGMV.exe",
                  "rpg_game.exe", "start.exe"]
  for name in common_names:
    path = os.path.join(game_dir, name)
    if os.path.isfile(path):
      return path
  # 扫描目录中任意 .exe（优先选小的，排除 installer）
  try:
    exes = []
    for f in os.listdir(game_dir):
      if f.lower().endswith(".exe"):
        fp = os.path.join(game_dir, f)
        size = os.path.getsize(fp)
        exes.append((size, fp))
    if exes:
      exes.sort()
      # 返回最大的 exe（通常是游戏主程序，NW.js 的游戏 exe 通常 1-2MB+）
      return exes[-1][1]
  except OSError:
    pass
  return ""


def _infer_game_dir_from_save(save_path: str) -> str:
  """从存档路径反推游戏根目录"""
  # 标准布局: game/www/Save/file1.rpgsave → game/
  save_dir = os.path.dirname(save_path)
  www_dir = os.path.dirname(save_dir)
  game_dir = os.path.dirname(www_dir)

  # 验证是否真的是游戏根目录
  if _is_game_root(game_dir):
    return game_dir

  # 非标准布局: game/Save/file1.rpgsave → game/
  game_dir2 = os.path.dirname(save_dir)
  if _is_game_root(game_dir2):
    return game_dir2

  # 更深的嵌套: game/bin/www/Save/
  for _ in range(2):
    game_dir = os.path.dirname(game_dir)
    if _is_game_root(game_dir):
      return game_dir

  return ""


def _scan_save_files(save_dir: str) -> list[str]:
  """扫描存档目录中的存档文件"""
  if not save_dir or not os.path.isdir(save_dir):
    return []
  files = []
  try:
    for f in sorted(os.listdir(save_dir)):
      if f.endswith((".rpgsave", ".rmmzsave")):
        # 排除 config 和 global 文件
        if not f.startswith(("config", "global", "Config", "Global")):
          files.append(os.path.join(save_dir, f))
  except OSError:
    pass
  return files


# ── 便捷函数 ──────────────────────────────────────────

def get_save_paths_for_game(game_dir: str) -> list[str]:
  """获取游戏的所有存档文件路径（默认返回第一个）"""
  info = detect_game(game_dir=game_dir)
  if info:
    return info.save_files
  return []


def get_primary_save(game_dir: str) -> str:
  """获取游戏的主存档文件"""
  files = get_save_paths_for_game(game_dir)
  return files[0] if files else ""


def is_rpg_maker_game(game_dir: str) -> bool:
  """快速判断是否为 RPG Maker 游戏"""
  info = detect_game(game_dir=game_dir)
  return info is not None and info.engine in ("rpg_mv", "rpg_mz")
