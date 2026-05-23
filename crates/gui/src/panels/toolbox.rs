//! 工具箱面板，提供独立于游戏的实用工具集合。
//!
//! # 工具列表
//! 1. **LZString 压缩/解压** — 处理 RPG Maker MV 存档的 LZString + Base64 格式
//! 2. **Base64 编解码** — 通用 Base64 编码/解码工具
//! 3. **存档信息查看器** — 快速查看存档文件格式、大小、修改时间等元信息
//! 4. **存档完整性检查** — 深度格式校验 + 数据逻辑检查
//! 5. **批量完整性检查** — 扫描目录中所有存档文件，批量校验
//! 6. **存档修复工具** — 尝试修复损坏的 RPG Maker 存档文件

use crate::state::{ToolboxAction, ToolboxState};
use crate::theme::colors;
use egui::Ui;

/// 渲染工具箱面板，返回用户触发的操作列表
pub fn render(ui: &mut Ui, state: &mut ToolboxState) -> Vec<ToolboxAction> {
    let mut actions = Vec::new();

    ui.heading("🧰 工具箱");
    ui.add_space(8.0);

    render_lzstring_section(ui, state);
    ui.add_space(8.0);

    render_base64_section(ui, state);
    ui.add_space(8.0);

    render_save_info_section(ui, state, &mut actions);
    ui.add_space(8.0);

    render_integrity_section(ui, state, &mut actions);
    ui.add_space(8.0);

    render_batch_section(ui, state, &mut actions);
    ui.add_space(8.0);

    render_repair_section(ui, state, &mut actions);

    actions
}

fn render_lzstring_section(ui: &mut Ui, state: &mut ToolboxState) {
    egui::CollapsingHeader::new("🗜 LZString 压缩/解压")
        .default_open(true)
        .show(ui, |ui| {
            ui.colored_label(colors::TEXT_SECONDARY, "RPG Maker MV 存档使用的 LZString + Base64 格式");
            ui.add_space(4.0);
            ui.label("输入 (JSON 文本或 Base64 压缩文本):");
            ui.add_sized(
                [ui.available_width(), 100.0],
                egui::TextEdit::multiline(&mut state.lz_input),
            );
            ui.horizontal(|ui| {
                if ui.button("压缩").clicked() {
                    match game_tool_core::lzstring::compress_to_base64(&state.lz_input) {
                        Ok(r) => { state.lz_output = r; state.lz_error.clear(); }
                        Err(e) => { state.lz_error = format!("{:?}", e); }
                    }
                }
                if ui.button("解压").clicked() {
                    match game_tool_core::lzstring::decompress_from_base64(&state.lz_input) {
                        Ok(r) => { state.lz_output = r; state.lz_error.clear(); }
                        Err(e) => { state.lz_error = format!("{:?}", e); }
                    }
                }
                if !state.lz_output.is_empty() && ui.button("📋 复制").clicked() {
                    ui.ctx().copy_text(state.lz_output.clone());
                }
            });
            if !state.lz_output.is_empty() {
                ui.colored_label(colors::SUCCESS, "结果:");
                ui.label(&state.lz_output);
            }
            if !state.lz_error.is_empty() {
                ui.colored_label(colors::ERROR, &state.lz_error);
            }
        });
}

fn render_base64_section(ui: &mut Ui, state: &mut ToolboxState) {
    egui::CollapsingHeader::new("🔤 Base64 编解码")
        .default_open(false)
        .show(ui, |ui| {
            ui.label("输入:");
            ui.add_sized(
                [ui.available_width(), 100.0],
                egui::TextEdit::multiline(&mut state.b64_input),
            );
            ui.horizontal(|ui| {
                if ui.button("编码").clicked() {
                    state.b64_output = game_tool_core::base64::encode(state.b64_input.as_bytes());
                }
                if ui.button("解码").clicked() {
                    if let Some(bytes) = game_tool_core::base64::decode(&state.b64_input) {
                        state.b64_output = String::from_utf8_lossy(&bytes).to_string();
                    } else {
                        state.b64_output = "解码失败: 无效的 Base64 输入".into();
                    }
                }
                if !state.b64_output.is_empty() && ui.button("📋 复制").clicked() {
                    ui.ctx().copy_text(state.b64_output.clone());
                }
            });
            if !state.b64_output.is_empty() {
                ui.label(format!("结果: {}", state.b64_output));
            }
        });
}

fn render_save_info_section(ui: &mut Ui, state: &mut ToolboxState, actions: &mut Vec<ToolboxAction>) {
    egui::CollapsingHeader::new("📄 存档信息查看器")
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("文件路径:");
                ui.add(egui::TextEdit::singleline(&mut state.info_path).hint_text("选择存档文件..."));
                if ui.button("选择").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("选择存档文件").pick_file() {
                        state.info_path = path.to_string_lossy().to_string();
                    }
                }
                if !state.info_path.is_empty() && ui.button("查看").clicked() {
                    actions.push(ToolboxAction::GetSaveInfo(state.info_path.clone()));
                }
            });
            if let Some(ref info) = state.info_result {
                ui.separator();
                ui.label(format!("格式: {}", info.format_name));
                ui.label(format!("引擎: {}", info.engine));
                ui.label(format!("大小: {} 字节", info.file_size));
                ui.label(format!("修改时间: {}", info.modified));
                if info.is_valid {
                    ui.colored_label(colors::SUCCESS, "状态: 有效");
                } else {
                    ui.colored_label(colors::ERROR, format!("状态: 无效 — {}", info.error.as_deref().unwrap_or("")));
                }
            }
        });
}

