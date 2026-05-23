use crate::theme::{self, colors};
use egui::Ui;
use game_tool_core::detector::EngineType;

pub fn render(
    ui: &mut Ui,
    has_game: bool,
    game_title: &str,
    engine: &EngineType,
    game_dir: &Option<String>,
) {
    ui.horizontal(|ui| {
        ui.heading("\u{1f3ae} GameSaveEditor");
        ui.separator();

        if has_game {
            if let Some(ref dir) = game_dir {
                let short = std::path::Path::new(dir)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| dir.clone());
                ui.colored_label(
                    colors::TEXT_SECONDARY,
                    format!("\u{6e38}\u{620f}: {}", short),
                )
                .on_hover_text(dir);
                ui.separator();
            }
            let ename = theme::engine_display_name(engine);
            ui.label(format!("\u{5f15}\u{64ce}: {}", ename));
            if !game_title.is_empty() {
                ui.separator();
                ui.label(format!("\u{6807}\u{9898}: {}", game_title));
            }
        } else {
            ui.colored_label(
                colors::TEXT_SECONDARY,
                "\u{672a}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}",
            );
        }
    });
}
