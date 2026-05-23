//! 主题系统：管理应用的明/暗色外观、颜色常量、以及标签页图标/名称映射。
//!
//! 色板分为暗色模式与亮色模式两组，外加通用语义颜色（成功绿、警告橙、错误红）。
//! 辅助函数提供引擎类型和标签页的本地化显示名称。

use egui::{Color32, Stroke, Visuals};

/// 主题配置：控制应用的明/暗色外观与控件样式
pub struct Theme {
    pub dark_mode: bool,  // true=暗色主题, false=亮色主题
}

impl Theme {
    /// 根据明暗模式创建主题实例
    pub fn new(dark_mode: bool) -> Self {
        Self { dark_mode }
    }

    /// 将主题应用到 egui 上下文：设置视觉样式（颜色、间距）和控件外观
    pub fn apply(&self, ctx: &egui::Context) {
        // 选择基础视觉方案（暗色/亮色）
        let mut visuals = if self.dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        // 根据模式选择背景色
        let bg = if self.dark_mode {
            Color32::from_rgb(13, 17, 23)
        } else {
            Color32::from_rgb(255, 255, 255)
        };
        // 面板背景色
        let panel_bg = if self.dark_mode {
            colors::PANEL_DARK
        } else {
            colors::PANEL_LIGHT
        };
        // 文字颜色
        let text = if self.dark_mode {
            colors::TEXT
        } else {
            colors::TEXT_LIGHT
        };
        // 强调色（用于按钮激活态、选中态等）
        let accent = if self.dark_mode {
            colors::ACCENT
        } else {
            colors::ACCENT_LIGHT
        };

        visuals.panel_fill = panel_bg;
        visuals.window_fill = bg;
        visuals.extreme_bg_color = bg;

        // 控件各交互状态的颜色
        visuals.widgets.noninteractive.bg_fill = panel_bg;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.inactive.bg_fill = panel_bg;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.hovered.bg_fill = if self.dark_mode {
            colors::HOVER_DARK
        } else {
            colors::HOVER_LIGHT
        };
        visuals.widgets.active.bg_fill = accent; // 点击/拖拽时的颜色

        // 选中文字的高亮背景与边框
        visuals.selection.bg_fill =
            Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 40);
        visuals.selection.stroke = Stroke::new(1.0, accent);

        visuals.window_shadow = egui::epaint::Shadow::NONE;
        visuals.indent_has_left_vline = false; // 缩进不显示左侧竖线

        ctx.set_visuals(visuals);

        // 设置全局间距样式
        let mut style: egui::Style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.indent = 12.0;
        ctx.set_style(style);
    }
}

/// 颜色常量模块：定义了暗/亮两种模式下使用的色板
pub mod colors {
    use egui::Color32;

    // 暗色模式颜色
    pub const PANEL_DARK: Color32 = Color32::from_rgb(22, 27, 34);   // 暗色面板背景
    pub const HOVER_DARK: Color32 = Color32::from_rgb(33, 38, 45);   // 暗色悬停背景
    pub const TEXT: Color32 = Color32::from_rgb(201, 209, 217);       // 暗色主文字
    pub const ACCENT: Color32 = Color32::from_rgb(88, 166, 255);     // 暗色强调色

    // 亮色模式颜色
    pub const PANEL_LIGHT: Color32 = Color32::from_rgb(246, 248, 250); // 亮色面板背景
    pub const HOVER_LIGHT: Color32 = Color32::from_rgb(234, 238, 242); // 亮色悬停背景
    pub const TEXT_LIGHT: Color32 = Color32::from_rgb(36, 41, 47);     // 亮色主文字
    pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(9, 105, 218);  // 亮色强调色

    // 通用语义颜色
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(139, 148, 158); // 次要文字（灰色提示）
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(72, 79, 88);     // 禁用态文字
    pub const SUCCESS: Color32 = Color32::from_rgb(63, 185, 80);          // 成功/正确（绿色）
    pub const WARNING: Color32 = Color32::from_rgb(210, 153, 34);         // 警告（橙色）
    pub const ERROR: Color32 = Color32::from_rgb(248, 81, 73);            // 错误（红色）
}

/// 返回标签页对应的 Uncode 图标字符
pub fn tab_icon(tab: &crate::state::TabMode) -> &'static str {
    match tab {
        crate::state::TabMode::SaveEditor => "\u{1f4c2}",      // 📂 存档编辑
        crate::state::TabMode::RealtimeEditor => "\u{26a1}",    // ⚡ 实时修改
        crate::state::TabMode::BackupManager => "\u{1f5c4}",    // 🗄 备份管理
        crate::state::TabMode::Toolbox => "\u{1f9f0}",          // 🧰 工具箱
        crate::state::TabMode::Settings => "\u{2699}",          // ⚙ 设置
    }
}

/// 返回标签页的中文名称
pub fn tab_name(tab: &crate::state::TabMode) -> &'static str {
    match tab {
        crate::state::TabMode::SaveEditor => "\u{5b58}\u{6863}\u{7f16}\u{8f91}",        // 存档编辑
        crate::state::TabMode::RealtimeEditor => "\u{5b9e}\u{65f6}\u{4fee}\u{6539}",    // 实时修改
        crate::state::TabMode::BackupManager => "\u{5907}\u{4efd}\u{7ba1}\u{7406}",     // 备份管理
        crate::state::TabMode::Toolbox => "\u{5de5}\u{5177}\u{7bb1}",                   // 工具箱
        crate::state::TabMode::Settings => "\u{8bbe}\u{7f6e}",                          // 设置
    }
}

/// 返回游戏引擎类型的中文/英文显示名称
pub fn engine_display_name(engine: &game_tool_core::detector::EngineType) -> &'static str {
    match engine {
        game_tool_core::detector::EngineType::RpgMakerMv => "RPG Maker MV",
        game_tool_core::detector::EngineType::RpgMakerMz => "RPG Maker MZ",
        game_tool_core::detector::EngineType::NwJs => "NW.js",
        game_tool_core::detector::EngineType::RenPy => "Ren'Py",
        game_tool_core::detector::EngineType::Unreal => "Unreal",
        game_tool_core::detector::EngineType::UnityMono => "Unity (Mono)",
        game_tool_core::detector::EngineType::UnityIl2Cpp => "Unity (IL2CPP)",
        game_tool_core::detector::EngineType::Godot => "Godot",
        game_tool_core::detector::EngineType::Unknown => "\u{672a}\u{77e5}",  // 未知
    }
}
