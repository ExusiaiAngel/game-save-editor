"""通用 JSON 格式处理器

许多游戏引擎使用纯 JSON 作为存档格式:
  - Unity (部分游戏使用 JSON 存档)
  - Godot (store_var 或 JSON.stringify)
  - MonoGame / FNA
  - 大量独立游戏

此处理器自动检测 JSON 内容并提取所有可编辑字段。
支持嵌套 JSON 结构的扁平化编辑。
"""
import json
import os
import re
import shutil
from datetime import datetime
from core.save_format import ISaveFormat, ModifiableField, SaveSummary


def _flatten_json(obj: dict, prefix: str = "") -> dict:
    """将嵌套 JSON 展平为 key-path → value 映射

    {"player": {"hp": 100, "name": "Hero"}}
    → {"player.hp": 100, "player.name": "Hero"}
    """
    result = {}
    for key, value in obj.items():
        full_key = f"{prefix}.{key}" if prefix else key
        if isinstance(value, dict):
            result.update(_flatten_json(value, full_key))
        elif isinstance(value, list):
            # 列表：展平为 key[0], key[1], ...
            for i, item in enumerate(value):
                if isinstance(item, dict):
                    result.update(_flatten_json(item, f"{full_key}[{i}]"))
                else:
                    result[f"{full_key}[{i}]"] = item
        else:
            result[full_key] = value
    return result


def _unflatten_json(flat: dict) -> dict:
    """将展平的 key-path 还原为嵌套 JSON"""
    result = {}
    for key_path, value in flat.items():
        # 解析 key[0].subkey 格式
        parts = re.split(r"\.(?![^\[]*\])", key_path)  # 按 . 分割，但忽略 [] 内的
        current = result
        for i, part in enumerate(parts):
            # 检查是否为数组索引: name[0]
            match = re.match(r"^(.+)\[(\d+)\]$", part)
            if match:
                arr_name = match.group(1)
                idx = int(match.group(2))
                if arr_name not in current:
                    current[arr_name] = []
                arr = current[arr_name]
                while len(arr) <= idx:
                    arr.append({} if i < len(parts) - 1 else None)
                if i == len(parts) - 1:
                    arr[idx] = value
                else:
                    current = arr[idx]
            else:
                if i == len(parts) - 1:
                    current[part] = value
                else:
                    if part not in current:
                        current[part] = {}
                    current = current[part]
    return result


# ── 已知字段名映射 ────────────────────────────────

FIELD_NAME_MAP = {
    "gold": "金币", "money": "金钱", "coin": "金币",
    "hp": "生命值", "health": "生命值",
    "mp": "魔法值", "mana": "魔法值",
    "level": "等级", "lvl": "等级",
    "exp": "经验值", "experience": "经验值",
    "name": "名称", "playerName": "角色名",
    "playTime": "游戏时间", "playtime": "游戏时间",
    "score": "分数", "points": "分数",
    "atk": "攻击力", "attack": "攻击力",
    "def": "防御力", "defense": "防御力",
}

CATEGORY_MAP = {
    "gold": "gold", "money": "gold", "coin": "gold",
    "hp": "actor", "health": "actor", "mp": "actor", "mana": "actor",
    "level": "actor", "lvl": "actor",
    "atk": "actor", "attack": "actor", "def": "actor", "defense": "actor",
    "exp": "actor", "experience": "actor",
}


