"""RPG Maker MV/MZ 存档解析与编辑器

使用 LZ-String 解压/压缩 + JSON 解析来读写 RPG Maker MV 存档。
"""
import json
import logging
import os
import shutil
import copy
from datetime import datetime
from pathlib import Path

from core.lzstring import decompress_from_base64, compress_to_base64


def _has_json_ex_c_format(data, _depth=0) -> bool:
  """递归检测数据中是否包含 JsonEx @c 压缩数组格式"""
  if _depth > 20:
    return False
  if isinstance(data, dict):
    if "@c" in data:
      return True
    for v in data.values():
      if _has_json_ex_c_format(v, _depth + 1):
        return True
  elif isinstance(data, list):
    for v in data:
      if _has_json_ex_c_format(v, _depth + 1):
        return True
  return False


def load_save(filepath: str) -> dict:
  """加载 RPG Maker MV 存档文件
  
  Args:
    filepath: 存档文件路径（.rmmzsave / .rpgsave）
  
  Returns:
    游戏状态字典，包含 party, actors, system 等键
  """
  path = Path(filepath)
  if not path.is_file():
    raise FileNotFoundError(f"存档文件不存在: {filepath}")
  
  with open(path, "r", encoding="utf-8") as f:
    raw = f.read().strip()
  
  if not raw:
    raise ValueError("存档文件为空")
  
  # 解压 LZ-String → JSON
  try:
    json_str = decompress_from_base64(raw)
  except Exception as e:
    raise ValueError(f"存档文件格式无效（不是有效的 LZ-String 格式）: {e}")
  
  if json_str is None or json_str == "":
    raise ValueError("解压后数据为空")
  
  try:
    data = json.loads(json_str)
  except json.JSONDecodeError as e:
    raise ValueError(f"存档 JSON 解析失败: {e}")

  # @c 是 RPG Maker MV/MZ JsonEx 标准序列化格式，非数据损坏
  if _has_json_ex_c_format(data):
    logging.debug("存档使用 JsonEx @c 压缩数组格式（RPG Maker MV/MZ 标准格式）")

  return data


def save_save(filepath: str, data: dict) -> None:
  """保存修改后的存档
  
  Args:
    filepath: 存档文件路径
    data: 修改后的游戏状态字典
  """
  json_str = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
  compressed = compress_to_base64(json_str)
  
  path = Path(filepath)
  path.parent.mkdir(parents=True, exist_ok=True)

  # 写入前备份原有存档
  if os.path.isfile(filepath):
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    backup_path = f"{filepath}.{timestamp}.bak"
    shutil.copy2(filepath, backup_path)

  with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.write(compressed)


def cleanup_old_backups(directory: str, keep: int = 10) -> int:
  """清理旧的备份文件，只保留最近的 N 个

  Args:
    directory: 备份文件所在目录
    keep: 保留的备份文件数量（默认 10）

  Returns:
    删除的文件数量
  """
  import glob
  pattern = os.path.join(directory, "*.bak")
  bak_files = sorted(glob.glob(pattern), key=os.path.getmtime, reverse=True)
  removed = 0
  for old_file in bak_files[keep:]:
    try:
      os.remove(old_file)
      removed += 1
    except OSError:
      pass
  return removed


# ── 金币 ──────────────────────────────────────────────

def get_gold(data: dict) -> int:
  """获取当前金币数"""
  party = data.get("party", {})
  return party.get("_gold", 0)


def set_gold(data: dict, amount: int) -> None:
  """设置金币数"""
  if "party" not in data:
    data["party"] = {}
  data["party"]["_gold"] = max(0, int(amount))


def get_max_gold(data: dict) -> int:
  """获取金币上限（通常 99999999）"""
  return 99999999


# ── 物品 ──────────────────────────────────────────────

def get_items(data: dict) -> list[dict]:
  """获取物品列表 [{id, name, count}]"""
  items = []
  raw_items = data.get("party", {}).get("_items", {})
  # JsonEx @c 引用格式：仅 @c 无 @a 表示全局引用池中的空数据
  if isinstance(raw_items, dict) and '@c' in raw_items and '@a' not in raw_items:
    return items
  # JsonEx 格式: {"@a": {"1":5, "2":3}}
  if isinstance(raw_items, dict) and "@a" in raw_items:
    party_items = raw_items.get("@a", {})
  elif isinstance(raw_items, dict):
    party_items = raw_items
  else:
    party_items = {}
  
  if not party_items:
    return items
  
  # 尝试从 data 中获取物品名称映射
  item_names = {}
  # RPG Maker MV 存档不包含物品名称，需要从游戏数据文件读取
  # 此处只返回 ID，名称由调用者提供
  
  for item_id, count in party_items.items():
    # 跳过 JsonEx 元数据键（@a, @c 等）
    if isinstance(item_id, str) and item_id.startswith("@"):
      continue
    if count and count > 0:
      items.append({
        "id": int(item_id),
        "name": item_names.get(int(item_id), f"物品{item_id}"),
        "count": count,
      })
  return items


