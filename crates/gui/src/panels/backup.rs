use crate::state::AppState;
use crate::theme::colors;
use egui::Ui;

pub enum BackupAction {
    Restore(usize),
    Delete(usize),
    CreateBackup,
}

pub fn render(ui: &mut Ui, state: &AppState) -> Vec<BackupAction> {
    let mut actions = Vec::new();

    ui.heading("\u{1f5c4} \u{5907}\u{4efd}\u{7ba1}\u{7406}");
    ui.add_space(8.0);

    if state.game_dir.is_none() {
        ui.colored_label(
            colors::TEXT_SECONDARY,
            "\u{8bf7}\u{5148}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}\u{3002}",
        );
        return actions;
    }

    ui.horizontal(|ui| {
        let current = state
            .save_panel
            .selected_save
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("\u{672a}\u{9009}\u{62e9}\u{5b58}\u{6863}");
        ui.label(format!("\u{5f53}\u{524d}\u{5b58}\u{6863}: {}", current));
    });

    ui.add_space(8.0);

    if state.save_panel.selected_save.is_some() && state.save_panel.save_data.is_some() {
        if ui
            .button("\u{1f4e6} \u{521b}\u{5efa}\u{5907}\u{4efd}")
            .clicked()
        {
            actions.push(BackupAction::CreateBackup);
        }
    } else {
        ui.colored_label(
            colors::TEXT_DISABLED,
            "\u{8bf7}\u{5148}\u{5728}\u{5b58}\u{6863}\u{7f16}\u{8f91}\u{4e2d}\u{52a0}\u{8f7d}\u{4e00}\u{4e2a}\u{5b58}\u{6863}\u{3002}",
        );
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    if state.backup_paths.is_empty() {
        ui.colored_label(
            colors::TEXT_SECONDARY,
            "\u{672a}\u{53d1}\u{73b0}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{3002}\u{52a0}\u{8f7d}\u{5b58}\u{6863}\u{540e}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{5c06}\u{663e}\u{793a}\u{5728}\u{6b64}\u{5904}\u{3002}",
        );
    } else {
        ui.label(format!(
            "\u{5171} {} \u{4e2a}\u{5907}\u{4efd}:",
            state.backup_paths.len()
        ));
        ui.add_space(4.0);

        for (i, bp) in state.backup_paths.iter().enumerate() {
            let name = std::path::Path::new(bp)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(bp);

            let size = std::fs::metadata(bp).map(|m| m.len()).unwrap_or(0);
            let size_str = if size > 1024 {
                format!("{} KB", size / 1024)
            } else {
                format!("{} B", size)
            };

            ui.horizontal(|ui| {
                ui.label(format!("\u{1f4c4} {}", name));
                ui.colored_label(colors::TEXT_SECONDARY, size_str);
                ui.separator();
                if ui.button("\u{267b} \u{6062}\u{590d}").clicked() {
                    actions.push(BackupAction::Restore(i));
                }
                if ui.button("\u{1f5d1} \u{5220}\u{9664}").clicked() {
                    actions.push(BackupAction::Delete(i));
                }
            });
        }
    }

    actions
}
