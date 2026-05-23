//! GUI 主状态机与布局管理器。
//!
//! 本模块是 GUI 的核心，实现了：
//! - **AppState**：应用程序全局状态，包含所有面板数据与 UI 状态
//! - **eframe::App 实现**：每帧的 update() 渲染循环，驱动整个 GUI
//! - **状态转换方法**：加载存档、保存、切换游戏、实时连接等核心操作
//!
//! 架构说明：
//! - update() 每帧首先调用 drain_rt_results() 处理桥接线程的异步结果
//! - 然后依次渲染：顶栏 → 标签栏 → 中央面板（按 active_tab 分发） → 状态栏
//! - 对话框（未保存确认、操作确认）在中央面板之后叠加渲染

use game_tool_core::config::load_config;
use game_tool_core::detector::{detect_by_filesystem, EngineType};
use game_tool_core::{BridgeCommand, MemoryCommand};
use serde_json::Value;

use crate::connection;
use crate::discovery;
use crate::factory::{self, create_format};

fn value_type_id(vt: &game_tool_memory::ValueType) -> u32 {
    match vt {
        game_tool_memory::ValueType::I32 => 0,
        game_tool_memory::ValueType::I64 => 1,
        game_tool_memory::ValueType::F32 => 2,
        game_tool_memory::ValueType::F64 => 3,
        game_tool_memory::ValueType::String(_) => 4,
        game_tool_memory::ValueType::Bytes(_) => 5,
    }
}
use crate::panels::{
    backup, realtime_editor, save_editor, settings, startup, status_bar, tab_bar, toolbox, top_bar,
};
use crate::state::{
    AppState, BridgeJob, BridgeMode, BridgeResult, ConfirmAction, ConfirmDialog, ConnectionStatus,
    RtPanelState, SavePanelState, TabMode, ToolboxAction, ToolboxState,
};

impl AppState {
    /// 检测游戏目录的引擎类型
    fn detect_engine(game_dir: &Option<String>) -> EngineType {
        game_dir
            .as_ref()
            .map(|d| detect_by_filesystem(d))
            .unwrap_or(EngineType::Unknown)
    }

    /// 加载游戏配置（开关/变量名称映射、游戏标题等）
    fn load_game_config(
        game_dir: &Option<String>,
        engine: &EngineType,
    ) -> (Option<game_tool_rpgmaker::scanner::GameConfig>, String) {
        let (config, title) = if let Some(ref dir) = game_dir {
            if *engine != EngineType::Unknown {
                let gc = game_tool_rpgmaker::scanner::scan_game_directory(dir);
                let title = if gc.data_loaded {
                    gc.game_title.clone()
                } else {
                    String::new()
                };
                (if gc.data_loaded { Some(gc) } else { None }, title)
            } else {
                (None, String::new())
            }
        } else {
            (None, String::new())
        };
        (config, title)
    }

    /// 初始化存档编辑面板
    fn init_save_panel(game_dir: &Option<String>, engine: &EngineType) -> SavePanelState {
        let panel_mode = factory::engine_to_panel_mode(engine);
        let format = create_format(engine);
        let save_files = if let (Some(ref dir), Some(ref fmt)) = (game_dir, &format) {
            discovery::find_save_files(dir, &**fmt)
        } else {
            Vec::new()
        };
        SavePanelState {
            format,
            save_files,
            panel_mode,
            readonly: false,
            selected_save: None,
            save_data: None,
            summary: None,
            fields: Vec::new(),
            dirty_count: 0,
            selected_category: None,
            search_query: String::new(),
            jump_id: String::new(),
        }
    }

