//! 启动页面（首页），在没有加载游戏目录时显示的欢迎界面。

use crate::state::AppState;
use crate::theme;
use egui::{vec2, Ui};

/// 启动页面产生的操作
pub enum StartupAction {
    /// 打开游戏目录选择对话框
    OpenGameDir,
    /// 打开最近的游戏目录
    OpenRecentGame(String),
}

/// 渲染启动页面（首页），在没有加载游戏时显示
///
/// 布局：垂直居中显示应用标题、"打开游戏目录"按钮和最近游戏列表。
/// 如果有最近游戏记录，显示可点击的列表；否则显示"暂无最近游戏记录"提示。
pub fn render(ui: &mut Ui, state: &AppState) -> Vec<StartupAction> {
    let mut actions = Vec::new();

    // 计算垂直居中偏移量
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

        // 主操作按钮：打开游戏目录
        let btn = egui::Button::new("\u{1f4c2} \u{6253}\u{5f00}\u{6e38}\u{620f}\u{76ee}\u{5f55}...")
            .min_size(vec2(200.0, 40.0));
        if ui.add(btn).clicked() {
            actions.push(StartupAction::OpenGameDir);
        }

        ui.add_space(24.0);

        // ── 最近游戏列表 ──
        if !state.recent_games.is_empty() {
            ui.colored_label(theme::colors::TEXT_SECONDARY, "\u{6700}\u{8fd1}\u{6e38}\u{620f}:");
            ui.add_space(8.0);

            for game_path in &state.recent_games {
                // 显示目录短名而非完整路径
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
        // 空状态：没有最近游戏记录
        } else {
            ui.colored_label(
                theme::colors::TEXT_DISABLED,
                "\u{6682}\u{65e0}\u{6700}\u{8fd1}\u{6e38}\u{620f}\u{8bb0}\u{5f55}",
            );
        }
    });

    actions
}