def set_item_count(data: dict, item_id: int, count: int) -> None:
  """设置物品数量（兼容 JsonEx @a 格式）"""
  if "party" not in data:
    data["party"] = {}
  if "_items" not in data["party"]:
    data["party"]["_items"] = {}
  items = data["party"]["_items"]
  # JsonEx @a 格式：写入 @a 内的字典
  if isinstance(items, dict) and "@a" in items:
    items = items["@a"]
  items[str(item_id)] = max(0, int(count))


# ── 队伍信息 ──────────────────────────────────────────

def get_party_info(data: dict) -> list[dict]:
  """获取队伍成员信息 [{name, level, hp, mhp, mp, mmp, ...}]"""
  actors_data = data.get("actors", {}).get("_data", [])
  # JsonEx 格式: _data 可能是 {"@a": [null, {...}, ...]}
  if isinstance(actors_data, dict):
    actors_data = actors_data.get("@a", [])
  if not isinstance(actors_data, list):
    actors_data = []
  
  # RPG Maker MV JsonEx 格式: _actors 可能是 {"@a": [1,2]} 或直接是列表
  raw_actors = data.get("party", {}).get("_actors", [])
  if isinstance(raw_actors, dict):
    party_members = raw_actors.get("@a", [])
  elif isinstance(raw_actors, list):
    party_members = raw_actors
  else:
    party_members = []
  
  result = []
  for actor_id in party_members:
    # 跳过 JsonEx 元数据键（@a, @c 等）
    if isinstance(actor_id, str) and actor_id.startswith("@"):
      continue
    actor_id = int(actor_id) if not isinstance(actor_id, int) else actor_id
    if 0 < actor_id < len(actors_data):
      actor = actors_data[actor_id]
      if actor:
        result.append({
          "id": actor_id,
          "name": actor.get("_name", f"角色{actor_id}"),
          "nickname": actor.get("_nickname", ""),
          "level": actor.get("_level", 1),
          "hp": actor.get("_hp", 0),
          "mhp": actor.get("_mhp", 1),
          "mp": actor.get("_mp", 0),
          "mmp": actor.get("_mmp", 1),
          "exp": actor.get("_exp", [0])[actor_id - 1] if isinstance(actor.get("_exp"), list) and len(actor.get("_exp", [])) > actor_id - 1 else 0,
          "atk": actor.get("_paramPlus", [0]*8)[2] if len(actor.get("_paramPlus", [])) > 2 else 0,
        })
  return result


def _get_actors_array(data: dict) -> list:
  """获取角色数据数组，自动解包 @a 格式"""
  actors_data = data.get("actors", {}).get("_data", [])
  if isinstance(actors_data, dict):
    return actors_data.get("@a", [])
  if isinstance(actors_data, list):
    return actors_data
  return []


def set_actor_hp(data: dict, actor_id: int, hp: int) -> None:
  """设置角色 HP"""
  actors_data = _get_actors_array(data)
  if 0 < actor_id < len(actors_data) and actors_data[actor_id]:
    actors_data[actor_id]["_hp"] = max(0, int(hp))


def set_actor_mp(data: dict, actor_id: int, mp: int) -> None:
  """设置角色 MP"""
  actors_data = _get_actors_array(data)
  if 0 < actor_id < len(actors_data) and actors_data[actor_id]:
    actors_data[actor_id]["_mp"] = max(0, int(mp))


def set_actor_level(data: dict, actor_id: int, level: int) -> None:
  """设置角色等级"""
  actors_data = _get_actors_array(data)
  if 0 < actor_id < len(actors_data) and actors_data[actor_id]:
    actors_data[actor_id]["_level"] = max(1, min(99, int(level)))


# ── 统计信息 ──────────────────────────────────────────

def get_save_summary(data: dict) -> dict:
  """获取存档摘要信息"""
  party_info = get_party_info(data)
  items = get_items(data)
  
  return {
    "gold": get_gold(data),
    "party_size": len(party_info),
    "members": [f"{m['name']} Lv.{m['level']} HP:{m['hp']}/{m['mhp']}" for m in party_info],
    "item_count": len(items),
    "save_count": data.get("system", {}).get("_saveCount", 0),
    "play_time": data.get("system", {}).get("_playtime", 0),
  }


