use crate::state::TabMode;
use crate::theme;
use egui::{vec2, Ui};

pub enum TabAction {
    SwitchTab(TabMode),
    SwitchGame,
}

pub fn render(ui: &mut Ui, active_tab: TabMode, has_game: bool, supports_rt: bool) -> Vec<TabAction> {
    let mut actions = Vec::new();

    let tabs = [
        TabMode::SaveEditor,
        TabMode::RealtimeEditor,
        TabMode::BackupManager,
        TabMode::Toolbox,
        TabMode::Settings,
    ];

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

        for tab in &tabs {
            let selected = active_tab == *tab;
            let enabled = match tab {
                TabMode::SaveEditor | TabMode::BackupManager => has_game,
                TabMode::RealtimeEditor => has_game && supports_rt,
                TabMode::Toolbox | TabMode::Settings => true,
            };

            let label = format!("{} {}", theme::tab_icon(tab), theme::tab_name(tab));

            let resp = ui
                .add_enabled_ui(enabled, |ui| ui.selectable_label(selected, label))
                .inner;

            if selected {
                let underline = egui::Rect::from_min_size(
                    egui::pos2(resp.rect.left(), resp.rect.bottom()),
                    egui::vec2(resp.rect.width(), 2.0),
                );
                ui.painter()
                    .rect_filled(underline, 0.0, theme::colors::ACCENT);
            }

            if resp.clicked() && enabled {
                actions.push(TabAction::SwitchTab(*tab));
            }

            if !enabled {
                let reason = match tab {
                    TabMode::RealtimeEditor if !supports_rt => {
                        "\u{5f53}\u{524d}\u{5f15}\u{64ce}\u{4e0d}\u{652f}\u{6301}\u{5b9e}\u{65f6}\u{4fee}\u{6539}"
                    }
                    _ => "\u{8bf7}\u{5148}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}",
                };
                resp.on_hover_text(reason);
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("\u{5207}\u{6362}\u{6e38}\u{620f}...").clicked() {
                actions.push(TabAction::SwitchGame);
            }
        });
    });

    actions
}
