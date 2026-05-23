//! 可编辑字段表格组件，是存档编辑器的核心渲染组件。
//!
//! 支持分类筛选、文本搜索、ID跳转、实时值对比等功能。

use crate::theme::colors;
use egui::{ScrollArea, Ui};
use game_tool_core::ModifiableField;
use serde_json::Value;
use std::collections::HashMap;

/// 渲染可编辑字段表格，是存单编辑器的核心组件
///
/// # 功能
/// - **过滤**：支持按分类筛选（含子范围）和搜索文本匹配（搜索 display_name 和 field_id）
/// - **跳转**：输入字段ID后自动滚动到目标行并以强调色高亮该行
/// - **实时值列**：当实时编辑器连接时（live_fields 非空），额外显示一列实时值，
///   实时值与存档值不同的行以警告色标注
/// - **编辑**：根据字段类型（bool/int/float/string）渲染对应的编辑控件
///
/// # live_map 机制
/// 将 `live_fields` 转换为 `HashMap<field_id, &ModifiableField>` 用于快速查找，
/// 判断每个存档字段是否有对应的实时值，以及二者是否相同。
///
/// # 返回值
/// 返回当前所有脏字段（dirty=true）的数量
pub fn render(
    ui: &mut Ui,
    fields: &mut [ModifiableField],
    readonly: bool,
    search_query: &str,
    selected_category: &Option<String>,
    jump_id: &mut String,
    live_fields: Option<&[ModifiableField]>,
) -> usize {
    // 统计所有字段中的脏字段数（非仅当前显示页）
    let dirty_count = fields.iter().filter(|f| f.dirty).count();

    // 构建实时字段的快速查找表（field_id -> &ModifiableField）
    let live_map: HashMap<&str, &ModifiableField> = live_fields
        .map(|lf| lf.iter().map(|f| (f.field_id.as_str(), f)).collect())
        .unwrap_or_default();
    // 是否有实时数据可显示
    let show_live_col = !live_map.is_empty();

    // 收集所有满足过滤条件的字段索引
    let all_indices: Vec<usize> = fields
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            // 解析分类+范围过滤条件
            let (cat_filter, range) =
                crate::widgets::category_tree::parse_range(selected_category);
            if let Some(cat) = cat_filter {
                if f.category != cat {
                    return false;
                }
                // 子范围过滤（如 "switch:0-99"）
                if let Some((rstart, rend)) = range {
                    let idx = f.item_id as usize;
                    if idx < rstart || idx > rend {
                        return false;
                    }
                }
            }
            // 文本搜索（匹配 display_name 或 field_id）
            if !search_query.is_empty() {
                let q = search_query.to_lowercase();
                return f.display_name.to_lowercase().contains(&q)
                    || f.field_id.to_lowercase().contains(&q);
            }
            true
        })
        .map(|(i, _)| i)
        .collect();

    let total = all_indices.len();

    // 跳转目标解析：消费 jump_id，在过滤后的索引中找到目标字段
    let jump_target = if !jump_id.is_empty() {
        let target = jump_id.clone();
        jump_id.clear();
        all_indices
            .iter()
            .find(|&&i| fields[i].field_id == target)
            .map(|_| target)
    } else {
        None
    };

    // 使用 ScrollArea 包裹表格，支持大量字段时的滚动
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // 使用 Grid 布局，带隔行变色
            egui::Grid::new("field_grid")
                .striped(true)
                .min_col_width(40.0)
                .show(ui, |ui| {
                    // ── 表头 ──
                    ui.strong("\u{5206}\u{7c7b}");
                    ui.strong("\u{540d}\u{79f0}");
                    ui.strong("\u{4fdd}\u{5b58}\u{503c}");
                    if show_live_col {
                        ui.strong("\u{5b9e}\u{65f6}\u{503c}");
                    }
                    ui.strong("\u{72b6}\u{6001}");
                    ui.end_row();

                    // ── 数据行 ──
                    for &idx in &all_indices {
                        let cat = fields[idx].category.clone();
                        let dname = fields[idx].display_name.clone();
                        let is_jump_target =
                            jump_target.as_deref() == Some(&fields[idx].field_id);

                        // 跳转目标：自动滚动到可见区域
                        if is_jump_target {
                            ui.scroll_to_cursor(Some(egui::Align::Center));
                        }

                        let save_val = fields[idx].save_value.clone();
                        let dirty = fields[idx].dirty;

                        // 分类和名称列（跳转目标以强调色高亮）
                        if is_jump_target {
                            ui.colored_label(colors::ACCENT, &cat);
                            ui.colored_label(colors::ACCENT, &dname);
                        } else {
                            ui.label(&cat);
                            ui.label(&dname);
                        }

                        // 存档值列：只读模式下仅显示文本，否则渲染编辑器
                        if readonly {
                            let ds = value_display(&save_val);
                            ui.label(&ds);
                        } else {
                            if let Some(new_val) =
                                render_field_editor(ui, &fields[idx], FieldSource::Save)
                            {
                                fields[idx].save_value = new_val;
                                fields[idx].dirty = true;
                            }
                        }

                        // 实时值列（仅在实时编辑器连接时显示）
                        if show_live_col {
                            let live_from_conn = live_map.get(fields[idx].field_id.as_str());
                            if let Some(lf) = live_from_conn {
                                let live_display = value_display(&lf.live_value);
                                let is_diff = lf.live_value != save_val;
                                if !live_display.is_empty() && live_display != "-" {
                                    // 实时值与存档值不同：警告色高亮
                                    if is_diff {
                                        ui.colored_label(
                                            colors::WARNING,
                                            &live_display,
                                        );
                                    // 相同：次要文本色
                                    } else {
                                        ui.colored_label(
                                            colors::TEXT_SECONDARY,
                                            &live_display,
                                        );
                                    }
                                } else {
                                    ui.colored_label(colors::TEXT_DISABLED, "-");
                                }
                            } else {
                                // 实时数据中无此字段
                                ui.colored_label(colors::TEXT_DISABLED, "-");
                            }
                        }

                        // 状态列：脏标记（*）和差异标记（←）
                        let mut status_parts: Vec<String> = Vec::new();
                        if dirty {
                            status_parts.push("*".into());
                        }
                        if show_live_col {
                            if let Some(lf) = live_map.get(fields[idx].field_id.as_str()) {
                                if lf.live_value != save_val {
                                    status_parts.push("\u{2190}".into());
                                }
                            }
                        }

                        let status_str = status_parts.join("");
                        if status_str.is_empty() {
                            ui.label("");
                        } else {
                            ui.colored_label(colors::WARNING, &status_str);
                        }

                        ui.end_row();
                    }
                });

            // 底部计数和空白状态
            if total == 0 && (!search_query.is_empty() || selected_category.is_some()) {
                ui.colored_label(colors::TEXT_SECONDARY, "\u{672a}\u{627e}\u{5230}\u{5339}\u{914d}\u{5b57}\u{6bb5}");
            } else {
                ui.label(format!("\u{5171} {} \u{9879}", total));
            }
        });

    dirty_count
}

