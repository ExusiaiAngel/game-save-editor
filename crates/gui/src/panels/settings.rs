use crate::state::AppState;
use crate::theme::colors;
use egui::Ui;

pub enum SettingsAction {
    ToggleDarkMode,
    SetPort(u16),
}

pub fn render(ui: &mut Ui, state: &AppState) -> Vec<SettingsAction> {
    let mut actions = Vec::new();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("\u{2699} \u{8bbe}\u{7f6e}");
        ui.add_space(12.0);

        ui.collapsing("\u{5916}\u{89c2}", |ui| {
            ui.horizontal(|ui| {
                ui.label("\u{4e3b}\u{9898}\u{6a21}\u{5f0f}:");
                let label = if state.dark_mode {
                    "\u{1f319} \u{6697}\u{8272}"
                } else {
                    "\u{2600} \u{4eae}\u{8272}"
                };
                if ui.button(label).clicked() {
                    actions.push(SettingsAction::ToggleDarkMode);
                }
            });
        });

        ui.add_space(8.0);

        ui.collapsing("\u{8fde}\u{63a5}\u{8bbe}\u{7f6e}", |ui| {
            ui.horizontal(|ui| {
                ui.label("\u{9ed8}\u{8ba4}\u{7aef}\u{53e3}:");
                let mut port = state.rt_panel.port;
                if ui
                    .add(egui::DragValue::new(&mut port).range(1024..=65535))
                    .changed()
                {
                    actions.push(SettingsAction::SetPort(port));
                }
            });
        });

        ui.add_space(8.0);

        ui.collapsing("\u{5173}\u{4e8e}", |ui| {
            ui.label("GameSaveEditor");
            ui.colored_label(
                colors::TEXT_SECONDARY,
                "\u{8de8}\u{5f15}\u{64ce}\u{6e38}\u{620f}\u{5b58}\u{6863}\u{7f16}\u{8f91}\u{5668}",
            );
            ui.add_space(4.0);
            ui.label("\u{652f}\u{6301}\u{5f15}\u{64ce}:");
            ui.label("  RPG Maker MV / MZ (NW.js)");
            ui.label("  Ren'Py");
            ui.label("  Unreal Engine (GVAS \u{53ea}\u{8bfb})");
            ui.label("  Unity / Godot (\u{901a}\u{7528} JSON)");
        });
    });

    actions
}
