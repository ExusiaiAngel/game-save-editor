use crate::state::RtPanelState;
use egui::Ui;
use game_tool_core::detector::EngineType;
use serde_json::Value;

pub enum RtAction {
    WriteField(String, Value),
    ReadAll,
    ToggleLock(String),
}

pub fn render(
    ui: &mut Ui,
    _state: &mut RtPanelState,
    _engine: &EngineType,
    _game_dir: &Option<String>,
) -> Vec<RtAction> {
    ui.heading("\u{26a1} \u{5b9e}\u{65f6}\u{4fee}\u{6539}");
    ui.add_space(8.0);
    ui.colored_label(
        crate::theme::colors::TEXT_SECONDARY,
        "\u{5b9e}\u{65f6}\u{4fee}\u{6539}\u{529f}\u{80fd}\u{5373}\u{5c06}\u{5b9e}\u{73b0}...",
    );
    Vec::new()
}
