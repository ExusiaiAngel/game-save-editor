use game_tool_core::config::load_config;
use game_tool_core::detector::{detect_by_filesystem, EngineType};
use game_tool_core::BridgeCommand;
use serde_json::Value;

use crate::connection;
use crate::discovery;
use crate::factory::{self, create_format};
use crate::panels::{
    backup, realtime_editor, save_editor, settings, startup, status_bar, tab_bar, toolbox, top_bar,
};
use crate::state::{
    AppState, BridgeJob, BridgeResult, ConfirmAction, ConfirmDialog, ConnectionStatus, RtPanelState,
    SavePanelState, TabMode, ToolboxState,
};

impl AppState {
    pub fn new(game_dir: Option<String>) -> Self {
        let config = load_config().unwrap_or_default();
        let port = config.tcp_port;
        let dark_mode = config.dark_mode;

        let engine = game_dir
            .as_ref()
            .map(|d| detect_by_filesystem(d))
            .unwrap_or(EngineType::Unknown);

        let game_config = if let Some(ref dir) = game_dir {
            if engine != EngineType::Unknown {
                let gc = game_tool_rpgmaker::scanner::scan_game_directory(dir);
                if gc.data_loaded {
                    Some(gc)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let game_title = game_config
            .as_ref()
            .map(|gc| gc.game_title.clone())
            .unwrap_or_default();

        let panel_mode = factory::engine_to_panel_mode(&engine);
        let readonly = factory::is_readonly(&engine);
        let format = create_format(&engine);
        let save_files = if let (Some(ref dir), Some(ref fmt)) = (&game_dir, &format) {
            discovery::find_save_files(dir, &**fmt)
        } else {
            Vec::new()
        };

        let plugin_installed = if factory::supports_realtime(&engine) {
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
            save_panel: SavePanelState {
                format,
                save_files,
                panel_mode,
                readonly,
                selected_save: None,
                save_data: None,
                summary: None,
                fields: Vec::new(),
                dirty_count: 0,
                selected_category: None,
                search_query: String::new(),
                jump_id: String::new(),
            },
            rt_panel: RtPanelState {
                conn: None,
                fields: Vec::new(),
                plugin_installed,
                host: "127.0.0.1".into(),
                port,
                error_message: String::new(),
                error_remaining: 0,
                write_feedback: String::new(),
                write_feedback_remaining: 0,
                search_query: String::new(),
                jump_id: String::new(),
                auto_refresh: true,
                locked_fields: std::collections::HashSet::new(),
                refresh_interval_secs: 3,
                last_refresh: None,
            },
            toolbox: ToolboxState {
                lz_input: String::new(),
                lz_output: String::new(),
                lz_error: String::new(),
                b64_input: String::new(),
                b64_output: String::new(),
            },
            status_message: String::new(),
            show_unsaved_dialog: false,
            show_confirm_dialog: None,
        }
    }

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
                self.save_panel.search_query.clear();
                self.save_panel.selected_category = None;
                self.save_panel.jump_id.clear();
            }
            Err(e) => {
                self.status_message = format!("\u{52a0}\u{8f7d}\u{5b58}\u{6863}\u{5931}\u{8d25}: {}", e);
            }
        }
    }

    fn save_current(&mut self) -> bool {
        let path = match &self.save_panel.selected_save {
            Some(p) => p.clone(),
            None => {
                self.status_message = "\u{672a}\u{9009}\u{62e9}\u{5b58}\u{6863}\u{6587}\u{4ef6}".into();
                return false;
            }
        };
        let save_data = match &mut self.save_panel.save_data {
            Some(d) => d,
            None => {
                self.status_message = "\u{5b58}\u{6863}\u{6570}\u{636e}\u{4e3a}\u{7a7a}".into();
                return false;
            }
        };
        let format = match &self.save_panel.format {
            Some(ref f) => f,
            None => return false,
        };

        let dirty: Vec<_> = self
            .save_panel
            .fields
            .iter()
            .filter(|f| f.dirty)
            .cloned()
            .collect();

        for field in &dirty {
            if let Err(e) = format.apply_field(save_data, field) {
                self.status_message = format!("\u{5199}\u{5165}\u{5b57}\u{6bb5} {} \u{5931}\u{8d25}: {}", field.display_name, e);
                return false;
            }
        }

        match format.save(&path, save_data) {
            Ok(()) => {
                for f in &mut self.save_panel.fields {
                    f.dirty = false;
                }
                self.save_panel.dirty_count = 0;
                self.status_message = "\u{5b58}\u{6863}\u{5df2}\u{4fdd}\u{5b58}".into();
                true
            }
            Err(e) => {
                self.status_message = format!("\u{4fdd}\u{5b58}\u{5931}\u{8d25}: {}", e);
                false
            }
        }
    }