def clone_data(data: dict) -> dict:
  """深拷贝存档数据"""
  return copy.deepcopy(data)


def get_weapons(data):
  return _get_inventory(data, '_weapons')


def get_armors(data):
  return _get_inventory(data, '_armors')


def _get_inventory(data, field):
  items = []
  raw = data.get("party", {}).get(field, {})
  if isinstance(raw, dict) and '@c' in raw and '@a' not in raw:
    return items
  if isinstance(raw, dict) and '@a' in raw:
    raw = raw.get('@a', {})
  elif not isinstance(raw, dict):
    return items
  for item_id, count in raw.items():
    if isinstance(item_id, str) and item_id.startswith('@'):
      continue
    if count and count > 0:
      items.append({"id": int(item_id), "name": f"#{item_id}", "count": count})
  return items


# ── 开关与变量 ────────────────────────────────────────

def _resolve_array(data, default=None):
  """解析 RPG Maker MV JsonEx 压缩数组格式（可能带 @c/@a 包装）"""
  if data is None:
    return default or []
  if isinstance(data, list):
    return data
  if isinstance(data, dict):
    if "@a" in data:
      arr = data["@a"]
      if isinstance(arr, list):
        return arr
      if isinstance(arr, dict):
        # @a is itself a sparse dict: @a: {"1": true, "5": false, ...}
        return arr
    if "@c" in data and "@a" not in data:
      return default or []
  return default or []


def _resolve_array_flat(data, default=None):
  """将 @a 稀疏字典或列表展开为连续列表"""
  arr = _resolve_array(data, default)
  if isinstance(arr, dict):
    # @a 稀疏字典: {"1": true, "3": false} → [None, true, None, false]
    result = []
    for k, v in sorted(arr.items(), key=lambda x: int(x[0]) if isinstance(x[0], (int, str)) and str(x[0]).isdigit() else 0):
      try:
        idx = int(k)
        while len(result) <= idx:
          result.append(None)
        result[idx] = v
      except (ValueError, TypeError):
        pass
    return result
  return arr


def get_switches(data: dict) -> dict[int, bool]:
  """从存档中读取所有开关状态

  注意: 开关位于顶层 data["switches"]["_data"]，不在 system 下！
  """
  result = {}
  # 实际位置: data["switches"]["_data"]，包含 @c/@a
  raw = data.get("switches", {}).get("_data", [])
  arr = _resolve_array(raw, [])
  if isinstance(arr, dict):
    # @a 稀疏字典
    for k, v in arr.items():
      try:
        idx = int(k)
        if idx > 0 and isinstance(v, bool):
          result[idx] = v
      except (ValueError, TypeError):
        pass
    return result
  for i, val in enumerate(arr):
    if i == 0:
      continue
    if val is True or val is False:
      result[i] = bool(val)
  return result


def get_variables(data: dict) -> dict[int, int]:
  """从存档中读取所有变量值

  注意: 变量位于顶层 data["variables"]["_data"]，不在 system 下！
  """
  result = {}
  raw = data.get("variables", {}).get("_data", [])
  arr = _resolve_array(raw, [])
  if isinstance(arr, dict):
    for k, v in arr.items():
      try:
        idx = int(k)
        if idx > 0 and isinstance(v, (int, float)) and v != 0:
          result[idx] = int(v)
      except (ValueError, TypeError):
        pass
    return result
  for i, val in enumerate(arr):
    if i == 0:
      continue
    if isinstance(val, (int, float)) and val != 0:
      result[i] = int(val)
  return result


def get_self_switches(data: dict) -> dict[str, bool]:
  """从存档中读取自开关状态"""
  result = {}
  raw = data.get("selfSwitches", data.get("self_switches", {}))
  sw_data = raw.get("_data", raw) if isinstance(raw, dict) else {}
  if isinstance(sw_data, dict):
    for key, val in sw_data.items():
      if isinstance(key, str) and key.startswith("@"):
        continue
      if val is True:
        result[str(key)] = True
  return result


def set_self_switch(data: dict, key: str, value: bool) -> None:
  """设置存档中的自开关值"""
  if "selfSwitches" not in data:
    data["selfSwitches"] = {}
  sw_data = data["selfSwitches"]
  if "_data" not in sw_data:
    sw_data["_data"] = {}
  sw_data["_data"][key] = bool(value)