/// 将 JSON `Value` 转换为可读的显示字符串
///
/// - `Null` → "-"
/// - `Bool(true)` → "ON" / `Bool(false)` → "OFF"
/// - `Number` → 数字字符串
/// - `String` → 原字符串
/// - `Array/Object` 等复杂类型 → JSON 格式化字符串
pub fn value_display(v: &Value) -> String {
    match v {
        Value::Null => "-".into(),
        Value::Bool(b) => if *b { "ON" } else { "OFF" }.into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

/// 字段编辑器操作的数值来源
pub enum FieldSource {
    /// 编辑存档中的值（存单编辑面板）
    Save,
    /// 编辑游戏内存中的实时值（实时编辑面板）
    Live,
}

/// 根据字段类型渲染对应的编辑器控件
///
/// - `bool` → 复选框
/// - `int` → 整数拖拽输入框，范围受 `min_val`/`max_val` 约束
/// - `float` → 浮点拖拽输入框（速度 0.1）
/// - 其他（string） → 单行文本编辑框
///
/// # 返回值
/// 如果用户修改了值则返回 `Some(新值)`，否则返回 `None`
pub fn render_field_editor(
    ui: &mut Ui,
    field: &ModifiableField,
    source: FieldSource,
) -> Option<Value> {
    // 根据 FieldSource 选择要编辑的值
    let val = match source {
        FieldSource::Save => &field.save_value,
        FieldSource::Live => &field.live_value,
    };

    match field.field_type.as_str() {
        "bool" => {
            let mut v = val.as_bool().unwrap_or(false);
            if ui.checkbox(&mut v, "").changed() {
                Some(Value::Bool(v))
            } else {
                None
            }
        }
        "int" => {
            let mut v = val.as_i64().unwrap_or(0) as i32;
            // 构建数值范围（处理 min > max 的边界情况）
            let range = (field.min_val.min(field.max_val) as f64)
                ..=(field.max_val.max(field.min_val) as f64);
            if ui
                .add(egui::DragValue::new(&mut v).range(range).speed(1))
                .changed()
            {
                Some(Value::Number(v.into()))
            } else {
                None
            }
        }
        "float" => {
            let mut v = val.as_f64().unwrap_or(0.0);
            if ui.add(egui::DragValue::new(&mut v).speed(0.1)).changed() {
                serde_json::Number::from_f64(v).map(Value::Number)
            } else {
                None
            }
        }
        // 默认使用字符串编辑器（兼容未知类型）
        _ => {
            let mut v = val.as_str().unwrap_or("").to_string();
            if ui.text_edit_singleline(&mut v).changed() {
                Some(Value::String(v))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_value_display_null() {
        assert_eq!(value_display(&Value::Null), "-");
    }

    #[test]
    fn test_value_display_bool_true() {
        assert_eq!(value_display(&Value::Bool(true)), "ON");
    }

    #[test]
    fn test_value_display_bool_false() {
        assert_eq!(value_display(&Value::Bool(false)), "OFF");
    }

    #[test]
    fn test_value_display_number() {
        assert_eq!(value_display(&Value::Number(42.into())), "42");
        assert_eq!(
            value_display(&Value::Number(9999999.into())),
            "9999999"
        );
    }

    #[test]
    fn test_value_display_string() {
        assert_eq!(value_display(&Value::String("hello".into())), "hello");
        assert_eq!(value_display(&Value::String("\u{6d4b}\u{8bd5}".into())), "\u{6d4b}\u{8bd5}");
    }

    #[test]
    fn test_value_display_array() {
        let arr = Value::Array(vec![Value::Number(1.into()), Value::Number(2.into())]);
        let ds = value_display(&arr);
        assert!(ds.contains('['));
    }

    #[test]
    fn test_value_display_object() {
        let mut m = serde_json::Map::new();
        m.insert("key".into(), Value::String("val".into()));
        let obj = Value::Object(m);
        let ds = value_display(&obj);
        assert!(ds.contains('{'));
    }

    #[test]
    fn test_field_source_variants() {
        let save = FieldSource::Save;
        let live = FieldSource::Live;
        let _ = match save {
            FieldSource::Save => true,
            FieldSource::Live => false,
        };
        let _ = match live {
            FieldSource::Save => false,
            FieldSource::Live => true,
        };
    }

    #[test]
    fn test_live_map_empty_when_none() {
        let map: HashMap<&str, &ModifiableField> = None
            .map(|lf: &[ModifiableField]| lf.iter().map(|f| (f.field_id.as_str(), f)).collect())
            .unwrap_or_default();
        assert!(map.is_empty());
    }

    #[test]
    fn test_live_map_populated() {
        let fields = vec![
            ModifiableField {
                field_id: "gold".into(),
                live_value: Value::Number(5000.into()),
                ..Default::default()
            },
            ModifiableField {
                field_id: "switch_1".into(),
                live_value: Value::Bool(true),
                ..Default::default()
            },
        ];
        let map: HashMap<&str, &ModifiableField> = Some(fields.as_slice())
            .map(|lf| lf.iter().map(|f| (f.field_id.as_str(), f)).collect())
            .unwrap_or_default();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("gold").unwrap().live_value, Value::Number(5000.into()));
    }
}
