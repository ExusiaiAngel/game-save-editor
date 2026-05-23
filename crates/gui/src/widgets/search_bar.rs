//! 搜索栏组件，提供字段名/ID的文本搜索输入框。

use egui::Ui;

/// 渲染搜索栏组件
///
/// 包含搜索图标（🔍）、文本输入框和清除按钮（✕）。
/// 输入框提示文字为"搜索字段..."，输入内容后显示清除按钮。
/// 搜索输入为空时清空提示文字。
pub fn render(ui: &mut Ui, query: &mut String) {
    ui.horizontal(|ui| {
        ui.label("🔍");
        // 输入框为空时显示提示文字
        let hint = if query.is_empty() {
            "搜索字段..."
        } else {
            ""
        };
        if ui
            .add(egui::TextEdit::singleline(query).hint_text(hint))
            .changed()
        {}
        // 清除按钮：有输入内容时显示
        if !query.is_empty() && ui.button("✕").clicked() {
            query.clear();
        }
    });
}
