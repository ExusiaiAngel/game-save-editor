use egui::Ui;

pub fn render(ui: &mut Ui, query: &mut String) {
    ui.horizontal(|ui| {
        ui.label("🔍");
        let hint = if query.is_empty() {
            "搜索字段..."
        } else {
            ""
        };
        if ui
            .add(egui::TextEdit::singleline(query).hint_text(hint))
            .changed()
        {}
        if !query.is_empty() && ui.button("✕").clicked() {
            query.clear();
        }
    });
}
