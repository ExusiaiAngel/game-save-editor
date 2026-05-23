//! 标签栏面板，提供功能标签页的切换和游戏目录切换入口。
//!
//! 标签栏位于顶部栏下方、中央面板上方，以横向标签页形式展示所有可用功能。
//! 当前选中标签下方会绘制强调色下划线；禁用标签悬停时显示原因 tooltip。
//! 右侧固定显示"切换游戏"按钮，可重新选择游戏目录。

use crate::state::TabMode;
use crate::theme;
use egui::{vec2, Ui};

/// 标签栏产生的操作
pub enum TabAction {
    /// 切换到指定标签页
    SwitchTab(TabMode),
    /// 切换游戏目录（弹出目录选择对话框）
    SwitchGame,
}

/// 渲染标签栏，显示所有功能标签页和切换游戏按钮
///
/// 标签启用逻辑：
/// - **存档编辑、备份管理**：需要已加载游戏（has_game）
/// - **实时编辑**：需要已加载游戏且引擎支持实时修改（has_game && supports_rt）
/// - **工具箱、设置**：始终可用
///
/// 被禁用的标签会显示悬停提示（tooltip）说明原因。
/// 当前选中标签下方会绘制强调色下划线。
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
            // 标签启用/禁用逻辑
            let enabled = match tab {
                TabMode::SaveEditor | TabMode::BackupManager => has_game,
                TabMode::RealtimeEditor => has_game && supports_rt,
                TabMode::Toolbox | TabMode::Settings => true,
            };

            let label = format!("{} {}", theme::tab_icon(tab), theme::tab_name(tab));

            let resp = ui
                .add_enabled_ui(enabled, |ui| ui.selectable_label(selected, label))
                .inner;

            // 选中标签下方绘制强调色下划线
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

            // 禁用标签显示理由 tooltip
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

        // 右侧对齐的"切换游戏"按钮
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("\u{5207}\u{6362}\u{6e38}\u{620f}...").clicked() {
                actions.push(TabAction::SwitchGame);
            }
        });
    });

    actions
}
