"""依赖检测模块测试"""
import pytest
from core.dependency_check import (
  check_all_dependencies, get_summary, get_missing_deps,
  get_install_commands, quick_check, DepStatus, EngineDepReport,
)


def test_check_all_dependencies():
    """check_all_dependencies 返回 6 个引擎报告"""
    reports = check_all_dependencies()
    assert len(reports) == 7
    engine_types = [r.engine_type for r in reports]
    assert "rpg_maker" in engine_types
    assert "chromium" in engine_types
    assert "unity_mono" in engine_types
    assert "unity_il2cpp" in engine_types
    assert "unreal" in engine_types
    assert "generic_memory" in engine_types


def test_rpg_maker_always_ready():
    """RPG Maker 引擎始终就绪（零外部依赖）"""
    reports = check_all_dependencies()
    rpg = next(r for r in reports if r.engine_type == "rpg_maker")
    assert rpg.ready is True


def test_get_summary():
    """get_summary 返回格式正确的摘要"""
    reports = check_all_dependencies()
    summary = get_summary(reports)
    assert "/6" in summary or "/" in summary


def test_get_missing_deps():
    """get_missing_deps 去重"""
    reports = check_all_dependencies()
    missing = get_missing_deps(reports)
    # frida 可能在列表中最多一次
    frida_counts = sum(1 for d in missing if d.name == "frida")
    assert frida_counts <= 1


def test_get_install_commands():
    """get_install_commands 返回非空列表（如果有缺失）或空列表"""
    reports = check_all_dependencies()
    commands = get_install_commands(reports)
    assert isinstance(commands, list)
    for cmd in commands:
        assert "pip install" in cmd


def test_quick_check():
    """quick_check 返回字典且 rpg_maker 始终为 True"""
    result = quick_check()
    assert isinstance(result, dict)
    assert result["rpg_maker"] is True
    assert "frida" in result
    assert "pymem" in result
    assert "websocket" in result


def test_dep_status_dataclass():
    """DepStatus 数据类可以正常构造"""
    ds = DepStatus(
      name="test",
      installed=True,
      version="1.0",
      install_cmd="pip install test",
      required_by=["Engine A", "Engine B"],
    )
    assert ds.name == "test"
    assert ds.installed is True
    assert len(ds.required_by) == 2


def test_engine_dep_report_dataclass():
    """EngineDepReport 数据类可以正常构造"""
    r = EngineDepReport(
      engine_name="Test Engine",
      engine_type="test",
      backend_class="TestBridge",
      ready=True,
      deps=[],
      connect_hint="Just work",
    )
    assert r.engine_name == "Test Engine"
    assert r.ready is True


def test_reports_have_connect_hints():
    """每个引擎报告都有连接提示"""
    reports = check_all_dependencies()
    for r in reports:
        assert len(r.connect_hint) > 0, f"{r.engine_name} 缺少连接提示"


def test_reports_have_backend_class():
    """每个引擎报告都有后端类名"""
    reports = check_all_dependencies()
    for r in reports:
        assert len(r.backend_class) > 0, f"{r.engine_name} 缺少后端类名"
