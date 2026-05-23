use egui::{Color32, Stroke, Visuals};

pub struct Theme {
    pub dark_mode: bool,
}

impl Theme {
    pub fn new(dark_mode: bool) -> Self {
        Self { dark_mode }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        let bg = if self.dark_mode {
            Color32::from_rgb(13, 17, 23)
        } else {
            Color32::from_rgb(255, 255, 255)
        };
        let panel_bg = if self.dark_mode {
            colors::PANEL_DARK
        } else {
            colors::PANEL_LIGHT
        };
        let text = if self.dark_mode {
            colors::TEXT
        } else {
            colors::TEXT_LIGHT
        };
        let accent = if self.dark_mode {
            colors::ACCENT
        } else {
            colors::ACCENT_LIGHT
        };

        visuals.panel_fill = panel_bg;
        visuals.window_fill = bg;
        visuals.extreme_bg_color = bg;

        visuals.widgets.noninteractive.bg_fill = panel_bg;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.inactive.bg_fill = panel_bg;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, text);
        visuals.widgets.hovered.bg_fill = if self.dark_mode {
            colors::HOVER_DARK
        } else {
            colors::HOVER_LIGHT
        };
        visuals.widgets.active.bg_fill = accent;

        visuals.selection.bg_fill =
            Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 40);
        visuals.selection.stroke = Stroke::new(1.0, accent);

        visuals.window_shadow = egui::epaint::Shadow::NONE;
        visuals.indent_has_left_vline = false;

        ctx.set_visuals(visuals);

        let mut style: egui::Style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.indent = 12.0;
        ctx.set_style(style);
    }
}

pub mod colors {
    use egui::Color32;

    pub const PANEL_DARK: Color32 = Color32::from_rgb(22, 27, 34);
    pub const PANEL_LIGHT: Color32 = Color32::from_rgb(246, 248, 250);
    pub const HOVER_DARK: Color32 = Color32::from_rgb(33, 38, 45);
    pub const HOVER_LIGHT: Color32 = Color32::from_rgb(234, 238, 242);
    pub const TEXT: Color32 = Color32::from_rgb(201, 209, 217);
    pub const TEXT_LIGHT: Color32 = Color32::from_rgb(36, 41, 47);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(139, 148, 158);
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(72, 79, 88);
    pub const ACCENT: Color32 = Color32::from_rgb(88, 166, 255);
    pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(9, 105, 218);
    pub const SUCCESS: Color32 = Color32::from_rgb(63, 185, 80);
    pub const WARNING: Color32 = Color32::from_rgb(210, 153, 34);
    pub const ERROR: Color32 = Color32::from_rgb(248, 81, 73);
}

pub fn tab_icon(tab: &crate::state::TabMode) -> &'static str {
    match tab {
        crate::state::TabMode::SaveEditor => "\u{1f4c2}",
        crate::state::TabMode::RealtimeEditor => "\u{26a1}",
        crate::state::TabMode::BackupManager => "\u{1f5c4}",
        crate::state::TabMode::Toolbox => "\u{1f9f0}",
        crate::state::TabMode::Settings => "\u{2699}",
    }
}

pub fn tab_name(tab: &crate::state::TabMode) -> &'static str {
    match tab {
        crate::state::TabMode::SaveEditor => "\u{5b58}\u{6863}\u{7f16}\u{8f91}",
        crate::state::TabMode::RealtimeEditor => "\u{5b9e}\u{65f6}\u{4fee}\u{6539}",
        crate::state::TabMode::BackupManager => "\u{5907}\u{4efd}\u{7ba1}\u{7406}",
        crate::state::TabMode::Toolbox => "\u{5de5}\u{5177}\u{7bb1}",
        crate::state::TabMode::Settings => "\u{8bbe}\u{7f6e}",
    }
}

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
        game_tool_core::detector::EngineType::Unknown => "\u{672a}\u{77e5}",
    }
}
