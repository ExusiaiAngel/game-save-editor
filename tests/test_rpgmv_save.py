"""RPG Maker MV 存档模块测试 (TDD)

测试 load_save, save_save, get_gold, set_gold, get_items, set_actor_hp 的
主要功能以及 save_save 的自动备份机制。
"""

import json
from pathlib import Path

import pytest

from core.lzstring import compress_to_base64
from core.rpgmv_save import (
    load_save,
    save_save,
    get_gold,
    set_gold,
    get_items,
    set_item_count,
    get_party_info,
    set_actor_hp,
    get_switches,
    get_variables,
    set_switch,
    set_variable,
)

# ── 测试数据 ──────────────────────────────────────────

SAMPLE_DATA = {
    "party": {"_gold": 3000},
    "actors": {
        "_data": [
            None,
            {
                "_name": "Player",
                "_hp": 100,
                "_mhp": 150,
                "_mp": 50,
                "_mmp": 75,
                "_level": 5,
            },
        ]
    },
}


# JsonEx @c 格式测试数据
ITEMS_WITH_AT_C = {
    "party": {
        "_gold": 1000,
        "_items": {
            "@c": ["test compressed array"],
            "@a": {"1": 5, "2": 3, "3": 10},
        },
    },
}

ACTORS_WITH_AT_C = {
    "party": {
        "_actors": {
            "@c": [[1, 2]],
            "@a": [1],
        },
    },
    "actors": {
        "_data": [
            None,
            {"_name": "TestHero", "_hp": 200, "_mhp": 300, "_mp": 50, "_mmp": 100, "_level": 10},
        ],
    },
}


# ── Fixtures ─────────────────────────────────────────


@pytest.fixture
def temp_save_path(temp_dir):
    """创建一个包含有效数据的临时 .rpgsave 文件并返回路径"""
    path = Path(temp_dir) / "save.rpgsave"
    json_str = json.dumps(SAMPLE_DATA, ensure_ascii=False, separators=(",", ":"))
    compressed = compress_to_base64(json_str)
    path.write_text(compressed, encoding="utf-8")
    return str(path)


# ── 测试用例 ─────────────────────────────────────────


def test_load_save_valid(temp_save_path):
    """a) 测试加载有效存档，验证 gold 正确"""
    data = load_save(temp_save_path)
    assert get_gold(data) == 3000


def test_save_save_roundtrip(temp_save_path):
    """b) 测试 load → set_gold → save → reload 往返一致性"""
    data = load_save(temp_save_path)
    assert get_gold(data) == 3000

    set_gold(data, 99999)
    save_save(temp_save_path, data)

    reloaded = load_save(temp_save_path)
    assert get_gold(reloaded) == 99999


def test_set_gold_negative_clamped():
    """c) 测试负值金币被钳制为 0"""
    data = {"party": {"_gold": 100}}
    set_gold(data, -100)
    assert get_gold(data) == 0


def test_get_items_empty():
    """d) 测试无物品时返回空列表"""
    data = {"party": {}}
    assert get_items(data) == []


def test_set_actor_hp_valid():
    """e) 测试设置有效角色的 HP"""
    data = {
        "actors": {"_data": [None, {"_name": "Player", "_hp": 100, "_mhp": 150}]}
    }
    set_actor_hp(data, 1, 50)
    assert data["actors"]["_data"][1]["_hp"] == 50


def test_backup_on_save(temp_save_path, temp_dir):
    """f) 测试 save_save 时自动创建 .bak 备份文件"""
    data = load_save(temp_save_path)
    set_gold(data, 5000)
    save_save(temp_save_path, data)

    bak_files = list(Path(temp_dir).glob("*.bak"))
    assert len(bak_files) >= 1, "应该生成至少一个 .bak 备份文件"


def test_items_with_at_c_skipped():
    """JsonEx @c 格式：get_items 跳过 @ 元数据键，只返回实际物品"""
    items = get_items(ITEMS_WITH_AT_C)
    item_ids = {i["id"] for i in items}
    # 应该包含 @a 中的物品 (1, 2) 和直接列出的物品 (3)
    assert 1 in item_ids
    assert 2 in item_ids
    assert 3 in item_ids
    # 不应该包含 @c 键本身
    for item in items:
        assert not isinstance(item["id"], str) or not item["id"].startswith("@")


def test_party_with_at_c_actors():
    """JsonEx @c 格式：get_party_info 跳过 @ 元数据键，正确解析队伍成员"""
    party = get_party_info(ACTORS_WITH_AT_C)
    assert len(party) >= 1
    # @a 中 actor_id=1，应该能找到 TestHero
    hero = next((m for m in party if m["name"] == "TestHero"), None)
    assert hero is not None
    assert hero["level"] == 10


def test_set_item_count_with_at_a_format():
    """JsonEx @a 格式：set_item_count 写入 @a 而不是覆盖结构"""
    data = {
        "party": {
            "_items": {
                "@c": ["compressed data"],
                "@a": {"1": 5, "2": 3},
            },
        },
    }
    set_item_count(data, 1, 10)
    set_item_count(data, 3, 7)
    # @a 应该保留，@c 应该保留
    assert "@a" in data["party"]["_items"]
    assert "@c" in data["party"]["_items"]
    assert data["party"]["_items"]["@a"]["1"] == 10
    assert data["party"]["_items"]["@a"]["2"] == 3
    assert data["party"]["_items"]["@a"]["3"] == 7


