"""游戏数据扫描器 — 综合扫描游戏内所有可修改项目

从游戏数据文件 (System.json, Actors.json, Items.json 等)、存档文件
和实时游戏状态中收集所有可修改的项目，生成统一的可编辑字段列表。
"""

import json
import os
from dataclasses import dataclass, field

from core.game_config import scan_game_directory
from core.rpgmv_save import get_switches, get_variables, get_self_switches, get_party_info, get_items, get_gold, get_weapons, get_armors


# ── 数据类 ────────────────────────────────────────────

@dataclass
class ModifiableField:
  """一个可修改的字段"""
  category: str          # "switch", "variable", "actor", "gold", "item", "self_switch"
  field_id: str          # 唯一标识符，如 "switch_12", "var_5", "actor_1_hp"
  display_name: str      # 显示名称
  item_id: int = 0       # 游戏内 ID
  field_type: str = "int"  # "bool", "int", "str"
  save_value: object = None   # 存档中的值
  live_value: object = None   # 游戏实时值
  default_value: object = None  # 默认值
  min_val: int = 0
  max_val: int = 99999999
  description: str = ""
  dirty: bool = False     # 用户是否编辑过此字段（避免覆盖未编辑字段）
  gold_var_id: int = 0    # 关联的金币变量 ID（当游戏用变量存金币时）


@dataclass
class GameScanResult:
  """游戏全面扫描结果"""
  game_dir: str = ""
  game_title: str = ""
  has_save_data: bool = False
  has_live_data: bool = False
  fields: list[ModifiableField] = field(default_factory=list)
  categories: dict[str, list[ModifiableField]] = field(default_factory=dict)


# ── 扫描函数 ──────────────────────────────────────────

