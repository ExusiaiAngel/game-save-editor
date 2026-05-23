//! 顶部栏面板，显示应用标题、游戏目录、引擎类型和游戏标题。
//!
//! 顶部栏有两种状态：
//! - **已加载游戏**：显示游戏目录短名（悬停显示完整路径）、引擎名称和游戏标题
//! - **未选择游戏**：显示"未选择游戏目录"提示文本

use crate::theme::{self, colors};
use egui::Ui;
use game_tool_core::detector::EngineType;

/// 渲染顶部栏
///
/// # 参数
/// - `has_game`: 是否已加载游戏目录
/// - `game_title`: 游戏标题，从存档中解析得到
/// - `engine`: 检测到的游戏引擎类型
/// - `game_dir`: 游戏目录路径（用于显示短名称和悬停提示完整路径）
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

        // ── 游戏已加载状态 ──
        if has_game {
            // 游戏目录短名 + 悬停显示完整路径
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
            // 引擎名称（经过本地化映射）
            let ename = theme::engine_display_name(engine);
            ui.label(format!("\u{5f15}\u{64ce}: {}", ename));
            // 游戏标题（如果存档中有解析到）
            if !game_title.is_empty() {
                ui.separator();
                ui.label(format!("\u{6807}\u{9898}: {}", game_title));
            }
        // ── 未加载游戏状态 ──
        } else {
            ui.colored_label(
                colors::TEXT_SECONDARY,
                "\u{672a}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}",
            );
        }
    });
}
