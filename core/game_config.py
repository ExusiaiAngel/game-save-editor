"""RPG Maker MV 游戏配置自动生成器

扫描游戏目录，自动识别所有可编辑字段，生成配置映射。
适用于任意 RPG Maker MV 游戏。
"""
import json
import os


def scan_game_directory(game_dir):
    """扫描 RPG Maker MV 游戏目录，提取所有游戏数据配置
    
    Args:
        game_dir: 游戏根目录路径
    
    Returns:
        完整配置字典，包含所有游戏数据的名称映射和字段定义
    """
    config = {
        "game_dir": game_dir,
        "game_title": "",
        "currency_unit": "G",
        "data_loaded": False,
    }
    
    # 查找数据目录
    data_dir = _find_data_dir(game_dir)
    if not data_dir:
        return config
    
    try:
        _load_system(config, data_dir)
        _load_actors(config, data_dir)
        _load_classes(config, data_dir)
        _load_items(config, data_dir)
        _load_weapons(config, data_dir)
        _load_armors(config, data_dir)
        _load_skills(config, data_dir)
        _load_states(config, data_dir)
        config["data_loaded"] = True
    except Exception as e:
        config["load_error"] = str(e)
    
    return config


def _find_data_dir(game_dir):
    """查找 www/data/ 目录"""
    for path in ["www/data", "data", "../data"]:
        d = os.path.join(game_dir, path)
        if os.path.isdir(d) and os.path.isfile(os.path.join(d, "System.json")):
            return d
    return None


def _load_json(data_dir, filename):
    """安全加载 JSON 数据文件"""
    path = os.path.join(data_dir, filename)
    if not os.path.isfile(path):
        return None
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def _load_system(config, data_dir):
    """加载系统设置"""
    sys = _load_json(data_dir, "System.json")
    if not sys:
        return
    config["game_title"] = sys.get("gameTitle", "")
    config["currency_unit"] = sys.get("currencyUnit", "G")
    
    # 开关名称
    switches = sys.get("switches", [])
    config["switch_names"] = {}
    for i, name in enumerate(switches):
        if name and isinstance(name, str) and name.strip():
            config["switch_names"][i] = name.strip()
    
    # 变量名称
    variables = sys.get("variables", [])
    config["variable_names"] = {}
    for i, name in enumerate(variables):
        if name and isinstance(name, str) and name.strip():
            config["variable_names"][i] = name.strip()
    
    # 术语
    terms = sys.get("terms", {})
    config["terms"] = {
        "params": terms.get("params", ["MHP", "MMP", "ATK", "DEF", "MAT", "MDF", "AGI", "LUK"]),
        "commands": terms.get("commands", []),
        "basic": terms.get("basic", []),
    }
    config["equip_types"] = sys.get("equipTypes", ["", "武器", "盾", "头", "身体", "装饰品"])
    config["skill_types"] = sys.get("skillTypes", [])
    config["armor_types"] = sys.get("armorTypes", [])


def _load_actors(config, data_dir):
    """加载角色定义"""
    data = _load_json(data_dir, "Actors.json")
    config["actor_names"] = {}
    if data:
        for i, actor in enumerate(data):
            if actor and isinstance(actor, dict):
                config["actor_names"][i] = actor.get("name", f"Actor#{i}")


def _load_classes(config, data_dir):
    """加载职业定义"""
    data = _load_json(data_dir, "Classes.json")
    config["class_names"] = {}
    if data:
        for i, cls in enumerate(data):
            if cls and isinstance(cls, dict):
                config["class_names"][i] = cls.get("name", f"Class#{i}")


def _load_items(config, data_dir):
    """加载物品定义"""
    data = _load_json(data_dir, "Items.json")
    config["item_names"] = {}
    config["item_info"] = {}
    if data:
        for i, item in enumerate(data):
            if item and isinstance(item, dict):
                config["item_names"][i] = item.get("name", f"Item#{i}")
                config["item_info"][i] = {
                    "price": item.get("price", 0),
                    "consumable": item.get("consumable", True),
                    "itype_id": item.get("itypeId", 1),
                }


