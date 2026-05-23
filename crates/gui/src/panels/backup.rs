//! 备份管理面板，用于创建、恢复、删除存档的备份文件。

use crate::state::AppState;
use crate::theme::colors;
use egui::Ui;

/// 备份管理面板产生的操作
pub enum BackupAction {
    /// 恢复指定索引的备份（覆盖当前存档）
    Restore(usize),
    /// 删除指定索引的备份文件
    Delete(usize),
    /// 创建当前存档的新备份
    CreateBackup,
    /// 批量删除选中的备份
    BatchDelete(Vec<usize>),
}

/// 渲染备份管理面板
///
/// # 功能区域
/// 1. **当前存档显示** — 显示存档编辑器中选中的存档文件名
/// 2. **创建备份按钮** — 需先加载存档数据后才可用
/// 3. **备份列表** — 每行：复选框、文件名、文件大小、恢复/删除按钮
/// 4. **批量操作** — 选中多个备份后可一次性批量删除
///
/// # 复选框选择逻辑
/// - 使用 `backup_selection`（BTreeSet）追踪已选备份的索引
/// - 勾选时插入索引，取消勾选时移除索引
/// - 批量删除操作会 `drain()` 清空选择集合
pub fn render(ui: &mut Ui, state: &mut AppState) -> Vec<BackupAction> {
    let mut actions = Vec::new();

    ui.heading("\u{1f5c4} \u{5907}\u{4efd}\u{7ba1}\u{7406}");
    ui.add_space(8.0);

    // 未选择游戏目录时的空状态
    if state.game_dir.is_none() {
        ui.colored_label(
            colors::TEXT_SECONDARY,
            "\u{8bf7}\u{5148}\u{9009}\u{62e9}\u{6e38}\u{620f}\u{76ee}\u{5f55}\u{3002}",
        );
        return actions;
    }

    // ── 当前存档显示 ──
    ui.horizontal(|ui| {
        let current = state
            .save_panel
            .selected_save
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("\u{672a}\u{9009}\u{62e9}\u{5b58}\u{6863}");
        ui.label(format!("\u{5f53}\u{524d}\u{5b58}\u{6863}: {}", current));
    });

    ui.add_space(8.0);

    // ── 创建备份：需要已选择存档且数据已加载 ──
    if state.save_panel.selected_save.is_some() && state.save_panel.save_data.is_some() {
        if ui
            .button("\u{1f4e6} \u{521b}\u{5efa}\u{5907}\u{4efd}")
            .clicked()
        {
            actions.push(BackupAction::CreateBackup);
        }
    } else {
        ui.colored_label(
            colors::TEXT_DISABLED,
            "\u{8bf7}\u{5148}\u{5728}\u{5b58}\u{6863}\u{7f16}\u{8f91}\u{4e2d}\u{52a0}\u{8f7d}\u{4e00}\u{4e2a}\u{5b58}\u{6863}\u{3002}",
        );
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // ── 备份列表 ──
    if state.backup_paths.is_empty() {
        // 空状态：没有备份文件
        ui.colored_label(
            colors::TEXT_SECONDARY,
            "\u{672a}\u{53d1}\u{73b0}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{3002}\u{52a0}\u{8f7d}\u{5b58}\u{6863}\u{540e}\u{5907}\u{4efd}\u{6587}\u{4ef6}\u{5c06}\u{663e}\u{793a}\u{5728}\u{6b64}\u{5904}\u{3002}",
        );
    } else {
        ui.label(format!(
            "\u{5171} {} \u{4e2a}\u{5907}\u{4efd}:",
            state.backup_paths.len()
        ));
        ui.add_space(4.0);

        // 使用网格布局渲染备份列表（隔行变色）
        egui::Grid::new("backup_grid")
            .striped(true)
            .min_col_width(20.0)
            .show(ui, |ui| {
                for (i, bp) in state.backup_paths.iter().enumerate() {
                    // 显示文件名（短名）
                    let name = std::path::Path::new(bp)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(bp);

                    // 获取文件大小并格式化（KB 或 B）
                    let size = std::fs::metadata(bp).map(|m| m.len()).unwrap_or(0);
                    let size_str = if size > 1024 {
                        format!("{} KB", size / 1024)
                    } else {
                        format!("{} B", size)
                    };

                    // 复选框：管理选中/取消选中状态
                    let mut selected = state.backup_selection.contains(&i);
                    if ui.checkbox(&mut selected, "").changed() {
                        if selected {
                            state.backup_selection.insert(i);
                        } else {
                            state.backup_selection.remove(&i);
                        }
                    }
                    ui.label(format!("\u{1f4c4} {}", name));
                    ui.colored_label(colors::TEXT_SECONDARY, size_str);

                    // 单行操作按钮
                    if ui.button("\u{267b} \u{6062}\u{590d}").clicked() {
                        actions.push(BackupAction::Restore(i));
                    }
                    if ui.button("\u{1f5d1} \u{5220}\u{9664}").clicked() {
                        actions.push(BackupAction::Delete(i));
                    }
                    ui.end_row();
                }
            });

        // ── 批量操作栏 ──
        let sel_count = state.backup_selection.len();
        if sel_count > 0 {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(format!("\u{5df2}\u{9009} {} \u{9879}", sel_count));
                if ui.button("\u{1f5d1}\u{6279}\u{91cf}\u{5220}\u{9664}").clicked() {
                    let indices: Vec<usize> = state.backup_selection.drain().collect();
                    actions.push(BackupAction::BatchDelete(indices));
                }
            });
        }
    }

    actions
}
