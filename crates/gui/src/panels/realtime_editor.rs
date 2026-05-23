use crate::state::{ConnectionStatus, RtPanelState};
use crate::theme::colors;
use crate::widgets::category_tree;
use crate::widgets::field_table::{render_field_editor, FieldSource};
use egui::{Color32, ScrollArea, Ui};
use game_tool_core::ModifiableField;
use serde_json::Value;
use std::collections::HashMap;

pub enum RtAction {
    WriteField(String, Value),
    ToggleLock(String),
    CopyToSave(String),
}

pub fn render(
    ui: &mut Ui,
    rt_panel: &mut RtPanelState,
    save_fields: &[ModifiableField],
) -> Vec<RtAction> {
    let mut actions = Vec::new();

    let is_connected = rt_panel
        .conn
        .as_ref()
        .map(|c| c.status == ConnectionStatus::Connected)
        .unwrap_or(false);

    let save_map: HashMap<&str, &ModifiableField> = save_fields
        .iter()
        .map(|f| (f.field_id.as_str(), f))
        .collect();

    // ── Filter bar ──
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

    // ── Jump target resolution ──
    let mut jump_target = None;
    if !rt_panel.jump_id.is_empty() {
        let target = rt_panel.jump_id.clone();
        rt_panel.jump_id.clear();
        if rt_panel.fields.iter().any(|f| f.field_id == target) {
            jump_target = Some(target);
        }
    }

    // ── Filtered & grouped fields ──
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

                // Determine if this field has a corresponding save field with different value
                let live_differs =
                    save_val.map(|sv| *sv != field.live_value).unwrap_or(false);

                ui.horizontal(|ui| {
                    let mut locked_state = locked;
                    let lock_icon = if locked { "\u{1f512}" } else { "\u{1f513}" };
                    if ui.checkbox(&mut locked_state, lock_icon).changed() {
                        actions.push(RtAction::ToggleLock(field.field_id.clone()));
                    }

                    if is_jump {
                        ui.colored_label(Color32::from_rgb(100, 200, 255), &field.display_name);
                    } else {
                        ui.label(&field.display_name);
                    }

                    // Live value editor
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

                    // Save value (read-only display)
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
                        ui.colored_label(colors::TEXT_DISABLED, "-");
                    }

                    // Copy to save button
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
            ui.label(format!("\u{5171} {} \u{9879}", total));
        });

    actions
}
