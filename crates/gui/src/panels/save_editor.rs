use crate::state::SavePanelState;
use egui::Ui;
use game_tool_rpgmaker::scanner::GameConfig;

pub enum SaveAction {
    LoadSave,
    RefreshFiles,
    Save,
}

pub fn render(
    ui: &mut Ui,
    _state: &mut SavePanelState,
    _game_config: Option<&GameConfig>,
) -> Vec<SaveAction> {
    ui.heading("\u{1f4c2} \u{5b58}\u{6863}\u{7f16}\u{8f91}");
    ui.add_space(8.0);
    ui.colored_label(
        crate::theme::colors::TEXT_SECONDARY,
        "\u{5b58}\u{6863}\u{7f16}\u{8f91}\u{529f}\u{80fd}\u{5373}\u{5c06}\u{5b9e}\u{73b0}...",
    );
    Vec::new()
}
