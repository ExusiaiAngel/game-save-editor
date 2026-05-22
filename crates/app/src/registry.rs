//! 格式与桥接注册中心

use game_tool_core::ISaveFormat;

pub struct FormatRegistry {
    formats: Vec<Box<dyn ISaveFormat>>,
}

impl FormatRegistry {
    pub fn new() -> Self {
        Self { formats: Vec::new() }
    }

    pub fn register(&mut self, format: Box<dyn ISaveFormat>) {
        self.formats.push(format);
    }

    #[allow(dead_code)]
    pub fn detect(&self, filepath: &str) -> Option<&dyn ISaveFormat> {
        self.formats.iter()
            .find(|f| f.detect(filepath))
            .map(|f| f.as_ref())
    }

    #[allow(dead_code)]
    pub fn get_supported_extensions(&self) -> Vec<String> {
        let mut exts: Vec<String> = self.formats.iter()
            .flat_map(|f| f.extensions())
            .collect();
        exts.sort();
        exts.dedup();
        exts
    }

    #[allow(dead_code)]
    pub fn list_formats(&self) -> Vec<&str> {
        self.formats.iter().map(|f| f.name()).collect()
    }
}

impl Default for FormatRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(game_tool_rpgmaker::format::RpgMakerFormat::new()));
        registry.register(Box::new(game_tool_renpy::format::RenPyFormat::new()));
        registry.register(Box::new(game_tool_unreal::format::UnrealGVASFormat::new()));
        registry.register(Box::new(game_tool_generic::format::GenericJsonFormat::new()));
        registry
    }
}