    fn refresh_save_files(&mut self) {
        if let (Some(ref dir), Some(ref fmt)) = (&self.game_dir, &self.save_panel.format) {
            self.save_panel.save_files = discovery::find_save_files(dir, &**fmt);
        }
    }

    fn switch_game(&mut self) {
        if let Some(new_dir) = rfd::FileDialog::new()
            .set_title("\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}")
            .pick_folder()
        {
            let dir_str = new_dir.to_string_lossy().to_string();
            self.game_dir = Some(dir_str.clone());
            self.engine = detect_by_filesystem(&dir_str);

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

            self.save_panel.format = create_format(&self.engine);
            self.save_panel.panel_mode = factory::engine_to_panel_mode(&self.engine);
            self.save_panel.readonly = factory::is_readonly(&self.engine);
            self.save_panel.selected_save = None;
            self.save_panel.save_data = None;
            self.save_panel.summary = None;
            self.save_panel.fields.clear();
            self.save_panel.dirty_count = 0;
            self.save_panel.selected_category = None;
            self.save_panel.search_query.clear();
            self.rt_panel.plugin_installed = false;

            if let Some(ref conn) = self.rt_panel.conn {
                let _ = conn.cmd_tx.send(BridgeJob::Disconnect);
            }
            self.rt_panel.conn = None;
            self.rt_panel.fields.clear();
            self.rt_panel.error_message.clear();
            self.rt_panel.error_remaining = 0;
            self.rt_panel.write_feedback.clear();
            self.rt_panel.write_feedback_remaining = 0;
            self.rt_panel.search_query.clear();
            self.rt_panel.jump_id.clear();
            self.rt_panel.auto_refresh = true;
            self.rt_panel.locked_fields.clear();
            self.rt_panel.last_refresh = None;

            self.save_panel.jump_id.clear();
            self.backup_paths.clear();
            self.backup_selection.clear();
            self.status_message.clear();

            self.refresh_save_files();

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

    fn inject_plugin(&mut self) {
        let dir = match &self.game_dir {
            Some(d) => d.clone(),
            None => {
                self.status_message = "\u{672a}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}".into();
                return;
            }
        };
        let result = match self.engine {
            EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
                game_tool_rpgmaker::tcp::inject_plugin(&dir, self.rt_panel.port).map_err(|e| e)
            }
            EngineType::RenPy => game_tool_renpy::bridge::inject_plugin(&dir).map_err(|e| e),
            _ => Err("\u{4e0d}\u{652f}\u{6301}".into()),
        };
        match result {
            Ok(()) => {
                self.rt_panel.plugin_installed = true;
            }
            Err(e) => {
                self.status_message = format!("\u{6ce8}\u{5165}\u{5931}\u{8d25}: {}", e);
            }
        }
    }

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

    fn rt_disconnect(&mut self) {
        if let Some(ref conn) = self.rt_panel.conn {
            let _ = conn.cmd_tx.send(BridgeJob::Disconnect);
        }
    }

    fn drain_rt_results(&mut self) {
        // Time-based auto-refresh
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
                None => true,
            };
            if should_refresh {
                self.rt_panel.last_refresh = Some(std::time::Instant::now());
                if let Some(ref conn) = self.rt_panel.conn {
                    let _ = conn.cmd_tx.send(BridgeJob::Execute(BridgeCommand::ReadAll));
                }
            }
        }

