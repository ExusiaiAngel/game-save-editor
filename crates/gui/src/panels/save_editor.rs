use crate::state::{AppState, ConnectionStatus, SavePanelMode};
use crate::theme::{self, colors};
use egui::Ui;

pub enum SaveAction {
    LoadSave,
    RefreshFiles,
    Save,
    UndoDirty,
}

pub fn render(ui: &mut Ui, state: &mut AppState) -> Vec<SaveAction> {
    let mut actions = Vec::new();

    // ── Game overview row ──
    ui.horizontal(|ui| {
        if !state.game_title.is_empty() {
            ui.label(format!("\u{1f3ae} {}", state.game_title));
        } else {
            ui.label("\u{1f3ae} \u{672a}\u{77e5}\u{6e38}\u{620f}");
        }
        ui.separator();
        let ename = theme::engine_display_name(&state.engine);
        ui.label(format!("\u{5f15}\u{64ce}: {}", ename));

        ui.separator();
        let conn_status = state
            .rt_panel
            .conn
            .as_ref()
            .map(|c| c.status)
            .unwrap_or(ConnectionStatus::Disconnected);
        let (status_icon, status_color, status_text) = match conn_status {
            ConnectionStatus::Connected => ("\u{25cf}", colors::SUCCESS, "\u{5df2}\u{8fde}\u{63a5}"),
            ConnectionStatus::Connecting => {
                ("\u{25cc}", colors::WARNING, "\u{8fde}\u{63a5}\u{4e2d}...")
            }
            ConnectionStatus::Disconnected => {
                ("\u{25cb}", colors::TEXT_DISABLED, "\u{672a}\u{8fde}\u{63a5}")
            }
        };
        ui.colored_label(status_color, format!("{} {}", status_icon, status_text));

        ui.separator();
        let currency = state
            .game_config
            .as_ref()
            .map(|c| c.currency_unit.as_str())
            .unwrap_or("G");
        ui.colored_label(
            colors::TEXT_SECONDARY,
            format!("\u{91d1}\u{5e01}\u{5355}\u{4f4d}: {}", currency),
        );
    });

    ui.add_space(6.0);

    // ── Save file selector row ──
    ui.horizontal(|ui| {
        ui.label("\u{5b58}\u{6863}:");
        let files: Vec<String> = state.save_panel.save_files.clone();
        let selected_idx = state
            .save_panel
            .selected_save
            .as_ref()
            .and_then(|sel| files.iter().position(|f| f == sel));

        let mut current_label = "\u{2014} \u{9009}\u{62e9}\u{5b58}\u{6863} \u{2014}".to_string();
        if let Some(idx) = selected_idx {
            let path = &files[idx];
            if let Some(name) = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
            {
                current_label = name.to_string();
            }
        }

        egui::ComboBox::from_id_salt("save_file_selector")
            .selected_text(&current_label)
            .show_ui(ui, |ui| {
                for (i, f) in files.iter().enumerate() {
                    let display = std::path::Path::new(f)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(f);
                    if ui.selectable_label(selected_idx == Some(i), display).clicked() {
                        state.save_panel.selected_save = Some(f.clone());
                        actions.push(SaveAction::LoadSave);
                    }
                }
            });

        if ui.button("\u{1f504} \u{5237}\u{65b0}").clicked() {
            actions.push(SaveAction::RefreshFiles);
        }

        if state.save_panel.dirty_count > 0 {
            if ui
                .button(format!(
                    "\u{1f4be} \u{4fdd}\u{5b58} ({})",
                    state.save_panel.dirty_count
                ))
                .clicked()
            {
                actions.push(SaveAction::Save);
            }
        }
    });

    ui.add_space(8.0);

    // ── Filter bar ──
    let is_rpgmaker = state.save_panel.panel_mode == SavePanelMode::RpgMaker
        && !state.save_panel.fields.is_empty();
    if is_rpgmaker {
        crate::widgets::category_tree::render_horizontal(
            ui,
            &state.save_panel.fields,
            &mut state.save_panel.selected_category,
        );
        ui.add_space(4.0);
    }

    ui.horizontal(|ui| {
        crate::widgets::search_bar::render(ui, &mut state.save_panel.search_query);

        ui.label("ID:");
        let _jump_resp = ui.add(
            egui::TextEdit::singleline(&mut state.save_panel.jump_id)
                .desired_width(80.0)
                .hint_text("\u{8df3}\u{8f6c}..."),
        );

        if state.save_panel.dirty_count > 0 {
            if ui.button("\u{21a9} \u{64a4}\u{9500}").clicked() {
                actions.push(SaveAction::UndoDirty);
            }
        }
    });

    if !state.save_panel.fields.is_empty() {
        ui.add_space(6.0);

        // ── Summary card ──
        if let Some(ref summary) = state.save_panel.summary {
            crate::widgets::summary_card::render(
                ui,
                summary,
                state
                    .game_config
                    .as_ref()
                    .map(|c| c.currency_unit.as_str())
                    .unwrap_or("G"),
            );
            ui.add_space(6.0);
        }

        // ── Field table ──
        let rt_connected = state
            .rt_panel
            .conn
            .as_ref()
            .map(|c| c.status == ConnectionStatus::Connected)
            .unwrap_or(false);

        let live_fields: Option<&[game_tool_core::ModifiableField]> =
            if rt_connected {
                Some(&state.rt_panel.fields)
            } else {
                None
            };

        let dirty_count = crate::widgets::field_table::render(
            ui,
            &mut state.save_panel.fields,
            state.save_panel.readonly,
            &state.save_panel.search_query,
            &state.save_panel.selected_category,
            &mut state.save_panel.jump_id,
            live_fields,
        );
        state.save_panel.dirty_count = dirty_count;
    }

    actions
}
