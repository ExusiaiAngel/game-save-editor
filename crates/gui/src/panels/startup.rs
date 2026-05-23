use crate::state::AppState;
use crate::theme;
use egui::{vec2, Ui};

pub enum StartupAction {
    OpenGameDir,
    OpenRecentGame(String),
}

pub fn render(ui: &mut Ui, state: &AppState) -> Vec<StartupAction> {
    let mut actions = Vec::new();

    let available = ui.available_size();
    let center_y = available.y / 2.0 - 120.0;

    ui.add_space(center_y.max(0.0));
    ui.vertical_centered(|ui| {
        ui.heading("\u{1f3ae} GameSaveEditor");
        ui.add_space(12.0);
        ui.colored_label(
            theme::colors::TEXT_SECONDARY,
            "\u{9009}\u{62e9}\u{4e00}\u{4e2a}\u{6e38}\u{620f}\u{76ee}\u{5f55}\u{5f00}\u{59cb}",
        );
        ui.add_space(16.0);

        let btn = egui::Button::new("\u{1f4c2} \u{6253}\u{5f00}\u{6e38}\u{620f}\u{76ee}\u{5f55}...")
            .min_size(vec2(200.0, 40.0));
        if ui.add(btn).clicked() {
            actions.push(StartupAction::OpenGameDir);
        }

        ui.add_space(24.0);

        if !state.recent_games.is_empty() {
            ui.colored_label(theme::colors::TEXT_SECONDARY, "\u{6700}\u{8fd1}\u{6e38}\u{620f}:");
            ui.add_space(8.0);

            for game_path in &state.recent_games {
                let display = std::path::Path::new(game_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(game_path);
                if ui
                    .selectable_label(false, format!("  \u{1f3ae} {}", display))
                    .clicked()
                {
                    actions.push(StartupAction::OpenRecentGame(game_path.clone()));
                }
            }
        } else {
            ui.colored_label(
                theme::colors::TEXT_DISABLED,
                "\u{6682}\u{65e0}\u{6700}\u{8fd1}\u{6e38}\u{620f}\u{8bb0}\u{5f55}",
            );
        }
    });

    actions
}
