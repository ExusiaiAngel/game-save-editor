//! 存档摘要卡片组件，显示游戏存档的关键概览信息。

use egui::Ui;
use game_tool_core::SaveSummary;

/// 渲染存档摘要卡片
///
/// 使用 `egui::Frame::group` 包裹，显示内容：
/// - 标题："存档摘要"
/// - **摘要行**：金币（含货币单位）、队伍人数、物品种类数、存档次数、游戏时长
///   - 游戏时长以 `HH:MM:SS` 格式显示
/// - **队员列表**：如果队伍中有成员，额外显示队员姓名列表
pub fn render(ui: &mut Ui, summary: &SaveSummary, currency_unit: &str) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.heading("存档摘要");
        ui.add_space(4.0);

        // 将秒数转换为 HH:MM:SS 格式
        let time = summary.play_time.max(0);
        let time_str = format!(
            "{:02}:{:02}:{:02}",
            time / 3600,
            (time % 3600) / 60,
            time % 60
        );

        // 金币标签（带货币单位）
        let gold_label = if currency_unit.is_empty() {
            "金币".to_string()
        } else {
            format!("金币 ({})", currency_unit)
        };

        // 主摘要行
        ui.label(format!(
            "{}: {}  队伍: {}人  物品: {}种  存档次数: {}  时长: {}",
            gold_label,
            summary.gold,
            summary.party_size,
            summary.item_count,
            summary.save_count,
            time_str,
        ));

        // 队员名单
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