    /// 初始化实时编辑面板
    fn init_rt_panel(
        game_dir: &Option<String>,
        engine: &EngineType,
        config: &game_tool_core::config::AppConfig,
    ) -> RtPanelState {
        let port = config.tcp_port;
        let plugin_installed = if factory::supports_realtime(engine) {
            if let Some(ref dir) = game_dir {
                match engine {
                    EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
                        game_tool_rpgmaker::tcp::is_plugin_installed(dir)
                    }
                    EngineType::RenPy => game_tool_renpy::bridge::is_plugin_installed(dir),
                    _ => false,
                }
            } else {
                false
            }
        } else {
            false
        };
        let bridge_mode = if matches!(
            engine,
            EngineType::Unreal
                | EngineType::UnityMono
                | EngineType::UnityIl2Cpp
                | EngineType::Godot
        ) {
            BridgeMode::Memory
        } else {
            BridgeMode::Tcp
        };
        RtPanelState {
            conn: None,
            fields: Vec::new(),
            plugin_installed,
            host: "127.0.0.1".into(),
            port,
            error_message: String::new(),
            error_expires_at: None,
            write_feedback: String::new(),
            write_feedback_expires_at: None,
            search_query: String::new(),
            selected_category: None,
            jump_id: String::new(),
            auto_refresh: true,
            locked_fields: std::collections::HashSet::new(),
            refresh_interval_secs: 3,
            last_refresh: None,
            bridge_mode,
            process_list: Vec::new(),
            selected_process: None,
            scan_value: String::new(),
            scan_value_type: game_tool_memory::ValueType::I32,
            scan_results: Vec::new(),
            scan_count: 0,
            next_scan_mode: 0,
            scan_in_progress: false,
            field_seeds: Vec::new(),
            save_fields_snapshot: Vec::new(),
        }
    }

    /// 创建初始应用状态
    pub fn new(game_dir: Option<String>) -> Self {
        let config = load_config().unwrap_or_default();
        let engine = Self::detect_engine(&game_dir);
        let (game_config, game_title) = Self::load_game_config(&game_dir, &engine);
        let dark_mode = config.dark_mode;
        let save_panel = Self::init_save_panel(&game_dir, &engine);
        let rt_panel = Self::init_rt_panel(&game_dir, &engine, &config);

        Self {
            game_dir,
            game_title,
            engine,
            game_config,
            active_tab: TabMode::SaveEditor,
            dark_mode,
            recent_games: config.recent_games.clone(),
            backup_paths: Vec::new(),
            backup_selection: std::collections::HashSet::new(),
            save_panel,
            rt_panel,
            toolbox: ToolboxState {
                lz_input: String::new(),
                lz_output: String::new(),
                lz_error: String::new(),
                b64_input: String::new(),
                b64_output: String::new(),
                info_path: String::new(),
                info_result: None,
                check_path: String::new(),
                check_result: None,
                batch_dir: String::new(),
                batch_results: Vec::new(),
                repair_path: String::new(),
                repair_result: None,
            },
            status_message: String::new(),
            show_unsaved_dialog: false,
            show_confirm_dialog: None,
        }
    }

    /// 加载当前选中的存档文件。
    ///
    /// 流程：读取文件 → 解析为 JSON → 提取摘要 → 扫描可编辑字段 → 重置 dirty 计数。
    /// 加载失败时在状态栏显示错误信息。
    fn load_save_file(&mut self) {
        let path = match &self.save_panel.selected_save {
            Some(p) => p.clone(),
            None => return,
        };
        let format = match &self.save_panel.format {
            Some(ref f) => f,
            None => return,
        };

        match format.load(&path) {
            Ok(data) => {
                let summary = format.get_summary(&data);
                let game_dir = self.game_dir.as_deref().unwrap_or("");
                let fields = format.scan_fields(&data, game_dir);
                self.save_panel.summary = Some(summary);
                self.save_panel.fields = fields;
                self.save_panel.save_data = Some(data);
                self.save_panel.dirty_count = 0;
                // 重置搜索和筛选状态
                self.save_panel.search_query.clear();
                self.save_panel.selected_category = None;
                self.save_panel.jump_id.clear();
            }
            Err(e) => {
                self.status_message = format!("加载存档失败: {}", e);
            }
        }
    }

    /// 将当前修改写回存档文件。
    ///
    /// 返回 true 表示保存成功，false 表示失败（会在状态栏显示错误）。
    ///
    /// 保存流程：
    /// 1. 收集所有 dirty=true 的字段
    /// 2. 逐个调用 format.apply_field() 将修改写入 JSON
    /// 3. 调用 format.save() 写回文件
    /// 4. 清除所有 dirty 标记
    fn save_current(&mut self) -> bool {
        let path = match &self.save_panel.selected_save {
            Some(p) => p.clone(),
            None => {
                self.status_message = "未选择存档文件".into();
                return false;
            }
        };
        let save_data = match &mut self.save_panel.save_data {
            Some(d) => d,
            None => {
                self.status_message = "存档数据为空".into();
                return false;
            }
        };
        let format = match &self.save_panel.format {
            Some(ref f) => f,
            None => return false,
        };

        // 收集所有已修改的字段
        let dirty: Vec<_> = self
            .save_panel
            .fields
            .iter()
            .filter(|f| f.dirty)
            .cloned()
            .collect();

        // 逐个应用修改到 JSON 数据
        for field in &dirty {
            if let Err(e) = format.apply_field(save_data, field) {
                self.status_message = format!("写入字段 {} 失败: {}", field.display_name, e);
                return false;
            }
        }

        // 写回文件
        match format.save(&path, save_data) {
            Ok(()) => {
                for f in &mut self.save_panel.fields {
                    f.dirty = false;
                }
                self.save_panel.dirty_count = 0;
                self.status_message = "存档已保存".into();
                true
            }
            Err(e) => {
                self.status_message = format!("保存失败: {}", e);
                false
            }
        }
    }

    /// 重新扫描游戏目录中的存档文件，刷新存档列表。
    fn refresh_save_files(&mut self) {
        if let (Some(ref dir), Some(ref fmt)) = (&self.game_dir, &self.save_panel.format) {
            self.save_panel.save_files = discovery::find_save_files(dir, &**fmt);
        }
    }

    /// 打开文件夹选择对话框，切换到新的游戏目录。
    ///
    /// 此操作会：
    /// - 重置所有面板状态（存档编辑、实时修改、备份）
    /// - 重新检测引擎类型
    /// - 创建新的格式处理器
    /// - 添加到最近游戏列表
    fn switch_game(&mut self) {
        if let Some(new_dir) = rfd::FileDialog::new()
            .set_title("选择游戏目录")
            .pick_folder()
        {
            let dir_str = new_dir.to_string_lossy().to_string();
            self.game_dir = Some(dir_str.clone());
            self.engine = detect_by_filesystem(&dir_str);

            // 重新扫描游戏配置
            self.game_config = if self.engine != EngineType::Unknown {
                let gc = game_tool_rpgmaker::scanner::scan_game_directory(&dir_str);
                if gc.data_loaded {
                    self.game_title = gc.game_title.clone();
                    Some(gc)
                } else {
                    self.game_title.clear();
                    None
                }
            } else {
                self.game_title.clear();
                None
            };

            // 重置存档面板状态
            self.save_panel.format = create_format(&self.engine);
            self.save_panel.panel_mode = factory::engine_to_panel_mode(&self.engine);
            self.save_panel.readonly = false;
            self.save_panel.selected_save = None;
            self.save_panel.save_data = None;
            self.save_panel.summary = None;
            self.save_panel.fields.clear();
            self.save_panel.dirty_count = 0;
            self.save_panel.selected_category = None;
            self.save_panel.search_query.clear();
            self.rt_panel.plugin_installed = false;

            // 断开现有实时连接
            if let Some(ref conn) = self.rt_panel.conn {
                let _ = conn.cmd_tx.send(BridgeJob::Disconnect);
            }
            self.rt_panel.conn = None;
            self.rt_panel.fields.clear();
            self.rt_panel.error_message.clear();
            self.rt_panel.error_expires_at = None;
            self.rt_panel.write_feedback.clear();
            self.rt_panel.write_feedback_expires_at = None;
            self.rt_panel.search_query.clear();
            self.rt_panel.selected_category = None;
            self.rt_panel.jump_id.clear();
            self.rt_panel.auto_refresh = true;
            self.rt_panel.locked_fields.clear();
            self.rt_panel.last_refresh = None;
            self.rt_panel.bridge_mode = if matches!(self.engine, EngineType::Unreal | EngineType::UnityMono | EngineType::UnityIl2Cpp | EngineType::Godot) { BridgeMode::Memory } else { BridgeMode::Tcp };
            self.rt_panel.process_list.clear();
            self.rt_panel.selected_process = None;
            self.rt_panel.scan_value.clear();
            self.rt_panel.scan_results.clear();
            self.rt_panel.scan_count = 0;
            self.rt_panel.field_seeds.clear();
            self.rt_panel.save_fields_snapshot.clear();

            self.save_panel.jump_id.clear();
            self.backup_paths.clear();
            self.backup_selection.clear();
            self.status_message.clear();

            self.refresh_save_files();

            // 重新检查插件安装状态
            if factory::supports_realtime(&self.engine) {
                match self.engine {
                    EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
                        self.rt_panel.plugin_installed =
                            game_tool_rpgmaker::tcp::is_plugin_installed(&dir_str);
                    }
                    EngineType::RenPy => {
                        self.rt_panel.plugin_installed =
                            game_tool_renpy::bridge::is_plugin_installed(&dir_str);
                    }
                    _ => {}
                }
            }

            // 更新最近游戏列表（去重，限制 5 个，新游戏排最前）
            if let Some(ref dir) = self.game_dir {
                let dir = dir.clone();
                self.recent_games.retain(|g| g != &dir);
                self.recent_games.insert(0, dir);
                self.recent_games.truncate(5);
                if let Ok(mut cfg) = load_config() {
                    cfg.recent_games = self.recent_games.clone();
                    let _ = game_tool_core::config::save_config(&cfg);
                }
            }
        }
    }

    /// 向游戏目录注入实时编辑插件（仅 TCP 模式）
    ///
    /// 根据引擎类型调用对应的注入逻辑：
    /// - RPG Maker：复制 TCP 桥接 JS 文件到游戏 js/plugins 目录
    /// - Ren'Py：注入 Python 桥接代码
    /// 注入成功后设置 plugin_installed = true。
    fn inject_plugin(&mut self) {
        let dir = match &self.game_dir {
            Some(d) => d.clone(),
            None => {
                self.status_message = "未选择游戏目录".into();
                return;
            }
        };
        let result = match self.engine {
            EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
                game_tool_rpgmaker::tcp::inject_plugin(&dir, self.rt_panel.port).map_err(|e| e)
            }
            EngineType::RenPy => game_tool_renpy::bridge::inject_plugin(&dir).map_err(|e| e),
            _ => Err("不支持".into()),
        };
        match result {
            Ok(()) => {
                self.rt_panel.plugin_installed = true;
            }
            Err(e) => {
                self.status_message = format!("注入失败: {}", e);
            }
        }
    }

    /// 发起实时连接到游戏进程。
    ///
    /// 创建一个桥接线程，设置状态为 Connecting，发送 Connect 指令。
    /// 连接成功/失败的结果由 drain_rt_results() 在下一次 update() 中处理。
    fn rt_connect(&mut self) {
        self.rt_panel.error_message.clear();
        let port = self.rt_panel.port;
        let host = self.rt_panel.host.clone();
        let engine = self.engine.clone();

        let mut conn = connection::spawn_bridge_thread(engine, host.clone(), port);
        conn.status = ConnectionStatus::Connecting;
        let _ = conn.cmd_tx.send(BridgeJob::Connect);
        self.rt_panel.conn = Some(conn);
    }

    /// 断开实时连接。
    ///
    /// 向桥接线程发送 Disconnect 指令，线程会主动断开并清理资源。
    fn rt_disconnect(&mut self) {
        if let Some(ref conn) = self.rt_panel.conn {
            let _ = conn.cmd_tx.send(BridgeJob::Disconnect);
        }
    }

    /// 附加游戏进程（内存桥模式）
    fn rt_attach_process(&mut self, pid: u32) {
        self.rt_panel.error_message.clear();
        let engine = self.engine.clone();
        let mut conn = connection::spawn_bridge_thread(engine, "127.0.0.1".into(), 0);
        conn.status = ConnectionStatus::Connecting;
        let _ = conn.cmd_tx.send(BridgeJob::MemoryCommand(MemoryCommand::Attach(pid)));
        self.rt_panel.conn = Some(conn);
    }

    /// 分离游戏进程（内存桥模式）
    fn rt_detach_process(&mut self) {
        if let Some(ref conn) = self.rt_panel.conn {
            let _ = conn.cmd_tx.send(BridgeJob::MemoryCommand(MemoryCommand::Detach));
        }
    }

    /// 枚举当前系统中的运行进程（用于内存桥模式）
    fn rt_list_processes(&mut self) {
        self.rt_panel.process_list = game_tool_memory::process::enumerate_processes();
    }

    /// 发送内存扫描命令
    fn rt_scan(&mut self, first: bool) {
        if let Some(ref conn) = self.rt_panel.conn {
            self.rt_panel.scan_in_progress = true;
            if first {
                let val: serde_json::Value = self.rt_panel.scan_value.parse::<i64>()
                    .map(|v| serde_json::Value::Number(v.into()))
                    .or_else(|_| self.rt_panel.scan_value.parse::<f64>().ok()
                        .and_then(|v| serde_json::Number::from_f64(v).map(serde_json::Value::Number))
                        .ok_or(()))
                    .unwrap_or(serde_json::Value::String(self.rt_panel.scan_value.clone()));
                let vt_id = value_type_id(&self.rt_panel.scan_value_type);
                let _ = conn.cmd_tx.send(BridgeJob::MemoryCommand(
                    MemoryCommand::FirstScan { value: val, value_type_id: vt_id }
                ));
            } else {
                let mode_id = self.rt_panel.next_scan_mode;
                let val = if mode_id == 0 {
                    self.rt_panel.scan_value.parse::<i64>()
                        .map(|v| Some(serde_json::Value::Number(v.into())))
                        .unwrap_or_else(|_| self.rt_panel.scan_value.parse::<f64>().ok()
                            .and_then(|v| serde_json::Number::from_f64(v).map(serde_json::Value::Number)))
                } else { None };
                let _ = conn.cmd_tx.send(BridgeJob::MemoryCommand(
                    MemoryCommand::NextScan { scan_mode_id: mode_id, value: val }
                ));
            }
        }
    }

    /// 从存档填充扫描种子
    fn rt_seed_from_save(&mut self) {
        if let Some(ref conn) = self.rt_panel.conn {
            let fields = self.save_panel.fields.clone();
            let _ = conn.cmd_tx.send(BridgeJob::MemoryCommand(
                MemoryCommand::SeedFromSave(fields)
            ));
        }
    }

    /// 处理实时连接的结果队列（每帧调用）。
    ///
    /// 主要职责：
    /// 1. **自动刷新**：如果启用了 auto_refresh 且到达刷新间隔，发送 ReadAll 命令
    /// 2. **消息处理**：遍历 result_rx 中的 BridgeResult 并更新 UI 状态
    /// 3. **定时清除**：自动清除过期的错误消息和写入反馈
    fn drain_rt_results(&mut self) {
        // === 自动刷新机制 ===
        // 如果连接已建立且开启了自动刷新，在到达刷新间隔时发送 ReadAll 命令
        if self.rt_panel.auto_refresh
            && self
                .rt_panel
                .conn
                .as_ref()
                .map(|c| c.status == ConnectionStatus::Connected)
                .unwrap_or(false)
        {
            let interval = std::time::Duration::from_secs(self.rt_panel.refresh_interval_secs);
            let should_refresh = match self.rt_panel.last_refresh {
                Some(last) => last.elapsed() >= interval,
                None => true,  // 首次自动触发
            };
            if should_refresh {
                self.rt_panel.last_refresh = Some(std::time::Instant::now());
                if let Some(ref conn) = self.rt_panel.conn {
                    let _ = conn.cmd_tx.send(BridgeJob::Execute(BridgeCommand::ReadAll));
                }
            }
        }

        // === 处理桥接线程返回的结果 ===
        if let Some(ref mut conn) = self.rt_panel.conn {
            let results = connection::drain_results(conn);
            for result in results {
                match result {
                    BridgeResult::Connected => {
                        // 连接成功：更新状态，清除错误，触发首次数据读取
                        conn.status = ConnectionStatus::Connected;
                        self.rt_panel.error_message.clear();
                        self.rt_panel.error_expires_at = None;
                        self.rt_panel.last_refresh = Some(std::time::Instant::now());
                        let _ = conn.cmd_tx.send(BridgeJob::Execute(BridgeCommand::ReadAll));
                    }
                    BridgeResult::Attached => {
                        // 内存桥：进程已附加
                        conn.status = ConnectionStatus::Connected;
                        self.rt_panel.error_message.clear();
                        self.rt_panel.error_expires_at = None;
                        self.rt_panel.last_refresh = Some(std::time::Instant::now());
                        self.rt_panel.scan_in_progress = false;
                        self.rt_panel.scan_results.clear();
                        self.rt_panel.scan_count = 0;
                        let _ = conn.cmd_tx.send(BridgeJob::Execute(BridgeCommand::ReadAll));
                    }
                    BridgeResult::Disconnected => {
                        // 连接断开：清空字段列表和错误信息
                        conn.status = ConnectionStatus::Disconnected;
                        self.rt_panel.fields.clear();
                        self.rt_panel.error_message.clear();
                        self.rt_panel.error_expires_at = None;
                        self.rt_panel.last_refresh = None;
                    }
                    BridgeResult::CommandResult(val) => {
                        // 命令执行结果：通常是 ReadAll 返回的 GameState JSON
                        if let Ok(gs) =
                            serde_json::from_value::<game_tool_core::GameState>(val.clone())
                        {
                            let mut new_fields = factory::game_state_to_fields(
                                &gs,
                                &self.engine,
                                self.game_config.as_ref(),
                            );
                            // 恢复被锁定字段的值（自动刷新时保留用户手动修改的值）
                            let locked: Vec<(String, Value)> = self
                                .rt_panel
                                .fields
                                .iter()
                                .filter(|of| self.rt_panel.locked_fields.contains(&of.field_id))
                                .map(|of| (of.field_id.clone(), of.live_value.clone()))
                                .collect();
                            for nf in &mut new_fields {
                                if let Some((_, lv)) =
                                    locked.iter().find(|(id, _)| *id == nf.field_id)
                                {
                                    nf.live_value = lv.clone();
                                }
                            }
                            self.rt_panel.fields = new_fields;
                        }
                        if val.as_str() == Some("ok") {
                            self.rt_panel.write_feedback = "✓ 已写入".into();
                            self.rt_panel.write_feedback_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                        }
                    }
                    BridgeResult::ScanResult(val) => {
                        // 内存扫描结果
                        if let Ok(addrs) = serde_json::from_value::<Vec<game_tool_core::ScannedAddr>>(val) {
                            self.rt_panel.scan_results = addrs;
                            self.rt_panel.scan_count = self.rt_panel.scan_results.len();
                        }
                        self.rt_panel.scan_in_progress = false;
                    }
                    BridgeResult::SeedResult(val) => {
                        // 存档种子扫描结果
                        if let Ok(seeds) = serde_json::from_value::<Vec<game_tool_core::FieldScanSeed>>(val) {
                            self.rt_panel.field_seeds = seeds;
                            self.rt_panel.scan_count = self.rt_panel.field_seeds.iter()
                                .map(|s| s.candidates.len()).sum();
                        }
                        self.rt_panel.scan_in_progress = false;
                    }
                    BridgeResult::Error(e) => {
                        // 错误处理：致命错误（连接失败/断开）会导致状态变为 Disconnected
                        let is_fatal = e.contains("连接失败")
                            || e.contains("未连接")
                            || e.contains("connection refused")
                            || e.contains("closed");
                        if is_fatal {
                            conn.status = ConnectionStatus::Disconnected;
                            self.rt_panel.fields.clear();
                        }
                        self.rt_panel.error_message = e;
                        // 错误消息 5 秒后自动消失
                        self.rt_panel.error_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                    }
                }
            }
        }

        // === 自动清除定时消息 ===
        // 错误消息和写入反馈都会在超时后自动消失，避免界面残留旧信息
        if let Some(at) = self.rt_panel.error_expires_at {
            if at <= std::time::Instant::now() {
                self.rt_panel.error_message.clear();
                self.rt_panel.error_expires_at = None;
            }
        }
        if let Some(at) = self.rt_panel.write_feedback_expires_at {
            if at <= std::time::Instant::now() {
                self.rt_panel.write_feedback.clear();
                self.rt_panel.write_feedback_expires_at = None;
            }
        }
    }

    /// 向桥接线程发送实时命令（便捷方法）
    fn rt_send_command(&self, cmd: BridgeCommand) {
        if let Some(ref conn) = self.rt_panel.conn {
            let _ = conn.cmd_tx.send(BridgeJob::Execute(cmd));
        }
    }

    /// 创建当前所选存档的备份。
    ///
    /// 使用 game_tool_core::backup 模块，保留最近 5 个备份版本。
    fn create_backup(&mut self) {
        let path = match &self.save_panel.selected_save {
            Some(p) => p.clone(),
            None => {
                self.status_message = "未选择存档".into();
                return;
            }
        };
        match game_tool_core::backup::save_backup(std::path::Path::new(&path), 5) {
            Ok(backup_path) => {
                self.backup_paths
                    .push(backup_path.to_string_lossy().to_string());
                self.status_message = "备份已创建".into();
            }
            Err(e) => {
                self.status_message =
                    format!("创建备份失败: {}", e);
            }
        }
    }

    /// 恢复指定索引的备份到当前存档（覆盖）。
    ///
    /// 通过文件复制实现，恢复成功后重新加载存档以刷新显示。
    fn restore_backup(&mut self, index: usize) {
        if index >= self.backup_paths.len() {
            return;
        }
        let backup_path = self.backup_paths[index].clone();
        let target = match &self.save_panel.selected_save {
            Some(p) => p.clone(),
            None => {
                self.status_message = "未选择存档".into();
                return;
            }
        };
        if let Err(e) = std::fs::copy(&backup_path, &target) {
            self.status_message = format!("恢复失败: {}", e);
        } else {
            self.status_message =
                "备份已恢复到当前存档"
                    .into();
            self.load_save_file();  // 恢复后重新加载以刷新显示
        }
    }

    /// 删除指定索引的备份文件。
    ///
    /// 先移除路径记录，再删除文件。如果删除失败则恢复路径记录。
    fn delete_backup(&mut self, index: usize) {
        if index >= self.backup_paths.len() {
            return;
        }
        let path = self.backup_paths.remove(index);
        match std::fs::remove_file(&path) {
            Ok(()) => {
                self.status_message = "备份已删除".into();
            }
            Err(e) => {
                // 删除失败时恢复路径到原位置
                self.backup_paths.insert(index, path);
                self.status_message = format!("删除失败: {}", e);
            }
        }
    }
}

