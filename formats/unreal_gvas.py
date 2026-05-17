"""Unreal Engine GVAS 格式处理器

Unreal Engine 4/5 存档格式 (.sav)，魔数 "GVAS"。
广泛用于: Palworld, Satisfactory, Hogwarts Legacy, 等 UE 游戏。

GVAS 结构:
  [Header] SaveGameVersion + PackageVersion + EngineVersion + SaveGameType
  [Properties] 类型标记的属性列表 (IntProperty, StrProperty, FloatProperty, ...)

本处理器提取可读属性为可编辑字段，完整保留原始二进制用于往返。
"""
import io
import os
import re
import struct
import shutil
from datetime import datetime
from core.save_format import ISaveFormat, ModifiableField, SaveSummary


class UnrealGVASFormat(ISaveFormat):
    """Unreal Engine GVAS 存档格式处理器"""

    MAGIC = b"GVAS"

    @property
    def name(self) -> str:
        return "Unreal Engine (GVAS)"

    @property
    def extensions(self) -> list[str]:
        return [".sav"]

    @property
    def engine_type(self) -> str:
        return "unreal"

    @property
    def compatible_bridges(self) -> list[str]:
        return ["frida"]

    @property
    def magic_bytes(self) -> bytes | None:
        return self.MAGIC

    # ── 检测 ────────────────────────────────────────

    def detect(self, filepath: str) -> bool:
        ext = os.path.splitext(filepath)[1].lower()
        if ext != ".sav":
            return False
        try:
            with open(filepath, "rb") as f:
                return f.read(4) == self.MAGIC
        except OSError:
            return False

    # ── 核心 I/O ────────────────────────────────────

    def load(self, filepath: str) -> dict:
        """加载 GVAS 存档 → 提取属性字典 + 保留原始二进制"""
        with open(filepath, "rb") as f:
            raw = f.read()

        if len(raw) < 4 or raw[:4] != self.MAGIC:
            raise ValueError("不是有效的 GVAS 存档文件")

        header = self._parse_header(raw)
        props = self._extract_properties(raw)

        return {
            "_format": "gvas",
            "_raw": raw,
            "_header": header,
            "_props": props,
        }

    def save(self, filepath: str, data: dict) -> None:
        """保存 GVAS 存档 — 写回原始二进制（属性修改暂为只读）

        当前实现：完整保留原始二进制。属性值修改在后续版本中
        通过二进制补丁方式实现。
        """
        raw = data.get("_raw")
        if not raw:
            raise ValueError("存档数据不完整 (缺少 _raw)")

        # 备份
        if os.path.isfile(filepath):
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            backup_path = f"{filepath}.{timestamp}.bak"
            shutil.copy2(filepath, backup_path)

        os.makedirs(os.path.dirname(filepath) or ".", exist_ok=True)
        with open(filepath, "wb") as f:
            f.write(raw)

    # ── 头部解析 ────────────────────────────────────

    def _parse_header(self, raw: bytes) -> dict:
        """解析 GVAS 头部"""
        header = {"magic": raw[:4].decode("ascii")}
        offset = 4

        try:
            # SaveGameVersion (int32 LE)
            header["save_game_version"] = struct.unpack_from("<i", raw, offset)[0]
            offset += 4

            # PackageVersion (int32 LE)
            header["package_version"] = struct.unpack_from("<i", raw, offset)[0]
            offset += 4

            # EngineVersion: Major(uint16), Minor(uint16), Patch(uint16), Changelist(uint32)
            eng = struct.unpack_from("<HHHI", raw, offset)
            header["engine_version"] = {
                "major": eng[0], "minor": eng[1],
                "patch": eng[2], "changelist": eng[3],
            }
            offset += 12

            # Branch string (16 bytes, null-terminated)
            branch_raw = raw[offset:offset + 16]
            branch = branch_raw.split(b"\x00")[0].decode("ascii", errors="replace")
            header["branch"] = branch
            offset += 16

            # CustomFormatVersion
            header["custom_format_version"] = struct.unpack_from("<i", raw, offset)[0]
            offset += 4

            # CustomFormatData count
            fmt_count = struct.unpack_from("<i", raw, offset)[0]
            offset += 4

            # Skip CustomFormatData entries (16 bytes GUID + 4 bytes int32 each)
            offset += fmt_count * 20

            # SaveGameType string (int32 length + chars)
            type_len = struct.unpack_from("<i", raw, offset)[0]
            offset += 4
            if 0 < type_len < 256 and offset + type_len <= len(raw):
                header["save_game_type"] = raw[offset:offset + type_len - 1].decode(
                    "utf-8", errors="replace")
                offset += type_len
        except (struct.error, IndexError):
            pass

        header["_data_offset"] = offset
        return header

    # ── 属性提取 ────────────────────────────────────

    def _extract_properties(self, raw: bytes) -> dict:
        """从二进制中提取可读的属性名/值对

        扫描 null 结尾的字符串作为候选属性名，
        尝试匹配后续的类型标记和值。
        """
        props = {}

        try:
            # 从 header 末尾开始扫描
            offset = 16  # 最小安全偏移（跳过魔数+头部初始字段）
            text = raw.decode("latin-1")

            # 查找常见的 UE 属性名模式
            # 属性名通常是驼峰命名的英文单词，后跟类型标记
            prop_pattern = re.finditer(
                rb"([A-Z][a-zA-Z0-9_]{2,40})\x00([\x02-\x10])",
                raw[offset:offset + 65536]  # 扫描前 64KB
            )

            for match in prop_pattern:
                try:
                    name = match.group(1).decode("ascii")
                    type_byte = match.group(2)[0]
                    val_offset = offset + match.end()

                    if type_byte == 0x02:  # IntProperty
                        if val_offset + 8 <= len(raw):
                            val = struct.unpack_from("<q", raw, val_offset)[0]
                            props[name] = val
                    elif type_byte == 0x03:  # FloatProperty
                        if val_offset + 4 <= len(raw):
                            val = struct.unpack_from("<f", raw, val_offset)[0]
                            props[name] = round(val, 6)
                    elif type_byte == 0x04:  # StrProperty
                        if val_offset + 4 <= len(raw):
                            slen = struct.unpack_from("<i", raw, val_offset)[0]
                            if 0 < slen < 1024 and val_offset + 4 + slen <= len(raw):
                                s = raw[val_offset + 4:val_offset + 4 + slen - 1]
                                try:
                                    props[name] = s.decode("utf-8")
                                except UnicodeDecodeError:
                                    props[name] = s.decode("latin-1")
                    elif type_byte == 0x08:  # BoolProperty
                        if val_offset < len(raw):
                            props[name] = raw[val_offset] == 1
                except (struct.error, IndexError, UnicodeDecodeError):
                    continue

        except Exception:
            pass

        return props

    # ── 摘要 ────────────────────────────────────────

    def get_summary(self, data: dict) -> SaveSummary:
        header = data.get("_header", {})
        props = data.get("_props", {})
        eng = header.get("engine_version", {})

        return SaveSummary(
            gold=props.get("Gold", props.get("Money", 0)),
            party_size=0,
            item_count=len(props),
            save_count=1,
            play_time=props.get("PlayTime", props.get("RealTimeSeconds", 0)),
            extra={
                "engine": f"UE{eng.get('major', '?')}.{eng.get('minor', '?')}",
                "branch": header.get("branch", ""),
                "save_type": header.get("save_game_type", "?"),
                "package_version": header.get("package_version", 0),
                "prop_count": len(props),
            },
        )

    # ── 字段扫描 ────────────────────────────────────

    def scan_fields(self, data: dict, game_dir: str) -> list[ModifiableField]:
        """扫描 GVAS 存档中提取的属性"""
        fields = []
        props = data.get("_props", {})

        # 常见游戏属性名称映射
        KNOWN_NAMES = {
            "Gold": "金币",
            "Money": "金钱",
            "Health": "生命值",
            "HP": "生命值",
            "MaxHealth": "最大生命值",
            "Level": "等级",
            "Experience": "经验值",
            "PlayTime": "游戏时间",
            "RealTimeSeconds": "游戏时间(秒)",
            "PlayerName": "角色名",
            "SaveSlotName": "存档槽名",
        }

        for prop_name, prop_value in sorted(props.items()):
            display = KNOWN_NAMES.get(prop_name, prop_name)

            if isinstance(prop_value, bool):
                ftype, min_v, max_v = "bool", 0, 1
            elif isinstance(prop_value, float):
                ftype, min_v, max_v = "int", 0, 99999999
            elif isinstance(prop_value, str):
                ftype, min_v, max_v = "str", 0, 1
            else:
                ftype, min_v, max_v = "int", 0, 99999999

            fields.append(ModifiableField(
                category="gvas_prop",
                field_id=f"gvas_{prop_name}",
                display_name=display,
                item_id=0,
                field_type=ftype,
                save_value=prop_value,
                min_val=min_v,
                max_val=max_v,
                description=f"UE 属性: {prop_name}",
            ))

        return fields

    # ── 字段写回 ────────────────────────────────────

    def apply_field(self, data: dict, field: ModifiableField) -> None:
        """将字段修改写回属性字典（当前为只读，后续实现二进制补丁）"""
        prop_name = field.field_id[5:]  # 去掉 "gvas_" 前缀
        if "_props" in data:
            data["_props"][prop_name] = field.save_value

    # ── 游戏目录发现 ────────────────────────────────

    def find_data_dir(self, game_dir: str) -> str | None:
        """查找 UE 存档目录 (Saved/SaveGames/)"""
        for sub in ["Saved/SaveGames", "Saved", "SaveGames"]:
            d = os.path.join(game_dir, sub)
            if os.path.isdir(d):
                return d
        return None
