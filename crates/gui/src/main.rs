//! GUI 应用程序入口点。
//!
//! 负责：
//! - 加载系统 CJK 字体（确保中文/日文/韩文正常显示）
//! - 配置 eframe 窗口参数
//! - 启动 egui 事件循环

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use game_tool_gui::state::AppState;

/// 尝试加载系统 CJK 字体的二进制数据。
///
/// 按优先级尝试以下字体文件：
/// 1. 微软雅黑 (msyh.ttc) — 最常用的中文系统字体
/// 2. 宋体 (simsun.ttc)
/// 3. 黑体 (SimHei.ttf)
///
/// 如果都找不到则返回 None（使用 egui 内置字体，但中文将显示为方框）。
fn load_cjk_font() -> Option<Vec<u8>> {
    for path in &[
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\SimHei.ttf",
    ] {
        if let Ok(data) = std::fs::read(path) {
            return Some(data);
        }
    }
    None
}

/// 启动时将 PDB 符号文件复制到配置目录（用于崩溃调试）。
fn copy_pdb_to_config_dir() {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let pdb_path = exe_path.with_extension("pdb");
    if !pdb_path.exists() {
        return;
    }
    let dest_dir = dirs_next::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("GameSaveEditor");
    let _ = std::fs::create_dir_all(&dest_dir);
    let dest_path = dest_dir.join("GameSaveEditor.pdb");

    let should_copy = match std::fs::metadata(&dest_path) {
        Ok(dest_meta) => std::fs::metadata(&pdb_path)
            .ok()
            .and_then(|src| {
                let t1 = src.modified().ok()?;
                let t2 = dest_meta.modified().ok()?;
                Some(t1 > t2)
            })
            .unwrap_or(true),
        Err(_) => true,
    };

    if should_copy {
        let _ = std::fs::copy(&pdb_path, &dest_path);
    }
}

/// 应用程序主入口：配置窗口、加载字体、启动事件循环。
fn main() {
    copy_pdb_to_config_dir();

    // 配置原生窗口：1200x800 尺寸，标题 "GameSaveEditor"
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("GameSaveEditor"),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "GameSaveEditor",
        native_options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();

            // 加载 CJK 字体并注册到 egui
            if let Some(cjk_data) = load_cjk_font() {
                fonts.font_data.insert(
                    "CJK".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(cjk_data)),
                );
                // 将 CJK 字体设为 Proportional 和 Monospace 字体系列的首选/备选
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "CJK".to_owned());
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .push("CJK".to_owned());
            }

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(AppState::new(None)))
        }),
    );
}
