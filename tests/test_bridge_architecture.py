"""游戏桥接架构测试

测试 IGameBridge 接口、BridgeFactory、GameConnection、
引擎检测和多后端注册。
"""
import pytest
from unittest.mock import MagicMock, patch

from core.game_bridge import (
  IGameBridge, GameState, BridgeFactory, GameConnection,
  TcpGameBridge, CdpGameBridge,
)
from core.bridge_backends import (
  register_all_backends,
  RpgMakerBridge, UnityMonoBridge, UnityIl2CppBridge,
  UnrealBridge, GenericMemoryBridge,
)
from core.engine_detect import (
  detect_engine_from_dir, EngineType, EngineInfo, get_engine_connect_hint,
)


# ═══════════════════════════════════════════════════════════════
# GameState 数据结构测试
# ═══════════════════════════════════════════════════════════════

def test_game_state_defaults():
    """GameState 默认值正确"""
    state = GameState()
    assert state.engine == "unknown"
    assert state.gold == 0
    assert state.party == []
    assert state.switches == {}
    assert state.variables == {}

def test_game_state_with_data():
    """GameState 可以填充各引擎数据"""
    state = GameState(
      engine="rpg_maker",
      gold=9999,
      switches={1: True, 5: False},
      variables={10: 42},
    )
    assert state.engine == "rpg_maker"
    assert state.gold == 9999
    assert state.switches[1] is True
    assert state.variables[10] == 42


# ═══════════════════════════════════════════════════════════════
# BridgeFactory 测试
# ═══════════════════════════════════════════════════════════════

def test_factory_registration():
    """工厂可以注册后端并排序"""
    factory = BridgeFactory()
    factory.register(GenericMemoryBridge)
    factory.register(TcpGameBridge)
    factory.register(CdpGameBridge)

    engines = factory.registered_engines
    # 应按优先级排序（数字越小越优先）
    assert engines[0] == "rpg_maker"      # 10
    assert engines[1] == "chromium"       # 20
    assert engines[2] == "generic_memory" # 90

def test_register_all_backends():
    """register_all_backends 注册所有引擎"""
    factory = register_all_backends()
    engines = factory.registered_engines
    assert "rpg_maker" in engines
    assert "chromium" in engines
    assert "unity_mono" in engines
    assert "unity_il2cpp" in engines
    assert "unreal" in engines
    assert "generic_memory" in engines
    assert len(engines) == 7


# ═══════════════════════════════════════════════════════════════
# IGameBridge 接口合规性测试
# ═══════════════════════════════════════════════════════════════

def test_tcp_bridge_implements_interface():
    """TcpGameBridge 正确实现 IGameBridge"""
    bridge = TcpGameBridge()
    assert isinstance(bridge, IGameBridge)
    assert hasattr(bridge, 'connect')
    assert hasattr(bridge, 'disconnect')
    assert hasattr(bridge, 'get_state')
    assert hasattr(bridge, 'set_gold')
    assert hasattr(bridge, 'set_switch')
    assert hasattr(bridge, 'set_variable')
    assert hasattr(bridge, 'set_actor_hp')
    assert hasattr(bridge, 'set_actor_mp')
    assert hasattr(bridge, 'set_item_count')
    assert bridge.is_connected is False
    assert bridge.engine_name() == "rpg_maker"
    assert bridge.priority() == 10

def test_cdp_bridge_implements_interface():
    """CdpGameBridge 正确实现 IGameBridge"""
    bridge = CdpGameBridge()
    assert isinstance(bridge, IGameBridge)
    assert bridge.is_connected is False
    assert bridge.engine_name() == "chromium"
    assert bridge.priority() == 20

def test_mock_bridge_interface():
    """Mock 实现的 IGameBridge 可以正常使用"""

    mock = MagicMock(spec=IGameBridge)
    mock.is_connected = True
    mock.engine_name = lambda: "mock"
    mock.get_state.return_value = GameState(engine="mock", gold=100)

    state = mock.get_state()
    assert state.engine == "mock"
    assert state.gold == 100
    mock.set_gold.assert_not_called()
    mock.set_gold(999)
    mock.set_gold.assert_called_once_with(999)


# ═══════════════════════════════════════════════════════════════
# GameConnection 测试
# ═══════════════════════════════════════════════════════════════

def test_game_connection_init():
    """GameConnection 初始化时注册内置后端"""
    conn = GameConnection()
    assert conn.is_connected is False
    assert conn.connection_type == ""

def test_game_connection_disconnect():
    """GameConnection.disconnect 安全处理未连接状态"""
    conn = GameConnection()
    conn.disconnect()  # 不应抛出异常
    assert conn.is_connected is False

def test_game_connection_register_backend():
    """GameConnection 可以注册额外后端"""
    conn = GameConnection()
    conn.register_backend(UnityMonoBridge)
    engines = conn.registered_engines
    assert "unity_mono" in engines


# ═══════════════════════════════════════════════════════════════
# 引擎检测测试
# ═══════════════════════════════════════════════════════════════

def test_engine_detect_unknown():
    """空目录返回未知引擎"""
    info = detect_engine_from_dir("")
    assert info.engine_type == EngineType.UNKNOWN

def test_get_connect_hint():
    """获取连接提示不为空"""
    hint = get_engine_connect_hint(EngineType.UNITY_MONO)
    assert "Frida" in hint or "frida" in hint.lower()

    hint_unknown = get_engine_connect_hint("garbage_type")
    assert len(hint_unknown) > 0

def test_engine_info_defaults():
    """EngineInfo 默认值"""
    info = EngineInfo()
    assert info.engine_type == EngineType.UNKNOWN
    assert info.engine_name == "未知引擎"
    assert info.modules == []
    assert info.is_64bit is True


# ═══════════════════════════════════════════════════════════════
# 后端优先级排序测试
# ═══════════════════════════════════════════════════════════════

def test_backend_priority_ordering():
    """后端按优先级正确排序"""
    factory = register_all_backends()
    engines = factory.registered_engines

    # RPG Maker 应该最高优先级
    assert engines[0] == "rpg_maker"
    # 通用内存应该最低优先级
    assert engines[-1] == "generic_memory"

def test_all_backends_have_priorities():
    """所有后端都有非默认优先级"""
    factory = register_all_backends()
    for bridge_cls in factory._registry:
        assert bridge_cls.priority() > 0
        assert bridge_cls.engine_name() != "generic"  # 都覆盖了默认值


# ═══════════════════════════════════════════════════════════════
# GameState 引擎兼容性测试
# ═══════════════════════════════════════════════════════════════

def test_game_state_accepts_engine_specific_data():
    """不同引擎的 GameState 可以携带不同字段"""
    rpg_state = GameState(
      engine="rpg_maker",
      switches={1: True, 2: False},
      variables={10: 500},
      self_switches={"1,5,A": True},
    )
    assert len(rpg_state.switches) == 2
    assert "1,5,A" in rpg_state.self_switches

    unity_state = GameState(
      engine="unity_mono",
      gold=1234,
      raw={"PlayerData": {"hp": 100, "mp": 50}},
    )
    assert unity_state.raw is not None
    assert unity_state.raw["PlayerData"]["hp"] == 100
