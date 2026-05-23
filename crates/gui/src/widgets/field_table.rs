use egui::{Color32, ScrollArea, Ui};
use game_tool_core::ModifiableField;
use serde_json::Value;
use std::collections::HashMap;

pub fn render(
    ui: &mut Ui,
    fields: &mut [ModifiableField],
    readonly: bool,
    search_query: &str,
    selected_category: &Option<String>,
    jump_id: &mut String,
    live_fields: Option<&[ModifiableField]>,
) -> usize {
    // Count dirty fields across ALL fields (not just visible page)
    let dirty_count = fields.iter().filter(|f| f.dirty).count();

    let live_map: HashMap<&str, &ModifiableField> = live_fields
        .map(|lf| lf.iter().map(|f| (f.field_id.as_str(), f)).collect())
        .unwrap_or_default();
    let show_live_col = !live_map.is_empty();

    let all_indices: Vec<usize> = fields
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            let (cat_filter, range) =
                crate::widgets::category_tree::parse_range(selected_category);
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
                    ui.strong("\u{5206}\u{7c7b}");
                    ui.strong("\u{540d}\u{79f0}");
                    ui.strong("\u{4fdd}\u{5b58}\u{503c}");
                    if show_live_col {
                        ui.strong("\u{5b9e}\u{65f6}\u{503c}");
                    }
                    ui.strong("\u{72b6}\u{6001}");
                    ui.end_row();

                    for &idx in &all_indices {
                        let cat = fields[idx].category.clone();
                        let dname = fields[idx].display_name.clone();
                        let is_jump_target =
                            jump_target.as_deref() == Some(&fields[idx].field_id);

                        if is_jump_target {
                            ui.scroll_to_cursor(Some(egui::Align::Center));
                        }

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

                        if show_live_col {
                            let live_from_conn = live_map.get(fields[idx].field_id.as_str());
                            if let Some(lf) = live_from_conn {
                                let live_display = value_display(&lf.live_value);
                                let is_diff = lf.live_value != save_val;
                                if !live_display.is_empty() && live_display != "-" {
                                    if is_diff {
                                        ui.colored_label(
                                            Color32::from_rgb(210, 153, 34),
                                            &live_display,
                                        );
                                    } else {
                                        ui.colored_label(
                                            Color32::from_rgb(139, 148, 158),
                                            &live_display,
                                        );
                                    }
                                } else {
                                    ui.colored_label(Color32::from_rgb(72, 79, 88), "-");
                                }
                            } else {
                                ui.colored_label(Color32::from_rgb(72, 79, 88), "-");
                            }
                        }

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
                            ui.colored_label(Color32::from_rgb(210, 153, 34), &status_str);
                        }

                        ui.end_row();
                    }
                });

            ui.label(format!("\u{5171} {} \u{9879}", total));
        });

    dirty_count
}

pub fn value_display(v: &Value) -> String {
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