def scan_all_modifiable(
  game_dir: str,
  save_data: dict | None = None,
  live_state: dict | None = None,
) -> GameScanResult:
  """全面扫描游戏内所有可修改项目

  Args:
    game_dir: 游戏根目录路径
    save_data: 已加载的存档数据（可选）
    live_state: 实时游戏状态（可选），来自 CDP 连接

  Returns:
    GameScanResult 包含所有可修改字段
  """
  result = GameScanResult(game_dir=game_dir)

  # 加载游戏配置（开关/变量/角色名称等）
  try:
    config = scan_game_directory(game_dir)
  except Exception:
    config = {}
  result.game_title = config.get("game_title", os.path.basename(game_dir))

  # 读取游戏数据文件的原始 JSON
  data_dir = _find_data_dir(game_dir)
  system_data = _load_json(data_dir, "System.json") if data_dir else {}
  actors_data = _load_json(data_dir, "Actors.json") if data_dir else []

  # 收集开关名称（来自 System.json，索引即开关ID）
  switch_names: dict[int, str] = {}
  if system_data:
    switches = system_data.get("switches", [])
    for i, name in enumerate(switches):
      if name and isinstance(name, str) and name.strip():
        switch_names[i] = name.strip()  # System.json 索引即开关ID（index 0为空占位）

  # 收集变量名称
  variable_names: dict[int, str] = {}
  if system_data:
    variables = system_data.get("variables", [])
    for i, name in enumerate(variables):
      if name and isinstance(name, str) and name.strip():
        variable_names[i] = name.strip()  # System.json 索引即变量ID

  # 收集角色名称
  actor_names: dict[int, str] = {}
  if isinstance(actors_data, list):
    for i, actor in enumerate(actors_data):
      if actor and isinstance(actor, dict):
        actor_names[i] = actor.get("name", f"角色{i}")

  # 读取存档中的开关/变量值
  save_switches: dict[int, bool] = {}
  save_variables: dict[int, int] = {}
  save_self_switches: dict[str, bool] = {}
  if save_data:
    result.has_save_data = True
    try:
      save_switches = get_switches(save_data)
    except Exception:
      pass
    try:
      save_variables = get_variables(save_data)
    except Exception:
      pass
    try:
      save_self_switches = get_self_switches(save_data)
    except Exception:
      pass

  # 读取实时游戏中的开关/变量值
  live_switches: dict[int, bool] = {}
  live_variables: dict[int, int] = {}
  if live_state:
    result.has_live_data = True
    if "switches" in live_state:
      live_switches = {int(k): v for k, v in live_state["switches"].items()}
    if "variables" in live_state:
      live_variables = {int(k): v for k, v in live_state["variables"].items()}

  # ── 构建字段列表 ──

  # 1. 开关
  all_switch_ids = set()
  all_switch_ids.update(switch_names.keys())
  all_switch_ids.update(save_switches.keys())
  all_switch_ids.update(live_switches.keys())
  for sw_id in sorted(all_switch_ids):
    name = switch_names.get(sw_id, f"开关 #{sw_id}")
    sv = save_switches.get(sw_id)
    lv = live_switches.get(sw_id)
    result.fields.append(ModifiableField(
      category="switch",
      field_id=f"switch_{sw_id}",
      display_name=name,
      item_id=sw_id,
      field_type="bool",
      save_value=sv,
      live_value=lv,
      default_value=False,
      min_val=0, max_val=1,
    ))

  # 2. 变量
  all_var_ids = set()
  all_var_ids.update(variable_names.keys())
  all_var_ids.update(save_variables.keys())
  all_var_ids.update(live_variables.keys())
  for var_id in sorted(all_var_ids):
    name = variable_names.get(var_id, f"变量 #{var_id}")
    sv = save_variables.get(var_id)
    lv = live_variables.get(var_id)
    result.fields.append(ModifiableField(
      category="variable",
      field_id=f"var_{var_id}",
      display_name=name,
      item_id=var_id,
      field_type="int",
      save_value=sv,
      live_value=lv,
      default_value=0,
      min_val=-99999999, max_val=99999999,
    ))

  # 3. 角色属性
  if save_data:
    try:
      party = get_party_info(save_data)
    except Exception:
      party = []
    for member in party:
      actor_id = member["id"]
      actor_name_str = member.get("name", actor_names.get(actor_id, f"角色 {actor_id}"))
      # HP
      result.fields.append(ModifiableField(
        category="actor",
        field_id=f"actor_{actor_id}_hp",
        display_name=f"{actor_name_str} - HP",
        item_id=actor_id,
        field_type="int",
        save_value=member.get("hp"),
        min_val=0, max_val=member.get("mhp", 9999),
        description="当前生命值",
      ))
      # MP
      result.fields.append(ModifiableField(
        category="actor",
        field_id=f"actor_{actor_id}_mp",
        display_name=f"{actor_name_str} - MP",
        item_id=actor_id,
        field_type="int",
        save_value=member.get("mp"),
        min_val=0, max_val=member.get("mmp", 9999),
        description="当前魔法值",
      ))
      # Level
      result.fields.append(ModifiableField(
        category="actor",
        field_id=f"actor_{actor_id}_level",
        display_name=f"{actor_name_str} - 等级",
        item_id=actor_id,
        field_type="int",
        save_value=member.get("level", 1),
        min_val=1, max_val=99,
        description="角色等级",
      ))

  # 4. 金币 (智能检测：部分游戏用变量存金币而非 party._gold)
  if save_data:
    # 检测金币相关变量名（中/英/日）
    gold_var_id = 0
    gold_keywords = ["所持金", "金币", "金钱", "Gold", "money", "お金", "ゴールド"]
    for var_id, var_name in variable_names.items():
      for kw in gold_keywords:
        if kw.lower() in var_name.lower():
          gold_var_id = var_id
          break
      if gold_var_id:
        break

    party_gold = get_gold(save_data)
    gold_display_value = party_gold
    gold_description = "队伍金币"

    # 如果 party._gold 为空（=0）但存在金币变量，使用变量值
    if gold_var_id > 0 and party_gold == 0:
      var_gold = save_variables.get(gold_var_id, 0)
      if var_gold > 0:
        gold_display_value = var_gold
        gold_description = "金币 (来自变量 #{}: {})".format(gold_var_id,
          variable_names.get(gold_var_id, "?"))

    result.fields.insert(0, ModifiableField(
      category="gold",
      field_id="gold",
      display_name="金币",
      item_id=0,
      field_type="int",
      save_value=gold_display_value,
      min_val=0, max_val=99999999,
      description=gold_description,
      gold_var_id=gold_var_id,
    ))

  # 5. 自开关
  for key in sorted(save_self_switches.keys()):
    result.fields.append(ModifiableField(
      category="self_switch",
      field_id=f"ss_{key}",
      display_name=f"自开关 [{key}]",
      item_id=0,
      field_type="bool",
      save_value=save_self_switches.get(key, False),
      default_value=False,
      min_val=0, max_val=1,
      description=f"地图/事件自开关: {key}",
    ))

  # 按类别分组
  result.categories = {}
  for f in result.fields:
    cat = f.category
    if cat not in result.categories:
      result.categories[cat] = []
    result.categories[cat].append(f)

  return result


def filter_by_category(result: GameScanResult, category: str) -> list[ModifiableField]:
  """按类别过滤字段"""
  return result.categories.get(category, [])


def search_fields(result: GameScanResult, query: str) -> list[ModifiableField]:
  """搜索字段（按名称或 ID）"""
  q = query.lower()
  return [
    f for f in result.fields
    if q in f.display_name.lower() or str(f.item_id) == q
  ]


def get_field_summary(field: ModifiableField) -> str:
  """获取字段值的简短摘要"""
  parts = [field.display_name]
  if field.save_value is not None:
    parts.append(f"存档: {field.save_value}")
  if field.live_value is not None:
    parts.append(f"实时: {field.live_value}")
  return " | ".join(parts)


# ── 辅助函数 ──────────────────────────────────────────

def _find_data_dir(game_dir: str) -> str | None:
  """查找 www/data/ 目录"""
  for path in ["www/data", "data"]:
    d = os.path.join(game_dir, path)
    if os.path.isdir(d) and os.path.isfile(os.path.join(d, "System.json")):
      return d
  return None


def _load_json(data_dir: str, filename: str) -> dict | list | None:
  """安全加载 JSON 数据文件"""
  if not data_dir:
    return None
  path = os.path.join(data_dir, filename)
  if not os.path.isfile(path):
    return None
  try:
    with open(path, "r", encoding="utf-8") as f:
      return json.load(f)
  except Exception:
    return None
