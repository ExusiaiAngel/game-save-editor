use crate::state::AppState;
use crate::theme::colors;
use egui::Ui;

pub fn render(ui: &mut Ui, _state: &AppState) {
    ui.heading("\u{1f9f0} \u{5de5}\u{5177}\u{7bb1}");
    ui.add_space(8.0);

    let mut lz_input = String::new();
    let mut lz_output = String::new();
    let mut lz_error = String::new();

    ui.collapsing("\u{1f5dc} LZString \u{538b}\u{7f29}/\u{89e3}\u{538b}", |ui| {
        ui.colored_label(
            colors::TEXT_SECONDARY,
            "RPG Maker MV \u{5b58}\u{6863}\u{4f7f}\u{7528}\u{7684} LZString + Base64 \u{683c}\u{5f0f}",
        );
        ui.add_space(4.0);
        ui.label("\u{8f93}\u{5165} (JSON \u{6587}\u{672c}\u{6216} Base64 \u{538b}\u{7f29}\u{6587}\u{672c}):");
        ui.text_edit_multiline(&mut lz_input);
        ui.horizontal(|ui| {
            if ui.button("\u{538b}\u{7f29}").clicked() {
                match game_tool_core::lzstring::compress_to_base64(&lz_input) {
                    Ok(r) => {
                        lz_output = r;
                        lz_error.clear();
                    }
                    Err(e) => {
                        lz_error = format!("{:?}", e);
                    }
                }
            }
            if ui.button("\u{89e3}\u{538b}").clicked() {
                match game_tool_core::lzstring::decompress_from_base64(&lz_input) {
                    Ok(r) => {
                        lz_output = r;
                        lz_error.clear();
                    }
                    Err(e) => {
                        lz_error = format!("{:?}", e);
                    }
                }
            }
            if !lz_output.is_empty()
                && ui.button("\u{1f4cb} \u{590d}\u{5236}").clicked()
            {
                ui.ctx().copy_text(lz_output.clone());
            }
        });
        if !lz_output.is_empty() {
            ui.colored_label(colors::SUCCESS, "\u{7ed3}\u{679c}:");
            ui.label(&lz_output);
        }
        if !lz_error.is_empty() {
            ui.colored_label(colors::ERROR, &lz_error);
        }
    });

    ui.add_space(8.0);

    let mut b64_input = String::new();
    let mut b64_output = String::new();

    ui.collapsing("\u{1f524} Base64 \u{7f16}\u{89e3}\u{7801}", |ui| {
        ui.label("\u{8f93}\u{5165}:");
        ui.text_edit_multiline(&mut b64_input);
        ui.horizontal(|ui| {
            if ui.button("\u{7f16}\u{7801}").clicked() {
                b64_output = game_tool_core::base64::encode(b64_input.as_bytes());
            }
            if ui.button("\u{89e3}\u{7801}").clicked() {
                if let Some(bytes) = game_tool_core::base64::decode(&b64_input) {
                    b64_output = String::from_utf8_lossy(&bytes).to_string();
                } else {
                    b64_output = "\u{89e3}\u{7801}\u{5931}\u{8d25}: \u{65e0}\u{6548}\u{7684} Base64 \u{8f93}\u{5165}".into();
                }
            }
            if !b64_output.is_empty()
                && ui.button("\u{1f4cb} \u{590d}\u{5236}").clicked()
            {
                ui.ctx().copy_text(b64_output.clone());
            }
        });
        if !b64_output.is_empty() {
            ui.label(format!("\u{7ed3}\u{679c}: {}", b64_output));
        }
    });

    ui.add_space(8.0);

    ui.collapsing("\u{1f50d} \u{5b58}\u{6863}\u{5b8c}\u{6574}\u{6027}\u{68c0}\u{67e5}", |ui| {
        ui.colored_label(
            colors::TEXT_SECONDARY,
            "\u{9009}\u{62e9}\u{5b58}\u{6863}\u{6587}\u{4ef6}\u{540e}\u{ff0c}\u{5c06}\u{68c0}\u{67e5}: JSON \u{5408}\u{6cd5}\u{6027}\u{3001}\u{5f15}\u{64ce}\u{683c}\u{5f0f}\u{5339}\u{914d}\u{3001}magic bytes\u{3001}\u{5fc5}\u{8981}\u{5b57}\u{6bb5}\u{5b8c}\u{6574}\u{6027}\u{3002}",
        );
        ui.colored_label(colors::TEXT_DISABLED, "\u{5c06}\u{5728} Phase 3 \u{5b9e}\u{73b0}\u{3002}");
    });

    ui.add_space(8.0);

    ui.collapsing("\u{1f4c2} \u{6e38}\u{620f}\u{76ee}\u{5f55}\u{626b}\u{63cf}", |ui| {
        ui.colored_label(
            colors::TEXT_SECONDARY,
            "\u{624b}\u{52a8}\u{626b}\u{63cf}\u{6e38}\u{620f}\u{76ee}\u{5f55}\u{ff0c}\u{67e5}\u{770b}\u{5f15}\u{64ce}\u{68c0}\u{6d4b}\u{7ed3}\u{679c}\u{3001}\u{5b58}\u{6863}\u{8def}\u{5f84}\u{3001}\u{5f00}\u{5173}/\u{53d8}\u{91cf}\u{6570}\u{91cf}\u{3002}",
        );
        ui.colored_label(colors::TEXT_DISABLED, "\u{5c06}\u{5728} Phase 3 \u{5b9e}\u{73b0}\u{3002}");
    });
}