def _ensure_switches_array(data: dict) -> list:
  """确保 switches._data 数组存在并返回（兼容纯列表和 JsonEx @a 格式）"""
  if "switches" not in data:
    data["switches"] = {}
  sw = data["switches"]
  if "_data" not in sw:
    sw["_data"] = [False]
  sw_data = sw["_data"]
  # 纯列表格式（最常见的 RPG Maker MV 格式）：直接使用
  if isinstance(sw_data, list):
    return sw_data
  if not isinstance(sw_data, dict):
    sw["_data"] = sw_data = [False]
    return sw_data
  # 如果已有 @a 数组，使用它
  if "@a" in sw_data and isinstance(sw_data["@a"], list):
    return sw_data["@a"]
  if "@a" in sw_data and isinstance(sw_data["@a"], dict):
    # 稀疏字典 → 展开为列表
    sparse = sw_data["@a"]
    arr = []
    for k, v in sorted(sparse.items(), key=lambda x: int(x[0]) if isinstance(x[0], str) and x[0].isdigit() else 0):
      try:
        idx = int(k)
        while len(arr) <= idx:
          arr.append(False)
        arr[idx] = v
      except (ValueError, TypeError):
        pass
    sw_data["@a"] = arr
    return arr
  # 没有 @a → 创建新数组（保留 @c/@ 引用以便 JsonEx 还能工作）
  arr = [False]
  sw_data["@a"] = arr
  return arr


def _ensure_variables_array(data: dict) -> list:
  """确保 variables._data 数组存在并返回（兼容纯列表和 JsonEx @a 格式）"""
  if "variables" not in data:
    data["variables"] = {}
  var = data["variables"]
  if "_data" not in var:
    var["_data"] = [0]
  var_data = var["_data"]
  # 纯列表格式（最常见的 RPG Maker MV 格式）：直接使用
  if isinstance(var_data, list):
    return var_data
  if not isinstance(var_data, dict):
    var["_data"] = var_data = [0]
    return var_data
  if "@a" in var_data and isinstance(var_data["@a"], list):
    return var_data["@a"]
  if "@a" in var_data and isinstance(var_data["@a"], dict):
    sparse = var_data["@a"]
    arr = []
    for k, v in sorted(sparse.items(), key=lambda x: int(x[0]) if isinstance(x[0], str) and x[0].isdigit() else 0):
      try:
        idx = int(k)
        while len(arr) <= idx:
          arr.append(0)
        arr[idx] = v
      except (ValueError, TypeError):
        pass
    var_data["@a"] = arr
    return arr
  arr = [0]
  var_data["@a"] = arr
  return arr


def set_switch(data: dict, switch_id: int, value: bool) -> None:
  """设置存档中的开关值（写入正确位置 switches._data.@a）"""
  arr = _ensure_switches_array(data)
  while len(arr) <= switch_id:
    arr.append(False)
  arr[switch_id] = bool(value)


def set_variable(data: dict, var_id: int, value: int) -> None:
  """设置存档中的变量值（写入正确位置 variables._data.@a）"""
  arr = _ensure_variables_array(data)
  while len(arr) <= var_id:
    arr.append(0)
  arr[var_id] = int(value)


def set_actor_max_hp(data: dict, actor_id: int, mhp: int) -> None:
  """设置角色最大 HP"""
  actors_data = _get_actors_array(data)
  if 0 < actor_id < len(actors_data) and actors_data[actor_id]:
    actors_data[actor_id]["_mhp"] = max(1, int(mhp))


def set_actor_max_mp(data: dict, actor_id: int, mmp: int) -> None:
  """设置角色最大 MP"""
  actors_data = _get_actors_array(data)
  if 0 < actor_id < len(actors_data) and actors_data[actor_id]:
    actors_data[actor_id]["_mmp"] = max(0, int(mmp))


def set_actor_param(data: dict, actor_id: int, param_index: int, value: int) -> None:
  """设置角色参数加成 (ATK/DEF/MAT/MDF/AGI/LUK)"""
  actors_data = _get_actors_array(data)
  if 0 < actor_id < len(actors_data) and actors_data[actor_id]:
    actor = actors_data[actor_id]
    if "_paramPlus" not in actor:
      actor["_paramPlus"] = [0] * 8
    pp = actor["_paramPlus"]
    if isinstance(pp, dict) and "@a" in pp:
      pp = pp["@a"]
      actor["_paramPlus"] = pp
    while len(pp) <= param_index:
      pp.append(0)
    pp[param_index] = int(value)
