#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use game_tool_gui::state::AppState;

fn load_cjk_font() -> Option<Vec<u8>> {
    for path in &[
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\SimHei.ttf",
    ] {
        if let Ok(data) = std::fs::read(path) {
            return Some(data);
        }
    }
    None
}

fn main() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("GameSaveEditor"),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "GameSaveEditor",
        native_options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();

            if let Some(cjk_data) = load_cjk_font() {
                fonts.font_data.insert(
                    "CJK".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(cjk_data)),
                );
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "CJK".to_owned());
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .push("CJK".to_owned());
            }

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(AppState::new(None)))
        }),
    );
}