        if let Some(ref mut conn) = self.rt_panel.conn {
            let results = connection::drain_results(conn);
            for result in results {
                match result {
                    BridgeResult::Connected => {
                        conn.status = ConnectionStatus::Connected;
                        self.rt_panel.error_message.clear();
                        self.rt_panel.error_remaining = 0;
                        self.rt_panel.last_refresh = Some(std::time::Instant::now());
                        let _ = conn.cmd_tx.send(BridgeJob::Execute(BridgeCommand::ReadAll));
                    }
                    BridgeResult::Disconnected => {
                        conn.status = ConnectionStatus::Disconnected;
                        self.rt_panel.fields.clear();
                        self.rt_panel.error_message.clear();
                        self.rt_panel.error_remaining = 0;
                        self.rt_panel.last_refresh = None;
                    }
                    BridgeResult::CommandResult(val) => {
                        if let Ok(gs) =
                            serde_json::from_value::<game_tool_core::GameState>(val.clone())
                        {
                            let mut new_fields = factory::game_state_to_fields(
                                &gs,
                                &self.engine,
                                self.game_config.as_ref(),
                            );
                            // Restore locked field values from old fields
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
                            self.rt_panel.write_feedback = "\u{2713} \u{5df2}\u{5199}\u{5165}".into();
                            self.rt_panel.write_feedback_remaining = 120;
                        }
                    }
                    BridgeResult::Error(e) => {
                        let is_fatal = e.contains("\u{8fde}\u{63a5}\u{5931}\u{8d25}")
                            || e.contains("\u{672a}\u{8fde}\u{63a5}")
                            || e.contains("connection refused")
                            || e.contains("closed");
                        if is_fatal {
                            conn.status = ConnectionStatus::Disconnected;
                            self.rt_panel.fields.clear();
                        }
                        self.rt_panel.error_message = e;
                        self.rt_panel.error_remaining = 300;
                    }
                }
            }
        }

