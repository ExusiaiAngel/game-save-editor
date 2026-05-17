"""主窗口 — 游戏存档编辑器 (单一面板，统一工作流)"""
import os
from PySide6.QtWidgets import (
    QMainWindow, QVBoxLayout, QWidget,
    QStatusBar, QLabel, QMessageBox, QPushButton,
)
from PySide6.QtCore import Qt, QTimer

from ui.modify_panel import ModifyPanel
from core.game_bridge import GameConnection, IGameBridge
from core.game_detector import detect_game
from core.plugin_injector import is_plugin_installed, inject_plugin, get_plugin_status_text
from core.game_profile import get_profile_manager, GameProfile
from core.engine_detect import detect_engine_from_dir, get_engine_connect_hint
from core.dependency_check import check_all_dependencies, get_summary, get_missing_deps


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("游戏存档编辑器")
        self.resize(1000, 720)
        self.setMinimumSize(800, 600)

        # ── 核心组件 ──
        self.connection = GameConnection()
        self.connection.register_all_known_backends()
        self.profile_mgr = get_profile_manager()
        self.current_profile: GameProfile | None = None
        self.detected_engine: str = ""

        # ── 中央组件 ──
        central = QWidget()
        self.setCentralWidget(central)
        layout = QVBoxLayout(central)
        layout.setContentsMargins(6, 6, 6, 6)

        # 唯一面板：存档编辑 + 项目修改
        self.modify_panel = ModifyPanel()
        layout.addWidget(self.modify_panel)

        # ── 状态栏 ──
        self.status_bar = QStatusBar()
        self.setStatusBar(self.status_bar)

        self.lbl_game = QLabel("未检测到游戏")
        self.lbl_game.setStyleSheet("color: gray; padding: 0 8px;")
        self.status_bar.addWidget(self.lbl_game)

        self.lbl_plugin = QLabel("")
        self.lbl_plugin.setStyleSheet("padding: 0 8px;")
        self.status_bar.addWidget(self.lbl_plugin)

        self.lbl_connection = QLabel("● 未连接")
        self.lbl_connection.setStyleSheet("color: gray; font-weight: bold; padding: 0 8px;")
        self.status_bar.addPermanentWidget(self.lbl_connection)

        self.btn_check_deps = QPushButton("🔍 检测依赖")
        self.btn_check_deps.setToolTip("检查各引擎后端所需的 Python 库是否已安装")
        self.btn_check_deps.clicked.connect(self._on_check_dependencies)
        self.btn_check_deps.setStyleSheet("padding: 2px 8px; font-size: 11px;")
        self.status_bar.addPermanentWidget(self.btn_check_deps)

        # ── 信号连接 ──
        self.modify_panel.game_connected.connect(self._on_game_connected)
        self.modify_panel.game_dir_detected.connect(self._on_game_dir_detected)

        # 自动连接定时器
        self._auto_connect_timer = QTimer()
        self._auto_connect_timer.setSingleShot(True)
        self._auto_connect_timer.timeout.connect(self._try_auto_connect)

    # ── 事件处理 ──────────────────────────────────────────

    def _on_game_connected(self, bridge: IGameBridge | None):
        """游戏连接成功后更新状态栏和面板"""
        self.modify_panel.set_game_ws(bridge)
        self._update_status_bar()
        if self.modify_panel.scan_result:
            self.modify_panel._on_refresh_live()

    def _on_game_dir_detected(self, game_dir: str):
        """检测到游戏目录（从存档路径反推）"""
        if not game_dir or not os.path.isdir(game_dir):
            return

        try:
            info = detect_game(game_dir=game_dir)
            if not info:
                return
        except Exception as e:
            print(f"游戏检测失败: {e}")
            return

        try:
            profile = self.profile_mgr.set_current_game(game_dir)
            self.current_profile = profile
        except Exception as e:
            print(f"加载存档配置失败: {e}")

        game_name = info.game_title or os.path.basename(game_dir)
        self.lbl_game.setText("🎮 " + game_name)
        self.lbl_game.setStyleSheet("color: #2a7; padding: 0 8px;")

        # 检测引擎类型
        try:
            engine_info = detect_engine_from_dir(game_dir)
            self.detected_engine = engine_info.engine_type
            hint = get_engine_connect_hint(engine_info.engine_type)
            self.lbl_game.setToolTip(
                f"引擎: {engine_info.engine_name}\n{game_dir}\n推荐: {hint}"
            )
        except Exception:
            self.detected_engine = "unknown"

        try:
            installed = is_plugin_installed(game_dir)
        except Exception:
            installed = False
        self.lbl_plugin.setText(get_plugin_status_text(game_dir))
        self.lbl_plugin.setStyleSheet(
            "color: #2a7; padding: 0 8px;" if installed
            else "color: #c44; padding: 0 8px;"
        )
        if profile:
            profile.plugin_installed = installed
            self.profile_mgr.save(profile)

        # 插件已安装时自动尝试连接
        if installed:
            self._auto_connect_timer.start(1500)

    def _try_auto_connect(self):
        """自动尝试连接游戏"""
        if self.connection.is_connected:
            return
        ok, method = self.connection.connect()
        if ok:
            self._on_game_connected(self.connection.bridge)
            if self.modify_panel.rpg_data:
                self.modify_panel._on_scan()
        self._update_status_bar()

    def _update_status_bar(self):
        """更新状态栏连接状态"""
        if self.connection.is_connected:
            m = self.connection.connection_type.upper()
            self.lbl_connection.setText(f"● 已连接 ({m})")
            self.lbl_connection.setStyleSheet(
                "color: #2a7; font-weight: bold; padding: 0 8px;"
            )
        else:
            self.lbl_connection.setText("● 未连接")
            self.lbl_connection.setStyleSheet(
                "color: gray; font-weight: bold; padding: 0 8px;"
            )

    def _on_check_dependencies(self):
        """弹出依赖检测对话框"""
        reports = check_all_dependencies()
        summary = get_summary(reports)
        missing = get_missing_deps(reports)

        lines = [f"📊 引擎依赖状态 — {summary}\n"]
        lines.append("━" * 45)

        for r in reports:
            icon = "✅" if r.ready else "❌"
            lines.append(f"\n{icon} {r.engine_name} ({r.backend_class})")
            lines.append(f"   提示: {r.connect_hint}")
            for d in r.deps:
                status = "✓" if d.installed else "✗ 缺失"
                ver = f" {d.version}" if d.version else ""
                lines.append(f"   {status} {d.name}{ver}")

        if missing:
            lines.append(f"\n{'━' * 45}")
            lines.append("📦 安装缺失依赖:")
            for d in missing:
                lines.append(f"  {d.install_cmd}")

        QMessageBox.information(self, "引擎依赖检测", "\n".join(lines))

    def closeEvent(self, event):
        self.connection.disconnect()
        self.modify_panel.set_game_ws(None)
        event.accept()

    def inject_plugin_and_restart(self):
        """一键注入插件并提示重启"""
        game_dir = self.modify_panel.current_game_dir
        if not game_dir:
            QMessageBox.warning(self, "错误", "请先加载存档以检测游戏目录")
            return False
        if is_plugin_installed(game_dir):
            QMessageBox.information(self, "提示", "插件已安装，无需重复注入")
            return True
        reply = QMessageBox.question(
            self, "注入插件",
            "将在游戏中安装 TCP 桥接插件。\n安装后需要重启游戏才能生效。\n\n继续吗？",
            QMessageBox.Yes | QMessageBox.No,
        )
        if reply != QMessageBox.Yes:
            return False
        try:
            ok = inject_plugin(game_dir)
            if ok:
                self.lbl_plugin.setText(get_plugin_status_text(game_dir))
                self.lbl_plugin.setStyleSheet("color: #2a7; padding: 0 8px;")
                if self.current_profile:
                    self.current_profile.plugin_installed = True
                    self.profile_mgr.save(self.current_profile)
                QMessageBox.information(self, "成功",
                                        "插件已注入！\n请重启游戏后点击「连接游戏」。")
                return True
            else:
                QMessageBox.warning(self, "失败", "插件注入失败")
        except Exception as e:
            QMessageBox.warning(self, "失败", str(e))
        return False