fn render_integrity_section(ui: &mut Ui, state: &mut ToolboxState, actions: &mut Vec<ToolboxAction>) {
    egui::CollapsingHeader::new("🔍 存档完整性检查")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("文件路径:");
                ui.add(egui::TextEdit::singleline(&mut state.check_path).hint_text("选择存档文件..."));
                if ui.button("选择").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("选择存档文件").pick_file() {
                        state.check_path = path.to_string_lossy().to_string();
                    }
                }
                if !state.check_path.is_empty() && ui.button("检查").clicked() {
                    actions.push(ToolboxAction::IntegrityCheck(state.check_path.clone()));
                }
            });

            if let Some(ref result) = state.check_result {
                ui.separator();
                ui.heading("检查结果");
                ui.label(format!("文件: {}", result.file_path));
                ui.label(format!("格式: {}", result.format_name));
                ui.label(format!("大小: {} 字节", result.file_size));
                ui.label(format!("字段数: {}", result.field_count));
                if let Some(ref s) = result.summary {
                    ui.label(format!("金币: {}", s.gold));
                    ui.label(format!("游玩时间: {} 秒", s.play_time));
                }

                if result.is_valid {
                    ui.colored_label(colors::SUCCESS, "✓ 格式校验通过");
                } else {
                    ui.colored_label(colors::ERROR, "✗ 格式校验失败");
                }

                if !result.errors.is_empty() {
                    ui.colored_label(colors::ERROR, "错误:");
                    for e in &result.errors {
                        ui.colored_label(colors::ERROR, format!("  • {}", e));
                    }
                }

                if !result.warnings.is_empty() {
                    ui.colored_label(colors::WARNING, "警告:");
                    for w in &result.warnings {
                        ui.colored_label(colors::WARNING, format!("  • {}", w));
                    }
                }

                if ui.button("清除结果").clicked() {
                    actions.push(ToolboxAction::ClearCheck);
                }
            }
        });
}

fn render_batch_section(ui: &mut Ui, state: &mut ToolboxState, actions: &mut Vec<ToolboxAction>) {
    egui::CollapsingHeader::new("📂 批量完整性检查")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("目录:");
                ui.add(egui::TextEdit::singleline(&mut state.batch_dir).hint_text("选择包含存档的目录..."));
                if ui.button("选择").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().set_title("选择存档目录").pick_folder() {
                        state.batch_dir = dir.to_string_lossy().to_string();
                    }
                }
                if !state.batch_dir.is_empty() && ui.button("扫描").clicked() {
                    actions.push(ToolboxAction::BatchCheck(state.batch_dir.clone()));
                }
            });

            if !state.batch_results.is_empty() {
                ui.separator();
                ui.label(format!("扫描结果: 共 {} 个文件", state.batch_results.len()));

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("文件名").strong());
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("格式").strong());
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("状态").strong());
                });
                ui.separator();

                for result in &state.batch_results {
                    let fname = std::path::Path::new(&result.file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&result.file_path);
                    ui.horizontal(|ui| {
                        ui.label(fname);
                        ui.add_space(20.0);
                        ui.label(&result.format_name);
                        ui.add_space(20.0);
                        if result.is_valid {
                            ui.colored_label(colors::SUCCESS, "✓");
                        } else {
                            ui.colored_label(colors::ERROR, "✗");
                        }
                    });
                }

                if ui.button("清除结果").clicked() {
                    actions.push(ToolboxAction::ClearBatch);
                }
            }
        });
}

fn render_repair_section(ui: &mut Ui, state: &mut ToolboxState, actions: &mut Vec<ToolboxAction>) {
    egui::CollapsingHeader::new("🔧 存档修复工具")
        .show(ui, |ui| {
            ui.colored_label(colors::TEXT_SECONDARY, "适用于 RPG Maker MV/MZ 存档 (.rpgsave/.rmmzsave)");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("文件路径:");
                ui.add(egui::TextEdit::singleline(&mut state.repair_path).hint_text("选择损坏的存档文件..."));
                if ui.button("选择").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_title("选择损坏的存档文件").pick_file() {
                        state.repair_path = path.to_string_lossy().to_string();
                    }
                }
                if !state.repair_path.is_empty() && ui.button("修复").clicked() {
                    actions.push(ToolboxAction::RepairSave(state.repair_path.clone()));
                }
            });

            if let Some(ref result) = state.repair_result {
                ui.separator();
                if result.success {
                    ui.colored_label(colors::SUCCESS, "✓ 修复成功");
                    if let Some(ref p) = result.repaired_path {
                        ui.label(format!("修复后文件: {}", p));
                    }
                } else {
                    ui.colored_label(colors::ERROR, "✗ 修复失败");
                    if !result.original_errors.is_empty() {
                        for e in &result.original_errors {
                            ui.colored_label(colors::ERROR, format!("  • {}", e));
                        }
                    }
                }
                if !result.remaining_errors.is_empty() {
                    ui.colored_label(colors::WARNING, "残留问题:");
                    for e in &result.remaining_errors {
                        ui.colored_label(colors::WARNING, format!("  • {}", e));
                    }
                }
                if ui.button("清除结果").clicked() {
                    actions.push(ToolboxAction::ClearRepair);
                }
            }
        });
}
