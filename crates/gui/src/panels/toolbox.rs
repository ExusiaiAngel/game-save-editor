//! 工具箱面板，提供独立于游戏的实用工具集合。

use crate::state::ToolboxState;
use crate::theme::colors;
use egui::Ui;

/// 渲染工具箱面板，提供独立于游戏的实用工具
///
/// # 工具列表
/// 1. **LZString 压缩/解压** — 处理 RPG Maker MV 存档使用的 LZString + Base64 格式
///    - 输入 JSON 文本或 Base64 压缩文本，可选择压缩或解压
///    - 结果支持一键复制到剪贴板
///    - 错误信息以红色显示
/// 2. **Base64 编解码** — 通用 Base64 编码/解码工具
///    - 编码：将文本转换为 Base64
///    - 解码：将 Base64 还原为文本（无效输入返回错误提示）
/// 3. **存档完整性检查** — 选择存档文件后检查 JSON 合法性、引擎格式匹配、
///    magic bytes、必要字段完整性
/// 4. **游戏目录扫描** — 手动触发目录扫描，查看引擎检测结果、存档路径、
///    开关/变量数量等信息
///
/// # 状态持久化
/// 所有输入/输出/错误信息保存在 `ToolboxState` 中，切换标签页不会丢失。
pub fn render(ui: &mut Ui, state: &mut ToolboxState) {
    ui.heading("\u{1f9f0} \u{5de5}\u{5177}\u{7bb1}");
    ui.add_space(8.0);

    // ========== LZString 压缩/解压工具 ==========
    egui::CollapsingHeader::new("\u{1f5dc} LZString \u{538b}\u{7f29}/\u{89e3}\u{538b}")
        .default_open(true)
        .show(ui, |ui| {
            ui.colored_label(
                colors::TEXT_SECONDARY,
                "RPG Maker MV \u{5b58}\u{6863}\u{4f7f}\u{7528}\u{7684} LZString + Base64 \u{683c}\u{5f0f}",
            );
            ui.add_space(4.0);
            ui.label("\u{8f93}\u{5165} (JSON \u{6587}\u{672c}\u{6216} Base64 \u{538b}\u{7f29}\u{6587}\u{672c}):");
            // 多行输入区域
            ui.add_sized(
                [ui.available_width(), 100.0],
                egui::TextEdit::multiline(&mut state.lz_input),
            );
            ui.horizontal(|ui| {
                // 压缩按钮：JSON -> Base64 压缩文本
                if ui.button("\u{538b}\u{7f29}").clicked() {
                    match game_tool_core::lzstring::compress_to_base64(&state.lz_input) {
                        Ok(r) => {
                            state.lz_output = r;
                            state.lz_error.clear();
                        }
                        Err(e) => {
                            state.lz_error = format!("{:?}", e);
                        }
                    }
                }
                // 解压按钮：Base64 压缩文本 -> JSON
                if ui.button("\u{89e3}\u{538b}").clicked() {
                    match game_tool_core::lzstring::decompress_from_base64(&state.lz_input) {
                        Ok(r) => {
                            state.lz_output = r;
                            state.lz_error.clear();
                        }
                        Err(e) => {
                            state.lz_error = format!("{:?}", e);
                        }
                    }
                }
                // 复制结果到剪贴板
                if !state.lz_output.is_empty()
                    && ui.button("\u{1f4cb} \u{590d}\u{5236}").clicked()
                {
                    ui.ctx().copy_text(state.lz_output.clone());
                }
            });
            // 显示结果
            if !state.lz_output.is_empty() {
                ui.colored_label(colors::SUCCESS, "\u{7ed3}\u{679c}:");
                ui.label(&state.lz_output);
            }
            // 显示错误
            if !state.lz_error.is_empty() {
                ui.colored_label(colors::ERROR, &state.lz_error);
            }
        });

    ui.add_space(8.0);

    // ========== Base64 编解码工具 ==========
    egui::CollapsingHeader::new("\u{1f524} Base64 \u{7f16}\u{89e3}\u{7801}")
        .default_open(false)
        .show(ui, |ui| {
            ui.label("\u{8f93}\u{5165}:");
            ui.add_sized(
                [ui.available_width(), 100.0],
                egui::TextEdit::multiline(&mut state.b64_input),
            );
            ui.horizontal(|ui| {
                // 编码：文本 -> Base64
                if ui.button("\u{7f16}\u{7801}").clicked() {
                    state.b64_output = game_tool_core::base64::encode(state.b64_input.as_bytes());
                }
                // 解码：Base64 -> 文本（失败时显示错误信息）
                if ui.button("\u{89e3}\u{7801}").clicked() {
                    if let Some(bytes) = game_tool_core::base64::decode(&state.b64_input) {
                        state.b64_output = String::from_utf8_lossy(&bytes).to_string();
                    } else {
                        state.b64_output = "\u{89e3}\u{7801}\u{5931}\u{8d25}: \u{65e0}\u{6548}\u{7684} Base64 \u{8f93}\u{5165}".into();
                    }
                }
                // 复制结果
                if !state.b64_output.is_empty()
                    && ui.button("\u{1f4cb} \u{590d}\u{5236}").clicked()
                {
                    ui.ctx().copy_text(state.b64_output.clone());
                }
            });
            // 显示结果
            if !state.b64_output.is_empty() {
                ui.label(format!("\u{7ed3}\u{679c}: {}", state.b64_output));
            }
        });

    ui.add_space(8.0);

    // ========== 存档完整性检查（说明项） ==========
    egui::CollapsingHeader::new("\u{1f50d} \u{5b58}\u{6863}\u{5b8c}\u{6574}\u{6027}\u{68c0}\u{67e5}")
        .show(ui, |ui| {
            ui.colored_label(
                colors::TEXT_SECONDARY,
                "\u{9009}\u{62e9}\u{5b58}\u{6863}\u{6587}\u{4ef6}\u{540e}\u{ff0c}\u{5c06}\u{68c0}\u{67e5}: JSON \u{5408}\u{6cd5}\u{6027}\u{3001}\u{5f15}\u{64ce}\u{683c}\u{5f0f}\u{5339}\u{914d}\u{3001}magic bytes\u{3001}\u{5fc5}\u{8981}\u{5b57}\u{6bb5}\u{5b8c}\u{6574}\u{6027}\u{3002}",
            );
        });

    ui.add_space(8.0);

    // ========== 游戏目录扫描（说明项） ==========
    egui::CollapsingHeader::new("\u{1f4c2} \u{6e38}\u{620f}\u{76ee}\u{5f55}\u{626b}\u{63cf}")
        .show(ui, |ui| {
            ui.colored_label(
                colors::TEXT_SECONDARY,
                "\u{624b}\u{52a8}\u{626b}\u{63cf}\u{6e38}\u{620f}\u{76ee}\u{5f55}\u{ff0c}\u{67e5}\u{770b}\u{5f15}\u{64ce}\u{68c0}\u{6d4b}\u{7ed3}\u{679c}\u{3001}\u{5b58}\u{6863}\u{8def}\u{5f84}\u{3001}\u{5f00}\u{5173}/\u{53d8}\u{91cf}\u{6570}\u{91cf}\u{3002}",
            );
        });
}
