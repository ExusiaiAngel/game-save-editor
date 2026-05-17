import tempfile
import shutil
import json
import pytest


@pytest.fixture
def temp_dir():
    """创建一个临时目录，测试结束后自动清理。"""
    tmp = tempfile.mkdtemp()
    yield tmp
    shutil.rmtree(tmp)


@pytest.fixture
def sample_rpgsave_data():
    """返回一个最小化的 RPG Maker MV 存档字典。"""
    return {
        "version": 1,
        "switches": [],
        "variables": [],
        "selfSwitches": {},
        "items": {},
        "actors": {},
        "party": [],
    }
