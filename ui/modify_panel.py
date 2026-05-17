"""项目修改面板 — 存档加载、编辑、保存一体化（多格式支持）

通过 ISaveFormat 抽象接口操作存档，支持多游戏引擎。
"""
import ctypes
import os
from ctypes import wintypes

from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, QLineEdit,
    QPushButton, QTableWidget, QTableWidgetItem,
    QHeaderView, QAbstractItemView, QComboBox, QInputDialog,
    QMessageBox, QFileDialog,
)
from PySide6.QtCore import Qt, Signal, QTimer

from core.save_format import ISaveFormat, ModifiableField as SaveField
from core.game_data_scanner import (
    scan_all_modifiable, GameScanResult, ModifiableField,
)
from core.game_bridge import IGameBridge, GameState
from core.rpgmv_save import get_party_info  # 仅用于「写入游戏」功能
from core.game_config import scan_game_directory
from core.format_registry import FormatRegistry
from core.engine_profile import get_engine_registry
from formats.rpgmaker import RpgMakerFormat
from formats.renpy import RenPyFormat
from formats.unreal_gvas import UnrealGVASFormat
from formats.generic_json import GenericJsonFormat


# ── 类别名称映射 ──────────────────────────────────────

CATEGORY_LABELS = {
    "gold": "金币",
    "switch": "开关",
    "variable": "变量",
    "actor": "角色属性",
    "item": "物品",
    "weapon": "武器",
    "armor": "防具",
    "self_switch": "自开关",
}