impl eframe::App for AppState {
    /// 每帧渲染回调：egui 事件循环的核心。
    ///
    /// 渲染顺序：
    /// 1. drain_rt_results() — 处理异步桥接结果
    /// 2. 应用主题样式
    /// 3. 顶栏 (top_bar)
    /// 4. 标签栏 (tab_bar) — 仅在已加载游戏时显示
    /// 5. 中央面板 — 根据 active_tab 分发到对应面板
    /// 6. 对话框遮罩层 — 未保存确认 / 确认对话框
    /// 7. 状态栏 (status_bar) — 底部
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 每帧首先处理桥接线程的异步结果
        self.drain_rt_results();

        // 应用当前主题（暗色/亮色）
        crate::theme::Theme::new(self.dark_mode).apply(ctx);

        let has_game = self.game_dir.is_some();

        // ===================================================================
        // 顶栏：游戏标题、引擎类型、当前目录路径
        // ===================================================================
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            top_bar::render(ui, has_game, &self.game_title, &self.engine, &self.game_dir);
        });

        if has_game {
            let supports_rt = factory::supports_realtime(&self.engine);

            // ===================================================================
            // 标签栏：存档编辑 | 实时修改 | 备份管理 | 工具箱 | 设置
            // 以及切换游戏按钮
            // ===================================================================
            egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
                let actions = tab_bar::render(ui, self.active_tab, has_game, supports_rt);
                for action in actions {
                    match action {
                        tab_bar::TabAction::SwitchTab(tab) => {
                            self.active_tab = tab;
                        }
                        tab_bar::TabAction::SwitchGame => {
                            // 如果有未保存的修改，先弹出确认对话框
                            if self.save_panel.dirty_count > 0 {
                                self.show_unsaved_dialog = true;
                            } else {
                                self.switch_game();
                            }
                        }
                    }
                }
            });

            // ===================================================================
            // 中央面板：根据 active_tab 渲染对应内容
            // ===================================================================
            egui::CentralPanel::default().show(ctx, |ui| {
                match self.active_tab {
                    // ----- 存档编辑面板 -----
                    TabMode::SaveEditor => {
                            let actions = save_editor::render(ui, self);
                            for action in actions {
                                match action {
                                    save_editor::SaveAction::LoadSave => self.load_save_file(),
                                    save_editor::SaveAction::RefreshFiles => self.refresh_save_files(),
                                    save_editor::SaveAction::Save => {
                                        self.save_current();
                                    }
                                    save_editor::SaveAction::UndoDirty => {
                                        // 撤销所有修改：直接重新加载存档
                                        self.load_save_file();
                                    }
                                }
                            }
                    }
                    // ----- 实时修改面板 -----
                    TabMode::RealtimeEditor => {
                        if !crate::factory::supports_realtime(&self.engine) {
                            ui.colored_label(
                                crate::theme::colors::TEXT_SECONDARY,
                                "当前引擎不支持实时修改",
                            );
                        } else if self.rt_panel.bridge_mode == BridgeMode::Memory {
                            // ════════════════════════════════════════
                            // 内存模式 UI
                            // ════════════════════════════════════════
                            let is_attached = self.rt_panel.conn.as_ref()
                                .map(|c| c.status == ConnectionStatus::Connected)
                                .unwrap_or(false);
                            let is_connecting = self.rt_panel.conn.as_ref()
                                .map(|c| c.status == ConnectionStatus::Connecting)
                                .unwrap_or(false);

                            // 进程选择行
                            ui.horizontal(|ui| {
                                if !is_attached && !is_connecting {
                                    if ui.button("↻ 刷新进程").clicked() {
                                        self.rt_list_processes();
                                    }
                                    egui::ComboBox::from_id_salt("process_selector")
                                        .selected_text(
                                            self.rt_panel.selected_process
                                                .clone()
                                                .unwrap_or_else(|| "选择进程...".into())
                                        )
                                        .show_ui(ui, |ui| {
                                            for proc in &self.rt_panel.process_list {
                                                let label = format!("{} (PID: {})", proc.name, proc.pid);
                                                if ui.selectable_label(
                                                    self.rt_panel.selected_process.as_deref() == Some(&proc.name),
                                                    &label,
                                                ).clicked() {
                                                    self.rt_panel.selected_process = Some(proc.name.clone());
                                                }
                                            }
                                        });
                                    if let Some(ref name) = self.rt_panel.selected_process.clone() {
                                        if let Some(proc) = self.rt_panel.process_list.iter().find(|p| p.name == *name) {
                                            if ui.button("● 附加").clicked() {
                                                self.rt_attach_process(proc.pid);
                                            }
                                        }
                                    }
                                } else if is_connecting {
                                    ui.colored_label(crate::theme::colors::WARNING, "附加中...");
                                } else if is_attached {
                                    ui.colored_label(crate::theme::colors::SUCCESS, "✓ 已附加");
                                    if ui.button("◎ 分离").clicked() {
                                        self.rt_detach_process();
                                    }
                                }
                            });

                            // 扫描器（仅已附加时显示）
                            if is_attached {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label("搜索:");
                                    ui.add(egui::TextEdit::singleline(&mut self.rt_panel.scan_value)
                                        .desired_width(100.0)
                                        .hint_text("输入值..."));
                                    egui::ComboBox::from_id_salt("scan_type")
                                        .selected_text(match self.rt_panel.scan_value_type {
                                            game_tool_memory::ValueType::I32 => "I32",
                                            game_tool_memory::ValueType::I64 => "I64",
                                            game_tool_memory::ValueType::F32 => "F32",
                                            game_tool_memory::ValueType::F64 => "F64",
                                            game_tool_memory::ValueType::String(_) => "Str",
                                            _ => "I32",
                                        })
                                        .show_ui(ui, |ui| {
                                            if ui.selectable_label(self.rt_panel.scan_value_type == game_tool_memory::ValueType::I32, "I32").clicked() {
                                                self.rt_panel.scan_value_type = game_tool_memory::ValueType::I32;
                                            }
                                            if ui.selectable_label(self.rt_panel.scan_value_type == game_tool_memory::ValueType::I64, "I64").clicked() {
                                                self.rt_panel.scan_value_type = game_tool_memory::ValueType::I64;
                                            }
                                            if ui.selectable_label(self.rt_panel.scan_value_type == game_tool_memory::ValueType::F32, "F32").clicked() {
                                                self.rt_panel.scan_value_type = game_tool_memory::ValueType::F32;
                                            }
                                            if ui.selectable_label(self.rt_panel.scan_value_type == game_tool_memory::ValueType::F64, "F64").clicked() {
                                                self.rt_panel.scan_value_type = game_tool_memory::ValueType::F64;
                                            }
                                        });
                                    if ui.button("首次扫描").clicked() {
                                        self.rt_scan(true);
                                    }
                                    ui.label(format!("候选: {}", self.rt_panel.scan_count));
                                });

                                // 二次扫描模式 + 存档辅助
                                ui.horizontal(|ui| {
                                    egui::ComboBox::from_id_salt("next_scan_mode")
                                        .selected_text(match self.rt_panel.next_scan_mode {
                                            0 => "精确值",
                                            1 => "增大",
                                            2 => "减小",
                                            3 => "未变",
                                            4 => "已变",
                                            _ => "",
                                        })
                                        .show_ui(ui, |ui| {
                                            if ui.selectable_label(self.rt_panel.next_scan_mode == 0, "精确值").clicked() { self.rt_panel.next_scan_mode = 0; }
                                            if ui.selectable_label(self.rt_panel.next_scan_mode == 1, "增大").clicked() { self.rt_panel.next_scan_mode = 1; }
                                            if ui.selectable_label(self.rt_panel.next_scan_mode == 2, "减小").clicked() { self.rt_panel.next_scan_mode = 2; }
                                            if ui.selectable_label(self.rt_panel.next_scan_mode == 3, "未变").clicked() { self.rt_panel.next_scan_mode = 3; }
                                            if ui.selectable_label(self.rt_panel.next_scan_mode == 4, "已变").clicked() { self.rt_panel.next_scan_mode = 4; }
                                        });
                                    if ui.button("再次扫描").clicked() {
                                        self.rt_scan(false);
                                    }
                                    if !self.save_panel.fields.is_empty() {
                                        if ui.button("从存档加载种子").clicked() {
                                            self.rt_seed_from_save();
                                        }
                                    }
                                });

                                // 扫描候选结果摘要
                                if !self.rt_panel.scan_results.is_empty() {
                                    ui.label(format!("候选地址: {} 个", self.rt_panel.scan_count));
                                }
                                if !self.rt_panel.field_seeds.is_empty() {
                                    let confirmed_count = self.rt_panel.field_seeds.iter()
                                        .filter(|s| s.confidence > 0.8).count();
                                    ui.label(format!("存档种子: {} 个(确认: {} 个)",
                                        self.rt_panel.field_seeds.len(), confirmed_count));
                                }
                            }

                            ui.separator();

                            // 刷新控制行（通用部分）
                            ui.horizontal(|ui| {
                                let auto = self.rt_panel.auto_refresh;
                                if ui.selectable_label(auto, if auto { "▶ 自动刷新" } else { "⏸ 暂停刷新" }).clicked() {
                                    self.rt_panel.auto_refresh = !auto;
                                }
                                if ui.button("📥 手动刷新").clicked() {
                                    self.rt_send_command(BridgeCommand::ReadAll);
                                }
                                ui.label("间隔:");
                                egui::ComboBox::from_id_salt("refresh_interval")
                                    .selected_text(format!("{}秒", self.rt_panel.refresh_interval_secs))
                                    .show_ui(ui, |ui| {
                                        for secs in &[1u64, 2, 3, 5] {
                                            if ui.selectable_label(self.rt_panel.refresh_interval_secs == *secs, format!("{}秒", secs)).clicked() {
                                                self.rt_panel.refresh_interval_secs = *secs;
                                                self.rt_panel.last_refresh = None;
                                            }
                                        }
                                    });
                            });

                            // 错误消息
                            if !self.rt_panel.error_message.is_empty() {
                                ui.colored_label(crate::theme::colors::ERROR, &self.rt_panel.error_message);
                            }
                            if !self.rt_panel.write_feedback.is_empty() {
                                ui.colored_label(crate::theme::colors::SUCCESS, &self.rt_panel.write_feedback);
                            }

                            ui.separator();

                            // 实时编辑器字段表
                            let actions = realtime_editor::render(ui, &mut self.rt_panel, &self.save_panel.fields);
                            for action in actions {
                                match action {
                                    realtime_editor::RtAction::WriteField(id, val) => {
                                        self.rt_send_command(BridgeCommand::WriteField(id, val));
                                    }
                                    realtime_editor::RtAction::ToggleLock(fid) => {
                                        if self.rt_panel.locked_fields.contains(&fid) {
                                            self.rt_panel.locked_fields.remove(&fid);
                                        } else {
                                            self.rt_panel.locked_fields.insert(fid);
                                        }
                                    }
                                    realtime_editor::RtAction::CopyToSave(fid) => {
                                        if let Some(rt_field) = self.rt_panel.fields.iter().find(|f| f.field_id == fid) {
                                            if let Some(save_field) = self.save_panel.fields.iter_mut().find(|f| f.field_id == fid) {
                                                save_field.save_value = rt_field.live_value.clone();
                                                save_field.dirty = true;
                                            }
                                        }
                                        self.save_panel.dirty_count = self.save_panel.fields.iter().filter(|f| f.dirty).count();
                                    }
                                }
                            }
                        } else {
                            // ════════════════════════════════════════
                            // TCP 模式 UI（原有逻辑）
                            // ════════════════════════════════════════
                            // 连接配置行：主机地址、端口、连接/断开按钮、插件注入按钮
                            ui.horizontal(|ui| {
                                ui.label("主机:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.rt_panel.host)
                                        .desired_width(100.0),
                                );
                                ui.label("端口:");
                                ui.add(
                                    egui::DragValue::new(&mut self.rt_panel.port)
                                        .range(1024..=65535),
                                );

                                let is_connected = self
                                    .rt_panel
                                    .conn
                                    .as_ref()
                                    .map(|c| c.status == ConnectionStatus::Connected)
                                    .unwrap_or(false);
                                let is_connecting = self
                                    .rt_panel
                                    .conn
                                    .as_ref()
                                    .map(|c| c.status == ConnectionStatus::Connecting)
                                    .unwrap_or(false);

                                if is_connecting {
                                    ui.colored_label(crate::theme::colors::WARNING, "连接中...");
                                } else if is_connected {
                                    if ui.button("◎ 断开").clicked() {
                                        self.rt_disconnect();
                                    }
                                } else {
                                    let can_connect = self.rt_panel.plugin_installed;
                                    let resp = ui.add_enabled_ui(can_connect, |ui| {
                                        ui.button("● 连接")
                                    });
                                    if !can_connect {
                                        resp.inner.clone().on_hover_text("请先点击「注入插件」，然后启动游戏");
                                    }
                                    if resp.inner.clicked() && can_connect {
                                        self.rt_connect();
                                    }
                                }

                                if !self.rt_panel.plugin_installed {
                                    if ui.button("注入插件").clicked() {
                                        self.inject_plugin();
                                    }
                                } else {
                                    ui.colored_label(
                                        crate::theme::colors::SUCCESS,
                                        "✓ 插件已注入",
                                    );
                                    if ui.button("🗑 移除插件").clicked() {
                                        if let Some(ref dir) = self.game_dir {
                                            match game_tool_rpgmaker::tcp::remove_plugin(dir) {
                                                Ok(()) => {
                                                    self.rt_panel.plugin_installed = false;
                                                    self.status_message = "插件已移除".into();
                                                }
                                                Err(e) => {
                                                    self.status_message = format!("移除失败: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            });

                            // 刷新控制行
                            ui.horizontal(|ui| {
                                let auto = self.rt_panel.auto_refresh;
                                if ui.selectable_label(auto, if auto { "▶ 自动刷新" } else { "⏸ 暂停刷新" }).clicked() {
                                    self.rt_panel.auto_refresh = !auto;
                                }
                                if ui.button("📥 手动刷新").clicked() {
                                    self.rt_send_command(BridgeCommand::ReadAll);
                                }
                                ui.label("间隔:");
                                egui::ComboBox::from_id_salt("refresh_interval")
                                    .selected_text(format!("{}秒", self.rt_panel.refresh_interval_secs))
                                    .show_ui(ui, |ui| {
                                        for secs in &[1u64, 2, 3, 5] {
                                            if ui.selectable_label(self.rt_panel.refresh_interval_secs == *secs, format!("{}秒", secs)).clicked() {
                                                self.rt_panel.refresh_interval_secs = *secs;
                                                self.rt_panel.last_refresh = None;
                                            }
                                        }
                                    });
                            });

                            // 错误消息和反馈
                            if !self.rt_panel.error_message.is_empty() {
                                ui.colored_label(crate::theme::colors::ERROR, &self.rt_panel.error_message);
                            }
                            if !self.rt_panel.write_feedback.is_empty() {
                                ui.colored_label(crate::theme::colors::SUCCESS, &self.rt_panel.write_feedback);
                            }

                            ui.separator();

                            // 实时编辑器字段表
                            let actions = realtime_editor::render(ui, &mut self.rt_panel, &self.save_panel.fields);
                            for action in actions {
                                match action {
                                    realtime_editor::RtAction::WriteField(id, val) => {
                                        self.rt_send_command(BridgeCommand::WriteField(id, val));
                                    }
                                    realtime_editor::RtAction::ToggleLock(fid) => {
                                        if self.rt_panel.locked_fields.contains(&fid) {
                                            self.rt_panel.locked_fields.remove(&fid);
                                        } else {
                                            self.rt_panel.locked_fields.insert(fid);
                                        }
                                    }
                                    realtime_editor::RtAction::CopyToSave(fid) => {
                                        if let Some(rt_field) = self.rt_panel.fields.iter().find(|f| f.field_id == fid) {
                                            if let Some(save_field) = self.save_panel.fields.iter_mut().find(|f| f.field_id == fid) {
                                                save_field.save_value = rt_field.live_value.clone();
                                                save_field.dirty = true;
                                            }
                                        }
                                        self.save_panel.dirty_count = self.save_panel.fields.iter().filter(|f| f.dirty).count();
                                    }
                                }
                            }
                        }
                    }
                    // ----- 备份管理面板 -----
                    TabMode::BackupManager => {
                        let actions = backup::render(ui, self);
                        for action in actions {
                            match action {
                                backup::BackupAction::CreateBackup => self.create_backup(),
                                backup::BackupAction::Restore(i) => {
                                    // 恢复备份前弹出确认对话框
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "恢复备份".into(),
                                        message: "确定用此备份覆盖当前存档？此操作不可撤销。".into(),
                                        on_confirm: ConfirmAction::RestoreBackup(i),
                                    });
                                }
                                backup::BackupAction::Delete(i) => {
                                    // 单删除前弹出确认对话框
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "删除备份".into(),
                                        message: "确定删除此备份文件？".into(),
                                        on_confirm: ConfirmAction::DeleteSingleBackup(i),
                                    });
                                }
                                backup::BackupAction::BatchDelete(indices) => {
                                    // 批量删除前弹出确认对话框
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "批量删除".into(),
                                        message: format!("确定删除选中的 {} 个备份文件？", indices.len()),
                                        on_confirm: ConfirmAction::DeleteBackups(indices),
                                    });
                                }
                            }
                        }
                    }
                    // ----- 工具箱面板（LZ/Base64/存档信息/完整性检查/批量扫描/修复） -----
                    TabMode::Toolbox => {
                        let actions = toolbox::render(ui, &mut self.toolbox);
                        for action in actions {
                            match action {
                                ToolboxAction::GetSaveInfo(path) => {
                                    self.toolbox.info_result =
                                        Some(game_tool_core::integrity::get_save_info(&path));
                                }
                                ToolboxAction::IntegrityCheck(path) => {
                                    self.toolbox.check_result =
                                        Some(game_tool_core::integrity::check_save_integrity(&path));
                                }
                                ToolboxAction::BatchCheck(dir) => {
                                    self.toolbox.batch_results =
                                        game_tool_core::integrity::batch_check_saves(&dir);
                                }
                                ToolboxAction::RepairSave(path) => {
                                    self.toolbox.repair_result =
                                        Some(game_tool_core::integrity::attempt_repair(&path));
                                }
                                ToolboxAction::ClearCheck => {
                                    self.toolbox.check_result = None;
                                }
                                ToolboxAction::ClearBatch => {
                                    self.toolbox.batch_results.clear();
                                }
                                ToolboxAction::ClearRepair => {
                                    self.toolbox.repair_result = None;
                                }
                            }
                        }
                    }
                    // ----- 设置面板 -----
                    TabMode::Settings => {
                        let actions = settings::render(ui, self);
                        for action in actions {
                            match action {
                                settings::SettingsAction::ToggleDarkMode => {
                                    self.dark_mode = !self.dark_mode;
                                }
                                settings::SettingsAction::SetPort(port) => {
                                    self.rt_panel.port = port;
                                    if self.rt_panel.conn.is_some() {
                                        self.status_message = "端口已更改，请断开后重新连接以生效。".into();
                                    }
                                }
                                settings::SettingsAction::RemoveRecentGame(path) => {
                                    self.recent_games.retain(|g| g != &path);
                                }
                                settings::SettingsAction::ClearRecentGames => {
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "清除记录".into(),
                                        message: "确定清除所有最近游戏记录？".into(),
                                        on_confirm: ConfirmAction::ClearRecentGames,
                                    });
                                }
                                settings::SettingsAction::SaveAll => {
                                    if let Ok(mut cfg) = load_config() {
                                        cfg.dark_mode = self.dark_mode;
                                        cfg.tcp_port = self.rt_panel.port;
                                        cfg.recent_games = self.recent_games.clone();
                                        match game_tool_core::config::save_config(&cfg) {
                                            Ok(()) => self.status_message = "设置已保存".into(),
                                            Err(e) => self.status_message = format!("保存失败: {}", e),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
        } else {
            // ===================================================================
            // 无游戏加载状态：显示启动面板，可打开/选择游戏目录
            // ===================================================================
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let actions = startup::render(ui, self);
                    for action in actions {
                        match action {
                            startup::StartupAction::OpenGameDir => self.switch_game(),
                            startup::StartupAction::OpenRecentGame(path) => {
                                // 从最近游戏列表打开游戏（逻辑与 switch_game 类似）
                                self.game_dir = Some(path.clone());
                                self.engine = detect_by_filesystem(&path);
                                self.game_config = if self.engine != EngineType::Unknown {
                                    let gc =
                                        game_tool_rpgmaker::scanner::scan_game_directory(&path);
                                    if gc.data_loaded {
                                        self.game_title = gc.game_title.clone();
                                        Some(gc)
                                    } else {
                                        self.game_title.clear();
                                        None
                                    }
                                } else {
                                    self.game_title.clear();
                                    None
                                };
                                self.save_panel.format = create_format(&self.engine);
                                self.save_panel.panel_mode =
                                    factory::engine_to_panel_mode(&self.engine);
                                self.save_panel.readonly = false;
                                self.save_panel.selected_save = None;
                                self.save_panel.save_data = None;
                                self.save_panel.summary = None;
                                self.save_panel.fields.clear();
                                self.save_panel.dirty_count = 0;
                                self.save_panel.selected_category = None;
                                self.save_panel.search_query.clear();
                                self.save_panel.jump_id.clear();

                                if let Some(ref conn) = self.rt_panel.conn {
                                    let _ = conn.cmd_tx.send(BridgeJob::Disconnect);
                                }
                                self.rt_panel.conn = None;
                                self.rt_panel.fields.clear();
                                self.rt_panel.plugin_installed = false;
                                self.rt_panel.error_message.clear();
                                self.rt_panel.error_expires_at = None;
                                self.rt_panel.write_feedback.clear();
                                self.rt_panel.write_feedback_expires_at = None;
                                self.rt_panel.search_query.clear();
                                self.rt_panel.selected_category = None;
                                self.rt_panel.jump_id.clear();
                                self.rt_panel.auto_refresh = true;
                                self.rt_panel.locked_fields.clear();
                                self.rt_panel.last_refresh = None;
                                self.rt_panel.bridge_mode = if matches!(self.engine, EngineType::Unreal | EngineType::UnityMono | EngineType::UnityIl2Cpp | EngineType::Godot) { BridgeMode::Memory } else { BridgeMode::Tcp };
                                self.rt_panel.process_list.clear();
                                self.rt_panel.selected_process = None;
                                self.rt_panel.scan_value.clear();
                                self.rt_panel.scan_results.clear();
                                self.rt_panel.scan_count = 0;
                                self.rt_panel.field_seeds.clear();
                                self.rt_panel.save_fields_snapshot.clear();

                                if factory::supports_realtime(&self.engine) {
                                    match self.engine {
                                        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
                                            self.rt_panel.plugin_installed = game_tool_rpgmaker::tcp::is_plugin_installed(&path);
                                        }
                                        EngineType::RenPy => {
                                            self.rt_panel.plugin_installed = game_tool_renpy::bridge::is_plugin_installed(&path);
                                        }
                                        _ => {}
                                    }
                                }

                                self.backup_paths.clear();
                                self.backup_selection.clear();
                                self.status_message.clear();

                                self.refresh_save_files();

                                if let Some(ref dir) = self.game_dir {
                                    let dir = dir.clone();
                                    self.recent_games.retain(|g| g != &dir);
                                    self.recent_games.insert(0, dir);
                                    self.recent_games.truncate(5);
                                    if let Ok(mut cfg) = load_config() {
                                        cfg.recent_games = self.recent_games.clone();
                                        let _ = game_tool_core::config::save_config(&cfg);
                                    }
                                }

                                self.active_tab = TabMode::SaveEditor;
                            }
                        }
                    }
                });
            });
        }

        // ===================================================================
        // "未保存修改"对话框：切换游戏时如有未保存字段则弹出
        // 选项：保存并切换 / 丢弃修改 / 取消
        // ===================================================================
        if self.show_unsaved_dialog {
            egui::Window::new("未保存的修改")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("有 {} 处未保存的修改。是否保存后再切换？", self.save_panel.dirty_count));
                    ui.horizontal(|ui| {
                        if ui.button("保存并切换").clicked()
                            && self.save_current()
                        {
                            self.show_unsaved_dialog = false;
                            self.switch_game();
                        }
                        if ui.button("丢弃修改").clicked() {
                            // 丢弃修改：重新从文件加载原始数据
                            let mut reload_ok = false;
                            if let Some(ref path) = self.save_panel.selected_save {
                                if let Some(ref format) = self.save_panel.format {
                                    if let Ok(data) = format.load(path) {
                                        let game_dir = self.game_dir.as_deref().unwrap_or("");
                                        let fields = format.scan_fields(&data, game_dir);
                                        self.save_panel.save_data = Some(data);
                                        self.save_panel.fields = fields;
                                        reload_ok = true;
                                    }
                                }
                            }
                            if !reload_ok {
                                self.status_message = "恢复存档数据失败".into();
                            } else {
                                self.save_panel.dirty_count = 0;
                                self.show_unsaved_dialog = false;
                                self.switch_game();
                            }
                        }
                        if ui.button("取消").clicked() {
                            self.show_unsaved_dialog = false;
                        }
                    });
                });
        }

        // ===================================================================
        // 确认对话框：用于删除备份、恢复备份、清除记录等危险操作
        // 包含标题、信息文字、"确认"与"取消"按钮
        // ===================================================================
        if let Some(ref dialog) = self.show_confirm_dialog {
            let title = dialog.title.clone();
            let message = dialog.message.clone();
            // 使用 replace 取出 dialog 所有权，避免借用冲突
            let action = std::mem::replace(&mut self.show_confirm_dialog, None);
            if let Some(dlg) = action {
                egui::Window::new(title)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(&message);
                        ui.horizontal(|ui| {
                            if ui.button("确认").clicked() {
                                match &dlg.on_confirm {
                                    ConfirmAction::DeleteBackups(indices) => {
                                        // 批量删除：倒序删除避免索引偏移
                                        let mut sorted: Vec<usize> = indices.clone();
                                        sorted.sort_by(|a, b| b.cmp(a));
                                        for i in sorted {
                                            if i < self.backup_paths.len() {
                                                let path = self.backup_paths.remove(i);
                                                let _ = std::fs::remove_file(&path);
                                            }
                                        }
                                    }
                                    ConfirmAction::RestoreBackup(i) => {
                                        self.restore_backup(*i);
                                    }
                                    ConfirmAction::ClearRecentGames => {
                                        self.recent_games.clear();
                                    }
                                    ConfirmAction::DeleteSingleBackup(i) => {
                                        self.delete_backup(*i);
                                    }
                                }
                                self.show_confirm_dialog = None;
                            }
                            if ui.button("取消").clicked() {
                                self.show_confirm_dialog = None;
                            }
                        });
                    });
            }
        }

        // ===================================================================
        // 底部状态栏：显示操作反馈、dirty 计数等状态信息
        // ===================================================================
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            status_bar::render(ui, self);
        });
    }
}
