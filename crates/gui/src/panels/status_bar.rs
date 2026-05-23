//! 状态栏面板，根据当前激活的标签页显示上下文相关状态信息。

use crate::state::{AppState, ConnectionStatus, TabMode};
use crate::theme;
use egui::Ui;

/// 渲染状态栏，根据当前激活的标签页显示不同信息
///
/// # 各标签页显示内容
/// - **存单编辑** — 未保存修改数量（高亮）、字段总数、当前存档文件名
/// - **实时编辑** — 连接状态（已连接/连接中/未连接，带彩色图标）、端口号、
///   锁定字段数量、差异项数量、字段总数
/// - **备份管理** — 备份文件总数、当前存档文件名
/// - **工具箱** — 提示"独立工具，无需加载游戏"
/// - **设置** — 提示"设置"
///
/// 所有标签页：如果有全局状态消息（如错误/成功信息），在末尾追加显示，
/// 含"失败"的消息以红色高亮。
pub fn render(ui: &mut Ui, state: &AppState) {
    ui.horizontal(|ui| {
        match state.active_tab {
            // ── 存单编辑状态栏 ──
            TabMode::SaveEditor => {
                let dirty = state.save_panel.dirty_count;
                if dirty > 0 {
                    ui.colored_label(
                        theme::colors::WARNING,
                        format!("\u{4fee}\u{6539} {} \u{5904}\u{672a}\u{4fdd}\u{5b58}", dirty),
                    );
                } else {
                    ui.label("\u{65e0}\u{672a}\u{4fdd}\u{5b58}\u{4fee}\u{6539}");
                }
                ui.separator();
                let field_count = state.save_panel.fields.len();
                ui.label(format!("\u{5171} {} \u{4e2a}\u{5b57}\u{6bb5}", field_count));
                // 显示当前打开的存档文件名
                if let Some(ref path) = state.save_panel.selected_save {
                    if let Some(name) = std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                    {
                        ui.colored_label(theme::colors::TEXT_SECONDARY, format!("|  {}", name));
                    }
                }
            }
            // ── 实时编辑状态栏 ──
            TabMode::RealtimeEditor => {
                // 连接状态（带颜色图标）
                let status = state
                    .rt_panel
                    .conn
                    .as_ref()
                    .map(|c| c.status)
                    .unwrap_or(ConnectionStatus::Disconnected);
                let (icon, icon_color, label) = match status {
                    ConnectionStatus::Connected => {
                        ("\u{25cf}", theme::colors::SUCCESS, "\u{5df2}\u{8fde}\u{63a5}")
                    }
                    ConnectionStatus::Connecting => {
                        ("\u{25cc}", theme::colors::WARNING, "\u{8fde}\u{63a5}\u{4e2d}...")
                    }
                    ConnectionStatus::Disconnected => (
                        "\u{25cb}",
                        theme::colors::TEXT_DISABLED,
                        "\u{672a}\u{8fde}\u{63a5}",
                    ),
                };
                ui.colored_label(icon_color, format!("{} {}", icon, label));
                ui.colored_label(
                    theme::colors::TEXT_SECONDARY,
                    format!(":{}", state.rt_panel.port),
                );

                // 锁定字段数量
                let locked = state.rt_panel.locked_fields.len();
                if locked > 0 {
                    ui.colored_label(
                        theme::colors::WARNING,
                        format!("\u{1f512} {} \u{4e2a}\u{9501}\u{5b9a}", locked),
                    );
                }

                // 差异项数量（实时值与存档值不同的字段）
                let diff_count = state.rt_panel.fields.iter().filter(|lf| {
                    state.save_panel.fields.iter().any(|sf| {
                        sf.field_id == lf.field_id && sf.save_value != lf.live_value
                    })
                }).count();
                if diff_count > 0 {
                    ui.colored_label(
                        theme::colors::WARNING,
                        format!("\u{5dee}\u{5f02} {}\u{9879}", diff_count),
                    );
                }

                ui.separator();
                let count = state.rt_panel.fields.len();
                ui.label(format!("\u{5171} {} \u{4e2a}\u{5b57}\u{6bb5}", count));
            }
            // ── 备份管理状态栏 ──
            TabMode::BackupManager => {
                let count = state.backup_paths.len();
                ui.label(format!("\u{5171} {} \u{4e2a}\u{5907}\u{4efd}", count));
                if let Some(ref path) = state.save_panel.selected_save {
                    if let Some(name) = std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                    {
                        ui.colored_label(
                            theme::colors::TEXT_SECONDARY,
                            format!("| \u{5f53}\u{524d}: {}", name),
                        );
                    }
                }
            }
            // ── 工具箱状态栏 ──
            TabMode::Toolbox => {
                ui.colored_label(
                    theme::colors::TEXT_SECONDARY,
                    "\u{5de5}\u{5177}\u{7bb1} \u{2014} \u{72ec}\u{7acb}\u{5de5}\u{5177}\u{ff0c}\u{65e0}\u{9700}\u{52a0}\u{8f7d}\u{6e38}\u{620f}",
                );
            }
            // ── 设置状态栏 ──
            TabMode::Settings => {
                ui.colored_label(theme::colors::TEXT_SECONDARY, "\u{8bbe}\u{7f6e}");
            }
        }

        // 全局状态消息（各标签通用）
        if !state.status_message.is_empty() {
            ui.separator();
            let is_error = state.status_message.contains("\u{5931}\u{8d25}");
            if is_error {
                ui.colored_label(theme::colors::ERROR, &state.status_message);
            } else {
                ui.label(&state.status_message);
            }
        }
    });
}
