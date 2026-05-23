use crate::state::TabMode;
use crate::theme;
use egui::{vec2, Color32, Ui};

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
            let text_color = if enabled {
                if selected {
                    theme::colors::ACCENT
                } else {
                    theme::colors::TEXT
                }
            } else {
                theme::colors::TEXT_DISABLED
            };

            let galley = ui.painter().layout_no_wrap(
                label.clone(),
                egui::FontId::proportional(13.0),
                text_color,
            );
            let padding = vec2(16.0, 8.0);
            let desired_size = galley.size() + padding * 2.0;

            let (_, rect) = ui.allocate_space(desired_size);
            let resp = ui.interact(rect, ui.next_auto_id(), egui::Sense::click());

            if resp.hovered() && enabled {
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    if selected {
                        Color32::from_rgba_premultiplied(
                            theme::colors::ACCENT.r(),
                            theme::colors::ACCENT.g(),
                            theme::colors::ACCENT.b(),
                            20,
                        )
                    } else {
                        theme::colors::HOVER_DARK
                    },
                );
            } else if selected {
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_rgba_premultiplied(
                        theme::colors::ACCENT.r(),
                        theme::colors::ACCENT.g(),
                        theme::colors::ACCENT.b(),
                        20,
                    ),
                );
                let underline = egui::Rect::from_min_size(
                    rect.left_bottom() - vec2(0.0, 3.0),
                    vec2(rect.width(), 3.0),
                );
                ui.painter().rect_filled(underline, 0.0, theme::colors::ACCENT);
            }

            let text_pos = rect.center();
            ui.painter().galley(text_pos, galley, text_color);

            if resp.clicked() && enabled {
                actions.push(TabAction::SwitchTab(*tab));
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