class ModifyPanel(QWidget):
    """存档编辑 + 项目修改 — 一体化面板（多格式支持）"""

    # 信号：通知主窗口游戏已连接
    game_connected = Signal(object)
    # 信号：通知主窗口检测到游戏目录
    game_dir_detected = Signal(str)

    def __init__(self):
        super().__init__()
        self.scan_result: GameScanResult | None = None
        self.rpg_data: dict | None = None
        self.current_path: str = ""
        self.game_config: dict | None = None
        self.bridge: IGameBridge | None = None
        self.current_game_dir: str = ""
        self.format_handler: ISaveFormat | None = None
        self.all_fields: list[ModifiableField] = []
        self.filtered_fields: list[ModifiableField] = []
        self._gold_var_id: int = 0
        self._auto_refresh_timer = QTimer()
        self._auto_refresh_timer.timeout.connect(self._auto_refresh_tick)

        # 注册已知格式（后续可改为自动发现）
        FormatRegistry.register(RpgMakerFormat)
        FormatRegistry.register(RenPyFormat)
        FormatRegistry.register(UnrealGVASFormat)
        FormatRegistry.register(GenericJsonFormat)

        # 初始化引擎关联注册中心（链接格式↔桥接↔引擎检测）
        self.engine_registry = get_engine_registry()

        self._init_ui()

    # ═══════════════════════════════════════════════════════
    # UI 构造
    # ═══════════════════════════════════════════════════════

    def _init_ui(self):
        layout = QVBoxLayout(self)

        # ── 第1行：文件路径 ──
        file_row = QHBoxLayout()
        file_row.addWidget(QLabel("存档:"))
        self.edit_file_path = QLineEdit()
        self.edit_file_path.setPlaceholderText("选择或输入存档文件路径（支持多格式）...")
        file_row.addWidget(self.edit_file_path, 1)

        self.btn_browse = QPushButton("浏览...")
        self.btn_browse.clicked.connect(self._browse_file)
        file_row.addWidget(self.btn_browse)

        self.btn_load = QPushButton("加载存档")
        self.btn_load.clicked.connect(self._load_rpg_save)
        self.btn_load.setStyleSheet("font-weight: bold;")
        file_row.addWidget(self.btn_load)

        self.btn_save = QPushButton("保存存档")
        self.btn_save.clicked.connect(self._save_rpg_save)
        self.btn_save.setEnabled(False)
        self.btn_save.setStyleSheet("font-weight: bold; color: #060;")
        file_row.addWidget(self.btn_save)

        self.lbl_file_info = QLabel("")
        self.lbl_file_info.setStyleSheet("color: #888; font-size: 11px;")
        file_row.addWidget(self.lbl_file_info)

        layout.addLayout(file_row)

        # ── 第2行：游戏连接 ──
        conn_row = QHBoxLayout()

        self.btn_scan_proc = QPushButton("扫描进程")
        self.btn_scan_proc.setToolTip("自动检测运行中的 Game.exe 并加载存档")
        self.btn_scan_proc.clicked.connect(self._on_scan_processes)
        conn_row.addWidget(self.btn_scan_proc)

        self.btn_connect_game = QPushButton("连接游戏")
        self.btn_connect_game.setToolTip("自动连接运行中的游戏（TCP 桥接或 CDP）")
        self.btn_connect_game.clicked.connect(self._on_connect_game)
        conn_row.addWidget(self.btn_connect_game)

        self.btn_inject_plugin = QPushButton("注入插件")
        self.btn_inject_plugin.setToolTip("安装 TCP 桥接插件到游戏（需重启游戏生效）")
        self.btn_inject_plugin.clicked.connect(self._on_inject_plugin)
        conn_row.addWidget(self.btn_inject_plugin)

        self.btn_write_game = QPushButton("写入游戏")
        self.btn_write_game.setToolTip("将当前存档中的数值实时写入游戏内存")
        self.btn_write_game.clicked.connect(self._on_write_to_game)
        self.btn_write_game.setEnabled(False)
        conn_row.addWidget(self.btn_write_game)

        self.lbl_game_status = QLabel("💡 加载存档后点击「连接游戏」获取实时数据")
        self.lbl_game_status.setStyleSheet("color: #666; font-size: 11px;")
        conn_row.addWidget(self.lbl_game_status, 1)

        layout.addLayout(conn_row)

        # ── 第3行：项目扫描工具栏 ──
        scan_row = QHBoxLayout()

        self.btn_scan = QPushButton("🔍 扫描游戏项目")
        self.btn_scan.setToolTip("扫描游戏数据文件和存档，列出所有可修改的项目")
        self.btn_scan.clicked.connect(self._on_scan)
        scan_row.addWidget(self.btn_scan)

        self.combo_category = QComboBox()
        self.combo_category.addItem("全部类别", "")
        for cat_key, cat_label in CATEGORY_LABELS.items():
            self.combo_category.addItem(cat_label, cat_key)
        self.combo_category.currentIndexChanged.connect(self._on_filter_changed)
        scan_row.addWidget(QLabel("类别:"))
        scan_row.addWidget(self.combo_category)

        self.edit_search = QLineEdit()
        self.edit_search.setPlaceholderText("搜索项目名称或 ID...")
        self.edit_search.textChanged.connect(self._on_filter_changed)
        scan_row.addWidget(self.edit_search, 1)

        self.lbl_count = QLabel("共 0 项")
        scan_row.addWidget(self.lbl_count)

        layout.addLayout(scan_row)

        # ── 第4行：存档摘要 ──
        self.lbl_summary = QLabel("")
        self.lbl_summary.setStyleSheet("color: #2a7; padding: 2px 8px; font-size: 12px; font-weight: bold;")
        layout.addWidget(self.lbl_summary)

        # ── 主体表格 ──
        self.table = QTableWidget()
        self.table.setColumnCount(7)
        self.table.setHorizontalHeaderLabels([
            "ID", "类别", "名称", "💾 存档值", "🎮 实时值", "改存档", "改实时"
        ])
        self.table.horizontalHeader().setSectionResizeMode(2, QHeaderView.Stretch)
        self.table.setEditTriggers(QAbstractItemView.NoEditTriggers)
        self.table.setSelectionBehavior(QAbstractItemView.SelectRows)
        self.table.setAlternatingRowColors(True)
        self.table.cellDoubleClicked.connect(self._on_cell_double_clicked)
        layout.addWidget(self.table, 1)

        # ── 底部操作行 ──
        bottom_row = QHBoxLayout()

        self.btn_save_all = QPushButton("💾 保存所有修改")
        self.btn_save_all.setToolTip("将所有「改存档」标记的修改写入存档文件")
        self.btn_save_all.clicked.connect(self._on_save_all)
        self.btn_save_all.setEnabled(False)
        bottom_row.addWidget(self.btn_save_all)

        self.btn_push_all = QPushButton("🎮 推送所有实时修改")
        self.btn_push_all.setToolTip("将所有「改实时」修改立即写入游戏内存")
        self.btn_push_all.clicked.connect(self._on_push_all)
        self.btn_push_all.setEnabled(False)
        bottom_row.addWidget(self.btn_push_all)

        self.btn_refresh_live = QPushButton("🔄 刷新实时数据")
        self.btn_refresh_live.setToolTip("重新从游戏读取开关/变量实时值")
        self.btn_refresh_live.clicked.connect(self._on_refresh_live)
        self.btn_refresh_live.setEnabled(False)
        bottom_row.addWidget(self.btn_refresh_live)

        bottom_row.addStretch(1)
        self.lbl_action_status = QLabel("")
        self.lbl_action_status.setStyleSheet("color: #888; font-size: 11px;")
        bottom_row.addWidget(self.lbl_action_status)
        layout.addLayout(bottom_row)

    # ═══════════════════════════════════════════════════════
    # 文件操作（原 save_panel）
    # ═══════════════════════════════════════════════════════

    def _browse_file(self):
        path, _ = QFileDialog.getOpenFileName(
            self, "选择游戏存档文件", "",
            FormatRegistry.get_file_filter())
        if path:
            self.edit_file_path.setText(path)
            self._load_rpg_save()

    def _load_rpg_save(self):
        """加载存档文件（自动检测格式）"""
        path = self.edit_file_path.text().strip()
        if not path:
            QMessageBox.warning(self, "错误", "请先选择存档文件路径")
            return
        try:
            # 自动检测格式
            self.format_handler = FormatRegistry.detect(path)
            self.rpg_data = self.format_handler.load(path)
        except ValueError as e:
            QMessageBox.warning(self, "错误", f"无法识别存档格式:\n{e}")
            return
        except Exception as e:
            QMessageBox.warning(self, "错误", f"加载存档失败: {e}")
            self.rpg_data = None
            return

        self.current_path = path
        self._detect_game_dir()

        # 更新 UI
        self._update_summary()
        self.btn_save.setEnabled(True)
        self.btn_save_all.setEnabled(True)
        self.lbl_file_info.setText(f"{os.path.basename(path)} [{self.format_handler.name}]")

        # 自动扫描
        self._on_scan()

        # 更新实时连接提示
        self._update_connection_hint()

    def _save_rpg_save(self):
        """保存存档文件（先应用所有脏字段再写入）"""
        path = self.edit_file_path.text().strip()
        if not path or self.rpg_data is None:
            QMessageBox.warning(self, "错误", "没有已加载的存档数据")
            return

        if not self.format_handler:
            QMessageBox.warning(self, "错误", "未检测到存档格式")
            return

        # 先应用所有修改面板中的脏字段
        self._apply_dirty_fields()

        try:
            self.format_handler.save(path, self.rpg_data)
            QMessageBox.information(self, "成功", "存档已保存")
            self._update_summary()
        except Exception as e:
            QMessageBox.warning(self, "错误", f"保存失败: {e}")

    def _detect_game_dir(self):
        """从存档路径推导游戏目录并加载配置"""
        save_path = self.edit_file_path.text().strip()
        if not save_path:
            return
        save_dir = os.path.dirname(save_path)   # www/Save/
        www_dir = os.path.dirname(save_dir)      # www/
        game_dir = os.path.dirname(www_dir)      # 游戏根目录
        self.current_game_dir = game_dir

        if game_dir and os.path.isdir(game_dir):
            self.game_dir_detected.emit(game_dir)

        try:
            self.game_config = scan_game_directory(game_dir)
            if self.game_config and self.game_config.get("data_loaded"):
                print(f"Game config loaded: {self.game_config['game_title']}")
        except Exception:
            self.game_config = None

    def _update_summary(self):
        """更新存档摘要标签"""
        if self.rpg_data is None:
            self.lbl_summary.setText("")
            return
        if self.format_handler:
            summary = self.format_handler.get_summary(self.rpg_data)
        else:
            # 回退：直接使用 rpgmv_save
            from core.rpgmv_save import get_save_summary
            summary_data = get_save_summary(self.rpg_data)
            from core.save_format import SaveSummary
            summary = SaveSummary(
                gold=summary_data.get("gold", 0),
                party_size=summary_data.get("party_size", 0),
                item_count=summary_data.get("item_count", 0),
                save_count=summary_data.get("save_count", 0),
                play_time=summary_data.get("play_time", 0),
            )
        self.lbl_summary.setText(
            f"📊 金币: {summary.gold} | "
            f"队伍: {summary.party_size}人 | "
            f"物品: {summary.item_count}种 | "
            f"存档次数: {summary.save_count} | "
            f"游戏时间: {summary.play_time}"
        )

    # ═══════════════════════════════════════════════════════
    # 游戏连接（原 save_panel）
    # ═══════════════════════════════════════════════════════

    def _on_connect_game(self):
        """连接运行中的游戏"""
        main_win = self.window()
        if not hasattr(main_win, "connection"):
            QMessageBox.warning(self, "错误", "无法获取连接管理器")
            return

        conn = main_win.connection
        ok, method = conn.connect()
        if ok:
            self.bridge = conn.bridge
            state = conn.get_state()
            if state and state.raw:
                self._update_live_comparison(state.raw)
            self.game_connected.emit(self.bridge)
            self.btn_write_game.setEnabled(True)
            self.btn_push_all.setEnabled(True)
            self.btn_refresh_live.setEnabled(True)
            QMessageBox.information(self, "连接成功",
                                    f"✓ 已通过 {method} 连接游戏")
        else:
            QMessageBox.information(self, "提示",
                "未检测到游戏连接。\n"
                "请确保:\n"
                "1. 游戏已启动并加载存档\n"
                "2. TCP 桥接插件已安装（点击「注入插件」）\n"
                "3. 或使用 CDP 调试端口 (需要 SDK 构建)")

    def _on_inject_plugin(self):
        """注入 TCP 桥接插件"""
        main_win = self.window()
        if hasattr(main_win, "inject_plugin_and_restart"):
            main_win.inject_plugin_and_restart()
        else:
            QMessageBox.information(self, "提示",
                                    "请先加载存档以确定游戏目录")

    def _on_write_to_game(self):
        """将当前队伍数据实时写入游戏内存"""
        if not self.bridge or not self.bridge.is_connected:
            QMessageBox.warning(self, "错误", "请先连接游戏")
            return
        if not self.rpg_data:
            QMessageBox.warning(self, "错误", "请先加载存档")
            return

        try:
            count = 0
            party_info = get_party_info(self.rpg_data)
            for member in party_info:
                aid = member["id"]
                if self.bridge.set_actor_hp(aid, member["hp"]):
                    count += 1
                if self.bridge.set_actor_mp(aid, member["mp"]):
                    count += 1
            QMessageBox.information(self, "写入完成",
                                    f"已向游戏写入 {count} 项数值")
        except Exception as e:
            QMessageBox.warning(self, "写入失败", str(e))

    def _update_live_comparison(self, live_state):
        """更新游戏实时数据对比"""
        if self.rpg_data and live_state:
            party_size = live_state.get("partySize", 0) if live_state else 0
            map_name = live_state.get("mapName", "?") if live_state else "?"
            self.lbl_game_status.setText(
                f"🟢 已连接 | 队伍: {party_size}人 | 地图: {map_name}"
            )
            self.lbl_game_status.setStyleSheet("color: #2a7; font-size: 11px;")
        self.btn_write_game.setEnabled(True)

    def _update_connection_hint(self):
        """根据加载的存档格式提示可用的实时连接方式"""
        if not self.format_handler:
            self.lbl_game_status.setText("💡 加载存档后点击「连接游戏」获取实时数据")
            self.lbl_game_status.setStyleSheet("color: #666; font-size: 11px;")
            return

        bridges = self.format_handler.compatible_bridges
        engine_type = self.format_handler.engine_type

        if not bridges:
            self.lbl_game_status.setText(
                f"💡 [{self.format_handler.name}] 暂无实时连接支持 (引擎: {engine_type})"
            )
            self.lbl_game_status.setStyleSheet("color: #888; font-size: 11px;")
            return

        # 获取引擎关联档案中的详细连接提示
        from core.engine_detect import ENGINE_CONNECT_HINTS
        hint = ENGINE_CONNECT_HINTS.get(engine_type, "")

        bridge_names = ", ".join(bridges)
        self.lbl_game_status.setText(
            f"💡 [{self.format_handler.name}] 可连接: {bridge_names}"
            + (f" | {hint}" if hint else "")
        )
        self.lbl_game_status.setStyleSheet("color: #2a7; font-size: 11px;")

    # ═══════════════════════════════════════════════════════
    # 进程扫描（原 save_panel）
    # ═══════════════════════════════════════════════════════

    def _on_scan_processes(self):
        """扫描按钮回调：检测运行中的游戏进程并加载存档"""
        players = self._scan_running_games()
        if not players:
            QMessageBox.information(self, "未找到", "未检测到运行中的游戏进程")
            return

        for proc in players:
            save_path = self._find_save_from_process(proc)
            if save_path:
                self.edit_file_path.setText(save_path)
                self._load_rpg_save()
                return

        msg = "检测到游戏进程，但未找到存档文件:\n" + "\n".join(
            f"{p['name']} (PID: {p['pid']})" for p in players[:5])
        QMessageBox.information(self, "已找到进程", msg)

    def _scan_running_games(self):
        """使用 Windows API 自动检测运行中的 Game.exe 进程"""
        class PROCESSENTRY32(ctypes.Structure):
            _fields_ = [
                ("dwSize", wintypes.DWORD),
                ("cntUsage", wintypes.DWORD),
                ("th32ProcessID", wintypes.DWORD),
                ("th32DefaultHeapID", ctypes.POINTER(ctypes.c_ulong)),
                ("th32ModuleID", wintypes.DWORD),
                ("cntThreads", wintypes.DWORD),
                ("th32ParentProcessID", wintypes.DWORD),
                ("pcPriClassBase", ctypes.c_long),
                ("dwFlags", wintypes.DWORD),
                ("szExeFile", ctypes.c_char * 260),
            ]

        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        CreateToolhelp32Snapshot = kernel32.CreateToolhelp32Snapshot
        CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
        CreateToolhelp32Snapshot.restype = wintypes.HANDLE

        Process32First = kernel32.Process32First
        Process32First.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32)]
        Process32First.restype = wintypes.BOOL

        Process32Next = kernel32.Process32Next
        Process32Next.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32)]
        Process32Next.restype = wintypes.BOOL

        CloseHandle = kernel32.CloseHandle
        CloseHandle.argtypes = [wintypes.HANDLE]
        CloseHandle.restype = wintypes.BOOL

        OpenProcess = kernel32.OpenProcess
        OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        OpenProcess.restype = wintypes.HANDLE

        QueryFullProcessImageNameW = kernel32.QueryFullProcessImageNameW
        QueryFullProcessImageNameW.argtypes = [
            wintypes.HANDLE, wintypes.DWORD,
            ctypes.POINTER(ctypes.c_wchar), ctypes.POINTER(wintypes.DWORD),
        ]
        QueryFullProcessImageNameW.restype = wintypes.BOOL

        snapshot = CreateToolhelp32Snapshot(0x00000002, 0)
        if not snapshot or snapshot == wintypes.HANDLE(-1).value:
            return []

        pe = PROCESSENTRY32()
        pe.dwSize = ctypes.sizeof(PROCESSENTRY32)

        results = []
        if Process32First(snapshot, ctypes.byref(pe)):
            while True:
                try:
                    exe_name = pe.szExeFile.decode("gbk", errors="replace").lower()
                except Exception:
                    exe_name = pe.szExeFile.decode("utf-8", errors="replace").lower()

                if exe_name == "game.exe":
                    pid = pe.th32ProcessID
                    exe_path = exe_name
                    hProcess = OpenProcess(0x0400 | 0x0010, False, pid)
                    if hProcess:
                        try:
                            buf = ctypes.create_unicode_buffer(260)
                            buf_size = wintypes.DWORD(260)
                            if QueryFullProcessImageNameW(hProcess, 0, buf, ctypes.byref(buf_size)):
                                exe_path = buf.value
                        except Exception:
                            pass
                        finally:
                            CloseHandle(hProcess)

                    results.append({
                        "name": exe_name,
                        "pid": pid,
                        "exe_path": exe_path,
                    })

                if not Process32Next(snapshot, ctypes.byref(pe)):
                    break

        CloseHandle(snapshot)
        return results

    def _find_save_from_process(self, proc_info):
        """从游戏进程信息推测存档路径（使用已注册的格式扩展名）"""
        exe_path = proc_info.get("exe_path", "")
        if not exe_path:
            return ""
        game_dir = os.path.dirname(exe_path)
        extensions = FormatRegistry.get_supported_extensions()
        for subdir in ["www/Save", "Save", "save", "saves"]:
            save_dir = os.path.join(game_dir, subdir)
            if os.path.isdir(save_dir):
                try:
                    files = sorted(os.listdir(save_dir))
                    for f in files:
                        if f.startswith("file") and any(f.endswith(ext) for ext in extensions):
                            return os.path.join(save_dir, f)
                    for f in files:
                        if any(f.endswith(ext) for ext in extensions) and not f.startswith(("config", "global")):
                            return os.path.join(save_dir, f)
                except OSError:
                    continue
        return ""

    # ═══════════════════════════════════════════════════════
    # 游戏桥接器设置（供主窗口调用）
    # ═══════════════════════════════════════════════════════

    def set_game_ws(self, bridge: IGameBridge | None):
        """设置游戏连接桥接器"""
        self.bridge = bridge
        has_conn = bridge is not None and bridge.is_connected
        self.btn_refresh_live.setEnabled(has_conn)
        self.btn_push_all.setEnabled(has_conn)
        self.btn_write_game.setEnabled(has_conn)
        if has_conn:
            self._auto_refresh_timer.start(10000)
        else:
            self._auto_refresh_timer.stop()

    def _auto_refresh_tick(self):
        """定时自动刷新实时数据（静默）"""
        if not self.bridge or not self.bridge.is_connected:
            self._auto_refresh_timer.stop()
            return
        if not self.scan_result:
            return
        try:
            state = self.bridge.get_state()
            if state:
                for field in self.all_fields:
                    if field.category == "switch":
                        field.live_value = state.switches.get(field.item_id)
                    elif field.category == "variable":
                        field.live_value = state.variables.get(field.item_id)
                self._refresh_table()
        except Exception:
            pass

    # ═══════════════════════════════════════════════════════
    # 项目扫描
    # ═══════════════════════════════════════════════════════

    def _on_scan(self):
        """扫描游戏项目"""
        game_dir = self.current_game_dir
        if not game_dir or not os.path.isdir(game_dir):
            QMessageBox.warning(self, "错误", "请先加载存档以确定游戏目录")
            return

        # 尝试读取实时游戏状态
        live_state = None
        try:
            if self.bridge:
                state = self.bridge.get_state()
                if state:
                    live_state = {
                        "switches": state.switches,
                        "variables": state.variables,
                        "selfSwitches": state.self_switches,
                    }
        except Exception:
            pass

        try:
            self.scan_result = scan_all_modifiable(
                game_dir=game_dir,
                save_data=self.rpg_data,
                live_state=live_state,
            )
        except Exception as e:
            QMessageBox.warning(self, "扫描失败", str(e))
            return

        self.all_fields = list(self.scan_result.fields)
        self.filtered_fields = list(self.all_fields)
        gold_f = next((f for f in self.all_fields if f.category == "gold"), None)
        self._gold_var_id = gold_f.gold_var_id if gold_f else 0
        self._refresh_table()

        self.lbl_count.setText(
            f"共 {len(self.all_fields)} 项 "
            f"(存档: {'✓' if self.scan_result.has_save_data else '✗'} "
            f"实时: {'✓' if self.scan_result.has_live_data else '✗'})"
        )
        self.btn_save_all.setEnabled(self.rpg_data is not None)
        self.lbl_action_status.setText(
            "存档:{} 实时:{} | 双击💾列改存档值，双击🎮列改实时值".format(
                "✓" if self.scan_result.has_save_data else "✗",
                "✓" if self.scan_result.has_live_data else "✗"
            )
        )

    def _on_refresh_live(self):
        """刷新实时游戏数据"""
        if not self.bridge or not self.bridge.is_connected:
            QMessageBox.warning(self, "未连接", "请先连接游戏")
            return

        try:
            state = self.bridge.get_state()
            if not state:
                QMessageBox.warning(self, "刷新失败", "无法读取游戏状态")
                return
        except Exception as e:
            QMessageBox.warning(self, "刷新失败", str(e))
            return

        if not self.scan_result:
            self._on_scan()
            return

        updated = 0
        for field in self.all_fields:
            if field.category == "switch":
                new_val = state.switches.get(field.item_id)
                if field.live_value != new_val:
                    field.live_value = new_val
                    updated += 1
            elif field.category == "variable":
                new_val = state.variables.get(field.item_id)
                if field.live_value != new_val:
                    field.live_value = new_val
                    updated += 1

        self._refresh_table()
        self.lbl_count.setText(
            f"共 {len(self.all_fields)} 项 | 实时数据已刷新 ({updated} 项变化)"
        )

    # ═══════════════════════════════════════════════════════
    # 表格刷新
    # ═══════════════════════════════════════════════════════

    def _refresh_table(self):
        """刷新表格内容"""
        fields = self.filtered_fields
        self.table.setRowCount(len(fields))

        for row, field in enumerate(fields):
            # ID
            id_item = QTableWidgetItem(str(field.item_id) if field.item_id else "-")
            id_item.setData(Qt.UserRole, field)
            self.table.setItem(row, 0, id_item)

            # 类别
            cat_label = CATEGORY_LABELS.get(field.category, field.category)
            self.table.setItem(row, 1, QTableWidgetItem(cat_label))

            # 名称
            name_item = QTableWidgetItem(field.display_name)
            name_item.setToolTip(field.description if field.description else field.display_name)
            self.table.setItem(row, 2, name_item)

            # 存档值
            sv = field.save_value
            sv_text = self._format_value(sv, field.field_type)
            sv_item = QTableWidgetItem(sv_text)
            if field.dirty:
                sv_item.setForeground(Qt.darkRed)
                font = sv_item.font()
                font.setBold(True)
                sv_item.setFont(font)
                sv_item.setToolTip("已修改，待保存")
            elif sv is not None:
                sv_item.setForeground(Qt.darkBlue)
            else:
                sv_item.setForeground(Qt.gray)
            self.table.setItem(row, 3, sv_item)

            # 实时值
            lv = field.live_value
            lv_text = self._format_value(lv, field.field_type)
            lv_item = QTableWidgetItem(lv_text)
            if lv is not None:
                lv_item.setForeground(Qt.darkGreen)
            else:
                lv_item.setForeground(Qt.gray)
            self.table.setItem(row, 4, lv_item)

            # 改存档按钮
            btn_save = QPushButton("编辑")
            btn_save.setToolTip("修改「存档值」（需点击底部「保存所有修改」写入文件）")
            btn_save.clicked.connect(lambda checked=False, r=row: self._edit_field(r, "save"))
            self.table.setCellWidget(row, 5, btn_save)

            # 改实时按钮
            btn_live = QPushButton("编辑")
            btn_live.setToolTip("修改「实时值」（会立即写入游戏内存）")
            btn_live.setStyleSheet("color: #060;")
            btn_live.clicked.connect(lambda checked=False, r=row: self._edit_field(r, "live"))
            self.table.setCellWidget(row, 6, btn_live)

        self.lbl_count.setText(f"显示 {len(fields)} / 共 {len(self.all_fields)} 项")

    def _format_value(self, val, field_type: str) -> str:
        if val is None:
            return "-"
        if field_type == "bool":
            return "ON" if val else "OFF"
        return str(val)

    # ═══════════════════════════════════════════════════════
    # 过滤
    # ═══════════════════════════════════════════════════════

    def _on_filter_changed(self):
        if not self.all_fields:
            return
        category = self.combo_category.currentData()
        query = self.edit_search.text().strip()

        fields = self.all_fields
        if category:
            fields = [f for f in fields if f.category == category]
        if query:
            q = query.lower()
            fields = [f for f in fields if q in f.display_name.lower() or
                      str(f.item_id) == q]

        self.filtered_fields = fields
        self._refresh_table()

    # ═══════════════════════════════════════════════════════
    # 编辑
    # ═══════════════════════════════════════════════════════

    def _edit_field(self, row: int, target: str = "save"):
        """编辑指定字段"""
        item = self.table.item(row, 0)
        if not item:
            return
        field = item.data(Qt.UserRole)
        if not isinstance(field, ModifiableField):
            return

        if target == "live":
            current_val = field.live_value if field.live_value is not None else field.save_value
            value_attr = "live_value"
            label_prefix = "实时值"
        else:
            current_val = field.save_value if field.save_value is not None else field.default_value
            value_attr = "save_value"
            label_prefix = "存档值"

        if current_val is None:
            current_val = 0 if field.field_type == "int" else False

        if field.field_type == "bool":
            new_val = not bool(current_val)
            setattr(field, value_attr, new_val)
            field.dirty = True
            self._refresh_table()
            if target == "live":
                self._push_single_field(field)
            return

        label = f"{field.display_name} - {field.description or ''}\n当前{label_prefix}: {current_val}\n当前实时值: {self._format_value(field.live_value, field.field_type)}"
        new_val, ok = QInputDialog.getInt(
            self, f"修改{label_prefix}", label,
            int(current_val) if current_val is not None else 0,
            field.min_val, field.max_val,
        )
        if ok:
            setattr(field, value_attr, new_val)
            field.dirty = True
            self._refresh_table()
            if target == "live":
                self._push_single_field(field)

    def _push_single_field(self, field: ModifiableField):
        if not self.bridge or not self.bridge.is_connected:
            return
        try:
            self._apply_field_to_game(field)
            self.lbl_action_status.setText(f"✓ 已推送: {field.display_name}")
        except Exception as e:
            self.lbl_action_status.setText(f"✗ 推送失败: {field.display_name}")

    def _on_cell_double_clicked(self, row: int, col: int):
        if col == 3:
            self._edit_field(row, "save")
        elif col == 4:
            self._edit_field(row, "live")

    # ═══════════════════════════════════════════════════════
    # 批量保存/推送
    # ═══════════════════════════════════════════════════════

    def _apply_dirty_fields(self):
        """将所有脏字段写入 rpg_data（内部使用）"""
        if not self.rpg_data:
            return
        for field in self.all_fields:
            if not field.dirty or field.save_value is None:
                continue
            try:
                self._apply_field_to_save(field)
            except Exception as e:
                print(f"[ERROR] 应用字段失败 {field.field_id}: {e}")

    def _on_save_all(self):
        """保存所有修改：先应用脏字段到 rpg_data，再写入文件"""
        if not self.rpg_data or not self.all_fields:
            QMessageBox.warning(self, "错误", "请先加载存档并扫描项目")
            return

        if not self.format_handler:
            QMessageBox.warning(self, "错误", "未检测到存档格式")
            return

        count = 0
        failed = 0
        for field in self.all_fields:
            if not field.dirty or field.save_value is None:
                continue
            try:
                self._apply_field_to_save(field)
                count += 1
            except Exception as e:
                failed += 1
                print(f"[ERROR] 保存字段失败 {field.field_id}: {e}")

        if count == 0 and failed == 0:
            QMessageBox.information(self, "提示", "没有需要保存的修改")
            return

        if failed > 0:
            self.lbl_action_status.setText(
                f"⚠ 已应用 {count} 项, {failed} 项失败 (详见控制台)"
            )

        # 清除脏标记
        for field in self.all_fields:
            field.dirty = False

        # 通过格式处理器写入文件
        try:
            self.format_handler.save(self.current_path, self.rpg_data)
            self._refresh_table()
            self._update_summary()
            if failed == 0:
                self.lbl_action_status.setText(f"✓ 已保存 {count} 项修改到存档")
            QMessageBox.information(self, "成功", f"已保存 {count} 项修改到存档文件")
        except Exception as e:
            QMessageBox.warning(self, "错误", f"写入存档文件失败: {e}")

    def _on_push_all(self):
        if not self.bridge or not self.bridge.is_connected:
            QMessageBox.warning(self, "错误", "请先连接游戏")
            return
        if not self.all_fields:
            return

        count = 0
        failed = 0
        for field in self.all_fields:
            if field.live_value is None:
                continue
            try:
                self._apply_field_to_game(field)
                count += 1
            except Exception as e:
                failed += 1
                print(f"[ERROR] 推送字段失败 {field.field_id}: {e}")

        if failed > 0:
            self.lbl_action_status.setText(
                f"⚠ 已推送 {count} 项, {failed} 项失败 (详见控制台)"
            )
        else:
            self.lbl_action_status.setText(f"✓ 已推送 {count} 项到游戏")

    def _apply_field_to_save(self, field: ModifiableField):
        """将单个字段的存档值写入 rpg_data（委托给格式处理器）"""
        if self.format_handler:
            # 转换为 save_format 的 ModifiableField（兼容格式处理器接口）
            sf = SaveField(
                category=field.category,
                field_id=field.field_id,
                display_name=field.display_name,
                item_id=field.item_id,
                field_type=field.field_type,
                save_value=field.save_value,
            )
            self.format_handler.apply_field(self.rpg_data, sf)

        # 金币变量双向同步（RPG Maker 特有逻辑，保留在此层）
        if field.category == "gold" and self._gold_var_id > 0:
            from core.rpgmv_save import set_variable
            set_variable(self.rpg_data, self._gold_var_id, int(field.save_value))
        elif field.category == "variable" and self._gold_var_id > 0 and field.item_id == self._gold_var_id:
            from core.rpgmv_save import set_gold
            set_gold(self.rpg_data, int(field.save_value))

    def _apply_field_to_game(self, field: ModifiableField):
        """将单个字段的实时值写入游戏内存"""
        if not self.bridge:
            return
        val = field.live_value if field.live_value is not None else field.save_value
        if val is None:
            return

        if field.category == "gold":
            self.bridge.set_gold(int(val))
            if field.gold_var_id > 0:
                self.bridge.set_variable(field.gold_var_id, int(val))
        elif field.category == "switch":
            self.bridge.set_switch(field.item_id, bool(val))
        elif field.category == "variable":
            self.bridge.set_variable(field.item_id, int(val))
            if self._gold_var_id > 0 and field.item_id == self._gold_var_id:
                self.bridge.set_gold(int(val))
        elif field.category == "actor":
            if field.field_id.endswith("_hp"):
                self.bridge.set_actor_hp(field.item_id, int(val))
            elif field.field_id.endswith("_mp"):
                self.bridge.set_actor_mp(field.item_id, int(val))