class GenericJsonFormat(ISaveFormat):
    """通用 JSON 存档格式处理器

    处理任何纯 JSON 格式的游戏存档。
    自动展平所有嵌套字段为可编辑列表。
    """

    @property
    def name(self) -> str:
        return "JSON (通用)"

    @property
    def extensions(self) -> list[str]:
        return [".json"]

    @property
    def engine_type(self) -> str:
        return "generic"

    @property
    def compatible_bridges(self) -> list[str]:
        # 通用 JSON 可能来自任何引擎
        return ["rpg_maker", "cdp", "frida", "bepinex", "pymem"]

    @property
    def magic_bytes(self) -> bytes | None:
        # JSON 文件无固定魔数，以 { 或 [ 开头
        return None

    # ── 检测 ────────────────────────────────────────

    def detect(self, filepath: str) -> bool:
        """检测是否为有效 JSON 存档"""
        ext = os.path.splitext(filepath)[1].lower()
        # 接受 .json 和 .sav/.dat 等可能包含 JSON 的文件
        if ext != ".json":
            return False
        try:
            with open(filepath, "r", encoding="utf-8") as f:
                json.load(f)
            return True
        except Exception:
            return False

    # ── 核心 I/O ────────────────────────────────────

    def load(self, filepath: str) -> dict:
        """加载 JSON 存档"""
        with open(filepath, "r", encoding="utf-8") as f:
            data = json.load(f)
        if not isinstance(data, dict):
            data = {"_root": data}
        # 展平嵌套结构
        flat = _flatten_json(data)
        return {
            "_format": "generic_json",
            "_root": data,
            "_flat": flat,
        }

    def save(self, filepath: str, data: dict) -> None:
        """保存 JSON 存档 — 从展平数据还原嵌套结构"""
        # 备份
        if os.path.isfile(filepath):
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            backup_path = f"{filepath}.{timestamp}.bak"
            shutil.copy2(filepath, backup_path)

        # 从展平数据重建嵌套结构
        flat = data.get("_flat", {})
        root = _unflatten_json(flat)

        # 如果原始顶层是列表，还原
        if "_root_list" in data:
            root = data["_root_list"]

        os.makedirs(os.path.dirname(filepath) or ".", exist_ok=True)
        with open(filepath, "w", encoding="utf-8") as f:
            json.dump(root, f, ensure_ascii=False, indent=2)

    # ── 摘要 ────────────────────────────────────────

    def get_summary(self, data: dict) -> SaveSummary:
        flat = data.get("_flat", {})
        gold = 0
        for key, val in flat.items():
            key_lower = key.lower()
            if any(kw in key_lower for kw in ("gold", "money", "coin")):
                if isinstance(val, (int, float)):
                    gold = int(val)
                    break

        return SaveSummary(
            gold=gold,
            party_size=0,
            item_count=len(flat),
            save_count=1,
            play_time=0,
            extra={
                "engine": "JSON (通用)",
                "field_count": len(flat),
            },
        )

    # ── 字段扫描 ────────────────────────────────────

    def scan_fields(self, data: dict, game_dir: str) -> list[ModifiableField]:
        """扫描所有展平字段"""
        fields = []
        flat = data.get("_flat", {})

        for key_path, value in sorted(flat.items()):
            # 确定字段类型
            if isinstance(value, bool):
                ftype, min_v, max_v = "bool", 0, 1
            elif isinstance(value, (int, float)):
                ftype, min_v, max_v = "int", -99999999, 99999999
            elif isinstance(value, str):
                ftype, min_v, max_v = "str", 0, 1
            else:
                continue  # 跳过 list/dict/None 等复杂类型

            # 查找友好名称
            key_basename = os.path.basename(key_path.replace(".", "/"))
            key_lower = key_basename.lower()
            display = FIELD_NAME_MAP.get(key_lower, key_path)
            category = CATEGORY_MAP.get(key_lower, "variable")

            fields.append(ModifiableField(
                category=category,
                field_id=f"json_{key_path}",
                display_name=display,
                item_id=0,
                field_type=ftype,
                save_value=value,
                min_val=min_v,
                max_val=max_v,
                description=f"JSON 路径: {key_path}",
            ))

        return fields

    # ── 字段写回 ────────────────────────────────────

    def apply_field(self, data: dict, field: ModifiableField) -> None:
        """将字段修改写回展平字典"""
        key_path = field.field_id[5:]  # 去掉 "json_" 前缀
        val = field.save_value

        # 类型转换
        if field.field_type == "int":
            val = int(val) if val is not None else 0
        elif field.field_type == "bool":
            val = bool(val)
        elif field.field_type == "str":
            val = str(val) if val is not None else ""

        if "_flat" in data:
            data["_flat"][key_path] = val

    # ── 游戏目录发现 ────────────────────────────────

    def find_data_dir(self, game_dir: str) -> str | None:
        """通用游戏数据目录查找"""
        for sub in ["data", "saves", "save", "game"]:
            d = os.path.join(game_dir, sub)
            if os.path.isdir(d):
                return d
        return game_dir if os.path.isdir(game_dir) else None
