use egui::Ui;
use game_tool_core::SaveSummary;

pub fn render(ui: &mut Ui, summary: &SaveSummary, currency_unit: &str) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.heading("存档摘要");
        ui.add_space(4.0);

        let time = summary.play_time.max(0);
        let time_str = format!(
            "{:02}:{:02}:{:02}",
            time / 3600,
            (time % 3600) / 60,
            time % 60
        );

        let gold_label = if currency_unit.is_empty() {
            "金币".to_string()
        } else {
            format!("金币 ({})", currency_unit)
        };

        ui.label(format!(
            "{}: {}  队伍: {}人  物品: {}种  存档次数: {}  时长: {}",
            gold_label,
            summary.gold,
            summary.party_size,
            summary.item_count,
            summary.save_count,
            time_str,
        ));

        if !summary.members.is_empty() {
            let m: Vec<&str> = summary
                .members
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| s.as_str())
                .collect();
            if !m.is_empty() {
                ui.label(format!("队员: {}", m.join(", ")));
            }
        }
    });
}