        // Auto-clear timed messages
        if self.rt_panel.error_remaining > 0 {
            self.rt_panel.error_remaining -= 1;
            if self.rt_panel.error_remaining == 0 {
                self.rt_panel.error_message.clear();
            }
        }
        if self.rt_panel.write_feedback_remaining > 0 {
            self.rt_panel.write_feedback_remaining -= 1;
            if self.rt_panel.write_feedback_remaining == 0 {
                self.rt_panel.write_feedback.clear();
            }
        }
    }

    fn rt_send_command(&self, cmd: BridgeCommand) {
        if let Some(ref conn) = self.rt_panel.conn {
            let _ = conn.cmd_tx.send(BridgeJob::Execute(cmd));
        }
    }

    fn create_backup(&mut self) {
        let path = match &self.save_panel.selected_save {
            Some(p) => p.clone(),
            None => {
                self.status_message = "\u{672a}\u{9009}\u{62e9}\u{5b58}\u{6863}".into();
                return;
            }
        };
        match game_tool_core::backup::save_backup(std::path::Path::new(&path), 5) {
            Ok(backup_path) => {
                self.backup_paths
                    .push(backup_path.to_string_lossy().to_string());
                self.status_message = "\u{5907}\u{4efd}\u{5df2}\u{521b}\u{5efa}".into();
            }
            Err(e) => {
                self.status_message =
                    format!("\u{521b}\u{5efa}\u{5907}\u{4efd}\u{5931}\u{8d25}: {}", e);
            }
        }
    }

    fn restore_backup(&mut self, index: usize) {
        if index >= self.backup_paths.len() {
            return;
        }
        let backup_path = self.backup_paths[index].clone();
        let target = match &self.save_panel.selected_save {
            Some(p) => p.clone(),
            None => {
                self.status_message = "\u{672a}\u{9009}\u{62e9}\u{5b58}\u{6863}".into();
                return;
            }
        };
        if let Err(e) = std::fs::copy(&backup_path, &target) {
            self.status_message = format!("\u{6062}\u{590d}\u{5931}\u{8d25}: {}", e);
        } else {
            self.status_message =
                "\u{5907}\u{4efd}\u{5df2}\u{6062}\u{590d}\u{5230}\u{5f53}\u{524d}\u{5b58}\u{6863}"
                    .into();
            self.load_save_file();
        }
    }

    fn delete_backup(&mut self, index: usize) {
        if index >= self.backup_paths.len() {
            return;
        }
        let path = self.backup_paths.remove(index);
        match std::fs::remove_file(&path) {
            Ok(()) => {
                self.status_message = "\u{5907}\u{4efd}\u{5df2}\u{5220}\u{9664}".into();
            }
            Err(e) => {
                self.backup_paths.insert(index, path);
                self.status_message = format!("\u{5220}\u{9664}\u{5931}\u{8d25}: {}", e);
            }
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_rt_results();

        crate::theme::Theme::new(self.dark_mode).apply(ctx);

        let has_game = self.game_dir.is_some();

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            top_bar::render(ui, has_game, &self.game_title, &self.engine, &self.game_dir);
        });

        if has_game {
            let supports_rt = factory::supports_realtime(&self.engine);
            egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
                let actions = tab_bar::render(ui, self.active_tab, has_game, supports_rt);
                for action in actions {
                    match action {
                        tab_bar::TabAction::SwitchTab(tab) => {
                            self.active_tab = tab;
                        }
                        tab_bar::TabAction::SwitchGame => {
                            if self.save_panel.dirty_count > 0 {
                                self.show_unsaved_dialog = true;
                            } else {
                                self.switch_game();
                            }
                        }
                    }
                }
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                match self.active_tab {
                    TabMode::SaveEditor => {
                        if self.game_dir.is_none() {
                            ui.colored_label(
                                egui::Color32::from_rgb(139, 148, 158),
                                "\u{8bf7}\u{5148}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}\u{3002}",
                            );
                        } else {
                            let actions = save_editor::render(ui, self);
                            for action in actions {
                                match action {
                                    save_editor::SaveAction::LoadSave => self.load_save_file(),
                                    save_editor::SaveAction::RefreshFiles => self.refresh_save_files(),
                                    save_editor::SaveAction::Save => {
                                        self.save_current();
                                    }
                                    save_editor::SaveAction::UndoDirty => {
                                        self.load_save_file();
                                    }
                                }
                            }
                        }
                    }
                    TabMode::RealtimeEditor => {
                        if self.game_dir.is_none() {
                            ui.colored_label(
                                egui::Color32::from_rgb(139, 148, 158),
                                "\u{8bf7}\u{5148}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}\u{3002}",
                            );
                        } else if !crate::factory::supports_realtime(&self.engine) {
                            ui.colored_label(
                                crate::theme::colors::TEXT_SECONDARY,
                                "\u{5f53}\u{524d}\u{5f15}\u{64ce}\u{4e0d}\u{652f}\u{6301}\u{5b9e}\u{65f6}\u{4fee}\u{6539}",
                            );
                        } else {
                            ui.horizontal(|ui| {
                                ui.label("\u{4e3b}\u{673a}:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.rt_panel.host)
                                        .desired_width(100.0),
                                );
                                ui.label("\u{7aef}\u{53e3}:");
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
                                    ui.colored_label(crate::theme::colors::WARNING, "\u{8fde}\u{63a5}\u{4e2d}...");
                                } else if is_connected {
                                    if ui.button("\u{25ce} \u{65ad}\u{5f00}").clicked() {
                                        self.rt_disconnect();
                                    }
                                } else {
                                    if ui.button("\u{25cf} \u{8fde}\u{63a5}").clicked() {
                                        self.rt_connect();
                                    }
                                }

                                if !self.rt_panel.plugin_installed {
                                    if ui.button("\u{6ce8}\u{5165}\u{63d2}\u{4ef6}").clicked() {
                                        self.inject_plugin();
                                    }
                                } else {
                                    ui.colored_label(
                                        crate::theme::colors::SUCCESS,
                                        "\u{2713} \u{63d2}\u{4ef6}\u{5df2}\u{6ce8}\u{5165}",
                                    );
                                }
                            });

                            ui.horizontal(|ui| {
                                let auto = self.rt_panel.auto_refresh;
                                if ui
                                    .selectable_label(
                                        auto,
                                        if auto {
                                            "\u{25b6} \u{81ea}\u{52a8}\u{5237}\u{65b0}"
                                        } else {
                                            "\u{23f8} \u{6682}\u{505c}\u{5237}\u{65b0}"
                                        },
                                    )
                                    .clicked()
                                {
                                    self.rt_panel.auto_refresh = !auto;
                                }
                                if ui
                                    .button("\u{1f4e5} \u{624b}\u{52a8}\u{5237}\u{65b0}")
                                    .clicked()
                                {
                                    self.rt_send_command(BridgeCommand::ReadAll);
                                }
                                ui.label("\u{95f4}\u{9694}:");
                                egui::ComboBox::from_id_salt("refresh_interval")
                                    .selected_text(format!(
                                        "{}秒",
                                        self.rt_panel.refresh_interval_secs
                                    ))
                                    .show_ui(ui, |ui| {
                                        for secs in &[1u64, 2, 3, 5] {
                                            if ui
                                                .selectable_label(
                                                    self.rt_panel.refresh_interval_secs == *secs,
                                                    format!("{}秒", secs),
                                                )
                                                .clicked()
                                            {
                                                self.rt_panel.refresh_interval_secs = *secs;
                                                self.rt_panel.last_refresh = None;
                                            }
                                        }
                                    });
                            });

                            if !self.rt_panel.error_message.is_empty() {
                                ui.colored_label(
                                    crate::theme::colors::ERROR,
                                    &self.rt_panel.error_message,
                                );
                            }
                            if !self.rt_panel.write_feedback.is_empty() {
                                ui.colored_label(
                                    crate::theme::colors::SUCCESS,
                                    &self.rt_panel.write_feedback,
                                );
                            }

                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("\u{1f50d}");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.rt_panel.search_query)
                                        .hint_text("\u{641c}\u{7d22}\u{5b57}\u{6bb5}...")
                                        .desired_width(150.0),
                                );
                                if !self.rt_panel.search_query.is_empty()
                                    && ui.button("\u{2715}").clicked()
                                {
                                    self.rt_panel.search_query.clear();
                                }
                                ui.separator();
                                ui.label("\u{8df3}\u{8f6c} ID:");
                                ui.text_edit_singleline(&mut self.rt_panel.jump_id);
                            });

                            ui.separator();

                            let actions = realtime_editor::render(
                                ui,
                                &mut self.rt_panel,
                                &self.save_panel.fields,
                            );
                            for action in actions {
                                match action {
                                    realtime_editor::RtAction::ReadAll => {
                                        self.rt_send_command(BridgeCommand::ReadAll);
                                    }
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
                                        if let Some(rt_field) =
                                            self.rt_panel.fields.iter().find(|f| f.field_id == fid)
                                        {
                                            if let Some(save_field) = self.save_panel.fields.iter_mut().find(|f| f.field_id == fid) {
                                                save_field.save_value = rt_field.live_value.clone();
                                                save_field.dirty = true;
                                            }
                                        }
                                        self.save_panel.dirty_count =
                                            self.save_panel.fields.iter().filter(|f| f.dirty).count();
                                    }
                                }
                            }
                        }
                    }
                    TabMode::BackupManager => {
                        let actions = backup::render(ui, self);
                        for action in actions {
                            match action {
                                backup::BackupAction::CreateBackup => self.create_backup(),
                                backup::BackupAction::Restore(i) => {
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "\u{6062}\u{590d}\u{5907}\u{4efd}".into(),
                                        message: "\u{786e}\u{5b9a}\u{7528}\u{6b64}\u{5907}\u{4efd}\u{8986}\u{76d6}\u{5f53}\u{524d}\u{5b58}\u{6863}\u{ff1f}\u{6b64}\u{64cd}\u{4f5c}\u{4e0d}\u{53ef}\u{64a4}\u{9500}\u{3002}".into(),
                                        on_confirm: ConfirmAction::RestoreBackup(i),
                                    });
                                }
                                backup::BackupAction::Delete(i) => {
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "\u{5220}\u{9664}\u{5907}\u{4efd}".into(),
                                        message: "\u{786e}\u{5b9a}\u{5220}\u{9664}\u{6b64}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{ff1f}".into(),
                                        on_confirm: ConfirmAction::DeleteSingleBackup(i),
                                    });
                                }
                                backup::BackupAction::BatchDelete(indices) => {
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "\u{6279}\u{91cf}\u{5220}\u{9664}".into(),
                                        message: format!("\u{786e}\u{5b9a}\u{5220}\u{9664}\u{9009}\u{4e2d}\u{7684} {} \u{4e2a}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{ff1f}", indices.len()),
                                        on_confirm: ConfirmAction::DeleteBackups(indices),
                                    });
                                }
                            }
                        }
                    }
                    TabMode::Toolbox => {
                        toolbox::render(ui, &mut self.toolbox);
                    }
                    TabMode::Settings => {
                        let actions = settings::render(ui, self);
                        for action in actions {
                            match action {
                                settings::SettingsAction::ToggleDarkMode => {
                                    self.dark_mode = !self.dark_mode;
                                    if let Ok(mut cfg) = load_config() {
                                        cfg.dark_mode = self.dark_mode;
                                        let _ = game_tool_core::config::save_config(&cfg);
                                    }
                                }
                                settings::SettingsAction::SetPort(port) => {
                                    self.rt_panel.port = port;
                                    if self.rt_panel.conn.is_some() {
                                        self.status_message = "\u{7aef}\u{53e3}\u{5df2}\u{66f4}\u{6539}\u{ff0c}\u{8bf7}\u{65ad}\u{5f00}\u{540e}\u{91cd}\u{65b0}\u{8fde}\u{63a5}\u{4ee5}\u{751f}\u{6548}\u{3002}".into();
                                    }
                                }
                                settings::SettingsAction::RemoveRecentGame(path) => {
                                    self.recent_games.retain(|g| g != &path);
                                }
                                settings::SettingsAction::ClearRecentGames => {
                                    self.show_confirm_dialog = Some(ConfirmDialog {
                                        title: "\u{6e05}\u{9664}\u{8bb0}\u{5f55}".into(),
                                        message: "\u{786e}\u{5b9a}\u{6e05}\u{9664}\u{6240}\u{6709}\u{6700}\u{8fd1}\u{6e38}\u{620f}\u{8bb0}\u{5f55}\u{ff1f}".into(),
                                        on_confirm: ConfirmAction::ClearRecentGames,
                                    });
                                }
                            }
                        }
                    }
                }
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let actions = startup::render(ui, self);
                    for action in actions {
                        match action {
                            startup::StartupAction::OpenGameDir => self.switch_game(),
                            startup::StartupAction::OpenRecentGame(path) => {
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
                                self.save_panel.readonly = factory::is_readonly(&self.engine);
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
                                self.rt_panel.error_remaining = 0;
                                self.rt_panel.write_feedback.clear();
                                self.rt_panel.write_feedback_remaining = 0;
                                self.rt_panel.search_query.clear();
                                self.rt_panel.jump_id.clear();
                                self.rt_panel.auto_refresh = true;
                                self.rt_panel.locked_fields.clear();
                                self.rt_panel.last_refresh = None;

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

        if self.show_unsaved_dialog {
            egui::Window::new("\u{672a}\u{4fdd}\u{5b58}\u{7684}\u{4fee}\u{6539}")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("\u{6709} {} \u{5904}\u{672a}\u{4fdd}\u{5b58}\u{7684}\u{4fee}\u{6539}\u{3002}\u{662f}\u{5426}\u{4fdd}\u{5b58}\u{540e}\u{518d}\u{5207}\u{6362}\u{ff1f}", self.save_panel.dirty_count));
                    ui.horizontal(|ui| {
                        if ui.button("\u{4fdd}\u{5b58}\u{5e76}\u{5207}\u{6362}").clicked()
                            && self.save_current()
                        {
                            self.show_unsaved_dialog = false;
                            self.switch_game();
                        }
                        if ui.button("\u{4e22}\u{5f03}\u{4fee}\u{6539}").clicked() {
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
                                self.status_message = "\u{6062}\u{590d}\u{5b58}\u{6863}\u{6570}\u{636e}\u{5931}\u{8d25}".into();
                            }
                            self.save_panel.dirty_count = 0;
                            self.show_unsaved_dialog = false;
                            self.switch_game();
                        }
                        if ui.button("\u{53d6}\u{6d88}").clicked() {
                            self.show_unsaved_dialog = false;
                        }
                    });
                });
        }

        if let Some(ref dialog) = self.show_confirm_dialog {
            let title = dialog.title.clone();
            let message = dialog.message.clone();
            let action = std::mem::replace(&mut self.show_confirm_dialog, None);
            if let Some(dlg) = action {
                egui::Window::new(title)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(&message);
                        ui.horizontal(|ui| {
                            if ui.button("\u{786e}\u{8ba4}").clicked() {
                                match &dlg.on_confirm {
                                    ConfirmAction::DeleteBackups(indices) => {
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
                            if ui.button("\u{53d6}\u{6d88}").clicked() {
                                self.show_confirm_dialog = None;
                            }
                        });
                    });
            }
        }

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            status_bar::render(ui, self);
        });
    }
}
