//! 实时编辑面板，用于连接运行中的游戏进程并实时修改内存数值。

use crate::state::{ConnectionStatus, RtPanelState};
use crate::theme::colors;
use crate::widgets::category_tree;
use crate::widgets::field_table::{render_field_editor, FieldSource};
use egui::{Color32, ScrollArea, Ui};
use game_tool_core::ModifiableField;
use serde_json::Value;
use std::collections::HashMap;

/// 实时编辑面板产生的操作
pub enum RtAction {
    /// 向游戏进程写入指定字段的新值
    WriteField(String, Value),
    /// 切换字段的锁定状态（锁定后的字段不会被实时值覆盖，防止误操作）
    ToggleLock(String),
    /// 将实时内存值复制到存档（同步到存档编辑器）
    CopyToSave(String),
}

/// 渲染实时编辑面板，用于修改运行中游戏的内存值
///
/// # 功能区域
/// 1. **过滤栏** — 横向分类筛选 + 搜索栏 + 跳转ID输入框
/// 2. **字段列表** — 每行包含：锁定开关、字段名、实时值编辑器、存档值对比、差异状态
///
/// # 核心逻辑
/// - 通过 `save_map`（从存档字段构建的 HashMap）与实时字段进行交叉匹配
/// - 锁定字段显示只读的实时值，且不可通过编辑器修改（防止误触发写入）
/// - 与存档值不同的实时值以警告色高亮，并提供"→存档"按钮将实时值复制回存档
/// - 跳转目标字段以蓝色高亮显示
pub fn render(
    ui: &mut Ui,
    rt_panel: &mut RtPanelState,
    save_fields: &[ModifiableField],
) -> Vec<RtAction> {
    let mut actions = Vec::new();

    // 判断当前是否已连接到游戏进程
    let is_connected = rt_panel
        .conn
        .as_ref()
        .map(|c| c.status == ConnectionStatus::Connected)
        .unwrap_or(false);

    // 构建存档字段 ID -> 字段 的快速查找表
    let save_map: HashMap<&str, &ModifiableField> = save_fields
        .iter()
        .map(|f| (f.field_id.as_str(), f))
        .collect();

    // ── 1. 过滤栏：分类筛选 + 搜索 + 跳转 ──
    category_tree::render_horizontal(ui, &rt_panel.fields, &mut rt_panel.selected_category);
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        crate::widgets::search_bar::render(ui, &mut rt_panel.search_query);

        ui.label("ID:");
        ui.add(
            egui::TextEdit::singleline(&mut rt_panel.jump_id)
                .desired_width(80.0)
                .hint_text("\u{8df3}\u{8f6c}..."),
        );
    });

    ui.add_space(6.0);

    // ── 2. 跳转目标解析：如果输入了跳转ID且字段存在则记录跳转目标 ──
    let mut jump_target = None;
    if !rt_panel.jump_id.is_empty() {
        let target = rt_panel.jump_id.clone();
        rt_panel.jump_id.clear(); // 消费输入，防止重复跳转
        if rt_panel.fields.iter().any(|f| f.field_id == target) {
            jump_target = Some(target);
        }
    }

    // ── 3. 过滤并收集符合条件的字段索引 ──
    let search_query = rt_panel.search_query.to_lowercase();
    let sel_cat = rt_panel.selected_category.clone();
    let filtered_indices: Vec<usize> = rt_panel
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            if let Some(ref cat) = sel_cat {
                if f.category != *cat { return false; }
            }
            if !search_query.is_empty() {
                f.display_name.to_lowercase().contains(&search_query)
                    || f.field_id.to_lowercase().contains(&search_query)
            } else {
                true
            }
        })
        .map(|(i, _)| i)
        .collect();

    let total = filtered_indices.len();

    // ── 4. 渲染字段列表 ──
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for &idx in &filtered_indices {
                let field = &rt_panel.fields[idx];
                let is_jump = jump_target.as_deref() == Some(&field.field_id);
                let locked = rt_panel.locked_fields.contains(&field.field_id);
                let save_val = save_map
                    .get(field.field_id.as_str())
                    .map(|sf| &sf.save_value);

                // 判断实时值与存档值是否不同
                let live_differs =
                    save_val.map(|sv| *sv != field.live_value).unwrap_or(false);

                ui.horizontal(|ui| {
                    // 锁定/解锁开关
                    let mut locked_state = locked;
                    let lock_icon = if locked { "\u{1f512}" } else { "\u{1f513}" };
                    if ui.checkbox(&mut locked_state, lock_icon).changed() {
                        actions.push(RtAction::ToggleLock(field.field_id.clone()));
                    }

                    // 字段名（跳转目标以蓝色高亮）
                    if is_jump {
                        ui.colored_label(Color32::from_rgb(100, 200, 255), &field.display_name);
                    } else {
                        ui.label(&field.display_name);
                    }

                    // 实时值编辑器：已连接且未锁定时可编辑，否则只读显示
                    if is_connected && !locked {
                        if let Some(new_val) = render_field_editor(
                            ui,
                            &rt_panel.fields[idx],
                            FieldSource::Live,
                        ) {
                            actions.push(RtAction::WriteField(
                                field.field_id.clone(),
                                new_val,
                            ));
                        }
                    } else {
                        let disabled_val = crate::widgets::field_table::value_display(
                            &rt_panel.fields[idx].live_value,
                        );
                        ui.add(
                            egui::Label::new(&disabled_val)
                                .selectable(false),
                        );
                    }

                    // 存档值（只读对比显示）：差异项以警告色高亮，相同项以次要文本色显示
                    if let Some(sv) = save_val {
                        if live_differs {
                            ui.colored_label(
                                colors::WARNING,
                                &crate::widgets::field_table::value_display(sv),
                            );
                        } else {
                            ui.colored_label(
                                colors::TEXT_SECONDARY,
                                &crate::widgets::field_table::value_display(sv),
                            );
                        }
                    } else {
                        // 存档中不存在此字段
                        ui.colored_label(colors::TEXT_DISABLED, "-");
                    }

                    // "→存档"按钮：将实时值复制到存档字段
                    if live_differs {
                        if ui.button("\u{1f4e4}\u{2192}\u{5b58}\u{6863}").clicked() {
                            actions.push(RtAction::CopyToSave(
                                field.field_id.clone(),
                            ));
                        }
                    }
                });
            }

            ui.add_space(4.0);
            // 底部显示过滤后总条数
            ui.label(format!("\u{5171} {} \u{9879}", total));
        });

    actions
}
