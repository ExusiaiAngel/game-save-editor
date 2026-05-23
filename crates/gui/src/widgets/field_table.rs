use egui::{Color32, ScrollArea, Ui};
use game_tool_core::ModifiableField;
use serde_json::Value;

pub fn render(
    ui: &mut Ui,
    fields: &mut [ModifiableField],
    readonly: bool,
    search_query: &str,
    selected_category: &Option<String>,
    jump_id: &mut String,
) -> usize {
    // Count dirty fields across ALL fields (not just visible page)
    let dirty_count = fields.iter().filter(|f| f.dirty).count();

    let all_indices: Vec<usize> = fields
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            let (cat_filter, range) = crate::widgets::category_tree::parse_range(selected_category);
            if let Some(cat) = cat_filter {
                if f.category != cat {
                    return false;
                }
                if let Some((rstart, rend)) = range {
                    let idx = f.item_id as usize;
                    if idx < rstart || idx > rend {
                        return false;
                    }
                }
            }
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

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("field_grid")
                .striped(true)
                .min_col_width(40.0)
                .show(ui, |ui| {
                    ui.strong("分类");
                    ui.strong("名称");
                    ui.strong("值");
                    ui.strong("实时");
                    ui.label("");
                    ui.end_row();

                    for &idx in &all_indices {
                        let cat = fields[idx].category.clone();
                        let dname = fields[idx].display_name.clone();
                        let is_jump_target = jump_target.as_deref() == Some(&fields[idx].field_id);
                        let live_val = fields[idx].live_value.clone();
                        let save_val = fields[idx].save_value.clone();
                        let dirty = fields[idx].dirty;

                        if is_jump_target {
                            ui.colored_label(Color32::from_rgb(100, 200, 255), &cat);
                            ui.colored_label(Color32::from_rgb(100, 200, 255), &dname);
                        } else {
                            ui.label(&cat);
                            ui.label(&dname);
                        }

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

                        let live_display = value_display(&live_val);
                        let is_diff = live_val != save_val;
                        if !live_display.is_empty() && live_display != "-" {
                            if is_diff {
                                ui.colored_label(Color32::from_rgb(255, 200, 0), &live_display);
                            } else {
                                ui.colored_label(Color32::from_rgb(139, 148, 158), &live_display);
                            }
                        } else {
                            ui.colored_label(Color32::from_rgb(72, 79, 88), "-");
                        }

                        if dirty {
                            ui.colored_label(Color32::from_rgb(255, 200, 0), "*");
                        } else {
                            ui.label("");
                        }

                        ui.end_row();
                    }
                });

            ui.label(format!("共 {} 项", total));
        });

    dirty_count
}

fn value_display(v: &Value) -> String {
    match v {
        Value::Null => "-".into(),
        Value::Bool(b) => if *b { "ON" } else { "OFF" }.into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

pub enum FieldSource {
    Save,
    Live,
}

pub fn render_field_editor(
    ui: &mut Ui,
    field: &ModifiableField,
    source: FieldSource,
) -> Option<Value> {
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
        assert_eq!(value_display(&Value::Number(9999999.into())), "9999999");
    }

    #[test]
    fn test_value_display_string() {
        assert_eq!(value_display(&Value::String("hello".into())), "hello");
        assert_eq!(value_display(&Value::String("测试".into())), "测试");
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
}