def test_set_item_count_simple_format():
    """简单格式：set_item_count 正常写入"""
    data = {"party": {"_items": {"1": 5}}}
    set_item_count(data, 1, 99)
    assert data["party"]["_items"]["1"] == 99


def test_set_item_count_negative_clamped():
    """set_item_count 负值钳制为 0"""
    data = {"party": {"_items": {"1": 5}}}
    set_item_count(data, 1, -10)
    # 负值被钳制为 0，但 get_items 过滤掉 count=0 的物品是正确行为
    assert data["party"]["_items"]["1"] == 0
    assert get_items(data) == []


# ═══════════════════════════════════════════════════════
# 纯列表格式测试 — 修复 _ensure_*_array 数据销毁 bug
# ═══════════════════════════════════════════════════════

def test_set_variable_plain_list_format():
    """纯列表格式：set_variable 不销毁原有数据"""
    data = {
        "variables": {
            "_data": [None, 100, 200, 300]  # 纯列表，index 0=None, 1=100, 2=200, 3=300
        }
    }
    set_variable(data, 1, 999)
    # 验证修改成功
    assert get_variables(data).get(1) == 999
    # 验证原有数据未被销毁
    assert get_variables(data).get(2) == 200
    assert get_variables(data).get(3) == 300
    # 验证 _data 仍是纯列表格式
    assert isinstance(data["variables"]["_data"], list)


def test_set_switch_plain_list_format():
    """纯列表格式：set_switch 不销毁原有数据"""
    data = {
        "switches": {
            "_data": [False, False, True, False]  # 纯列表, switch[2]=True
        }
    }
    set_switch(data, 1, True)
    # 验证修改成功
    assert get_switches(data).get(1) == True
    # 验证原有数据未被销毁
    assert get_switches(data).get(2) == True
    # 验证 _data 仍是纯列表格式
    assert isinstance(data["switches"]["_data"], list)


def test_set_variable_plain_list_preserves_roundtrip(temp_dir):
    """纯列表格式：修改后 save → load 往返不丢失数据"""
    from pathlib import Path
    data = {
        "variables": {
            "_data": [None, 10, 20, 30, 40, 50]  # 5 个有效变量
        },
        "party": {},
    }
    path = str(Path(temp_dir) / "save_plain_list.rpgsave")
    save_save(path, data)

    # 加载后修改
    loaded = load_save(path)
    set_variable(loaded, 1, 999)
    set_variable(loaded, 3, 888)
    save_save(path, loaded)

    # 重新加载验证
    reloaded = load_save(path)
    vars_result = get_variables(reloaded)
    assert vars_result.get(1) == 999
    assert vars_result.get(2) == 20   # 未修改的保留
    assert vars_result.get(3) == 888
    assert vars_result.get(4) == 40   # 未修改的保留
    assert vars_result.get(5) == 50   # 未修改的保留


def test_set_variable_dict_format_preserves_at_c(temp_dir):
    """JsonEx @c/@a 格式：set_variable 保留 @c 字段"""
    from pathlib import Path
    data = {
        "variables": {
            "_data": {
                "@c": 115,
                "@a": [0, 10000000, 1, 700000, 50],
            }
        },
        "party": {},
    }
    path = str(Path(temp_dir) / "save_dict_format.rpgsave")
    save_save(path, data)

    loaded = load_save(path)
    set_variable(loaded, 3, 99999)
    save_save(path, loaded)

    reloaded = load_save(path)
    # 验证 @c 字段未被移除
    var_data = reloaded["variables"]["_data"]
    assert isinstance(var_data, dict)
    assert "@c" in var_data
    assert "@a" in var_data
    # 验证变量值正确
    vars_result = get_variables(reloaded)
    assert vars_result.get(1) == 10000000
    assert vars_result.get(3) == 99999


def test_set_switch_dict_format_preserves_at_c(temp_dir):
    """JsonEx @c/@a 格式：set_switch 保留 @c 字段"""
    from pathlib import Path
    data = {
        "switches": {
            "_data": {
                "@c": 113,
                "@a": [False, False, True, False],
            }
        },
        "party": {},
    }
    path = str(Path(temp_dir) / "save_sw_dict.rpgsave")
    save_save(path, data)

    loaded = load_save(path)
    set_switch(loaded, 1, True)
    set_switch(loaded, 2, False)
    save_save(path, loaded)

    reloaded = load_save(path)
    sw_data = reloaded["switches"]["_data"]
    assert isinstance(sw_data, dict)
    assert "@c" in sw_data
    assert "@a" in sw_data
    sw_result = get_switches(reloaded)
    assert sw_result.get(1) == True
    assert sw_result.get(2) == False


def test_set_variable_extends_plain_list():
    """纯列表格式：设置超出长度的变量自动扩展列表"""
    data = {
        "variables": {
            "_data": [None, 10]  # 只有变量 1
        }
    }
    set_variable(data, 5, 555)
    arr = data["variables"]["_data"]
    assert len(arr) >= 6  # 扩展到 index 5
    assert arr[5] == 555
    # 原有数据保留
    assert arr[1] == 10
    assert get_variables(data).get(1) == 10
    assert get_variables(data).get(5) == 555