def _load_weapons(config, data_dir):
    """加载武器定义"""
    data = _load_json(data_dir, "Weapons.json")
    config["weapon_names"] = {}
    if data:
        for i, w in enumerate(data):
            if w and isinstance(w, dict):
                config["weapon_names"][i] = w.get("name", f"Weapon#{i}")


def _load_armors(config, data_dir):
    """加载防具定义"""
    data = _load_json(data_dir, "Armors.json")
    config["armor_names"] = {}
    if data:
        for i, a in enumerate(data):
            if a and isinstance(a, dict):
                config["armor_names"][i] = a.get("name", f"Armor#{i}")


def _load_skills(config, data_dir):
    """加载技能定义"""
    data = _load_json(data_dir, "Skills.json")
    config["skill_names"] = {}
    if data:
        for i, s in enumerate(data):
            if s and isinstance(s, dict):
                config["skill_names"][i] = s.get("name", f"Skill#{i}")


def _load_states(config, data_dir):
    """加载状态定义"""
    data = _load_json(data_dir, "States.json")
    config["state_names"] = {}
    if data:
        for i, s in enumerate(data):
            if s and isinstance(s, dict):
                config["state_names"][i] = s.get("name", f"State#{i}")
                if s.get("name", "").startswith("(空"):
                    config["state_names"][i] = ""


def generate_editable_fields(config):
    """根据配置生成所有可编辑字段列表"""
    fields = []
    
    # 金钱
    fields.append({
        "field_name": "party._gold",
        "display_name": "金币",
        "field_type": "int",
        "min": 0, "max": 99999999, "step": 100,
    })
    
    # 步数
    fields.append({
        "field_name": "party._steps",
        "display_name": "步数",
        "field_type": "int",
        "min": 0, "max": 999999, "step": 1,
    })
    
    # 物品
    if config.get("item_names"):
        for iid in sorted(config["item_names"].keys()):
            fields.append({
                "field_name": f"party._items[{iid}]",
                "display_name": config["item_names"][iid],
                "field_type": "int",
                "min": 0, "max": 99, "step": 1,
            })
    
    # 武器
    if config.get("weapon_names"):
        for wid in sorted(config["weapon_names"].keys()):
            fields.append({
                "field_name": f"party._weapons[{wid}]",
                "display_name": config["weapon_names"][wid],
                "field_type": "int",
                "min": 0, "max": 99, "step": 1,
            })
    
    # 防具
    if config.get("armor_names"):
        for aid in sorted(config["armor_names"].keys()):
            fields.append({
                "field_name": f"party._armors[{aid}]",
                "display_name": config["armor_names"][aid],
                "field_type": "int",
                "min": 0, "max": 99, "step": 1,
            })
    
    return fields


def item_name(config, item_id):
    """获取物品名称"""
    return config.get("item_names", {}).get(item_id, f"Item#{item_id}")


def weapon_name(config, weapon_id):
    """获取武器名称"""
    return config.get("weapon_names", {}).get(weapon_id, f"Weapon#{weapon_id}")


def armor_name(config, armor_id):
    """获取防具名称"""
    return config.get("armor_names", {}).get(armor_id, f"Armor#{armor_id}")


def actor_name(config, actor_id):
    """获取角色名称"""
    return config.get("actor_names", {}).get(actor_id, f"Actor#{actor_id}")


def class_name(config, class_id):
    """获取职业名称"""
    return config.get("class_names", {}).get(class_id, f"Class#{class_id}")


def skill_name(config, skill_id):
    """获取技能名称"""
    return config.get("skill_names", {}).get(skill_id, f"Skill#{skill_id}")


def switch_name(config, sw_id):
  """获取开关名称（System.json 索引即开关ID）"""
  return config.get("switch_names", {}).get(sw_id, f"Switch#{sw_id}")


def variable_name(config, var_id):
  """获取变量名称（System.json 索引即变量ID）"""
  return config.get("variable_names", {}).get(var_id, f"Var#{var_id}")
