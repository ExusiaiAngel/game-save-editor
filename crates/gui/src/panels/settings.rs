//! 设置面板，提供应用外观、连接参数、历史记录等配置管理。

use crate::state::AppState;
use crate::theme::colors;
use egui::Ui;

/// 设置面板产生的操作
pub enum SettingsAction {
    /// 切换暗色/亮色主题
    ToggleDarkMode,
    /// 设置实时编辑器的默认端口号
    SetPort(u16),
    /// 移除单个最近游戏目录记录
    RemoveRecentGame(String),
    /// 清除所有最近游戏目录记录
    ClearRecentGames,
    /// 保存所有设置到配置文件
    SaveAll,
}

/// 渲染设置面板
///
/// # 设置区域（折叠面板）
/// 1. **外观** — 切换暗色/亮色主题模式，显示当前主题图标和文字
/// 2. **连接设置** — 设置实时编辑器的默认端口号（范围 1024-65535）
/// 3. **最近游戏** — 列出最近打开的游戏目录路径，支持单独移除或全部清除
/// 4. **配置** — 显示配置文件目录路径，支持一键在文件管理器中打开
/// 5. **关于** — 显示应用名称、简介和支持的引擎列表
pub fn render(ui: &mut Ui, state: &AppState) -> Vec<SettingsAction> {
    let mut actions = Vec::new();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("\u{2699} \u{8bbe}\u{7f6e}");
        ui.add_space(12.0);

        // ── 外观设置：主题模式切换 ──
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

        // ── 连接设置：实时编辑器端口 ──
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

        // ── 最近游戏记录管理 ──
        ui.collapsing("\u{6700}\u{8fd1}\u{6e38}\u{620f}", |ui| {
            if state.recent_games.is_empty() {
                // 空状态
                ui.colored_label(
                    colors::TEXT_SECONDARY,
                    "\u{6682}\u{65e0}\u{6700}\u{8fd1}\u{8bb0}\u{5f55}\u{3002}",
                );
            } else {
                // 逐条列出记录，每条带"× 移除"按钮
                for path in &state.recent_games {
                    ui.horizontal(|ui| {
                        ui.label(path);
                        if ui.button("\u{d7} \u{79fb}\u{9664}").clicked() {
                            actions.push(SettingsAction::RemoveRecentGame(path.clone()));
                        }
                    });
                }
            }
            // 全部清除按钮
            if !state.recent_games.is_empty() {
                ui.add_space(4.0);
                if ui.button("\u{1f5d1} \u{6e05}\u{9664}\u{5168}\u{90e8}\u{8bb0}\u{5f55}").clicked() {
                    actions.push(SettingsAction::ClearRecentGames);
                }
            }
        });

        ui.add_space(8.0);

        // ── 配置目录信息 ──
        ui.collapsing("\u{914d}\u{7f6e}", |ui| {
            if let Some(config_dir) = dirs_next::config_dir() {
                let config_path = config_dir.join("GameSaveEditor");
                ui.label(format!(
                    "\u{914d}\u{7f6e}\u{76ee}\u{5f55}: {}",
                    config_path.display()
                ));
                ui.horizontal(|ui| {
                    // 在系统文件管理器中打开配置目录
                    if ui.button("\u{1f4c2} \u{6253}\u{5f00}\u{76ee}\u{5f55}").clicked() {
                        let _ = open::that(&config_path);
                    }
                });
            } else {
                // 无法获取配置目录（极少数情况）
                ui.colored_label(
                    colors::TEXT_SECONDARY,
                    "\u{65e0}\u{6cd5}\u{83b7}\u{53d6}\u{914d}\u{7f6e}\u{76ee}\u{5f55}\u{3002}",
                );
            }
        });

        ui.add_space(8.0);

        // ── 关于信息 ──
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

    // ── 保存按钮 ──
    ui.add_space(12.0);
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("\u{1f4be} \u{4fdd}\u{5b58}\u{8bbe}\u{7f6e}").clicked() {
            actions.push(SettingsAction::SaveAll);
        }
    });

    actions
}
