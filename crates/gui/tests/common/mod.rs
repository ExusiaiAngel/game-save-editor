use game_tool_core::error::GameToolError;
use game_tool_core::{ISaveFormat, ModifiableField, SaveSummary};
use serde_json::Value;

#[allow(dead_code)]
pub struct MockSaveFormat {
    pub name: &'static str,
    pub extensions: Vec<String>,
    pub data_dir: Option<String>,
    pub engine: &'static str,
}

impl MockSaveFormat {
    pub fn new(extensions: Vec<String>, data_dir: Option<String>) -> Self {
        Self {
            name: "MockFormat",
            extensions,
            data_dir,
            engine: "mock",
        }
    }
}

impl ISaveFormat for MockSaveFormat {
    fn name(&self) -> &str {
        self.name
    }

    fn extensions(&self) -> Vec<String> {
        self.extensions.clone()
    }

    fn engine_type(&self) -> &str {
        self.engine
    }

    fn magic_bytes(&self) -> Option<&[u8]> {
        None
    }

    fn load(&self, _filepath: &str) -> Result<Value, GameToolError> {
        Ok(Value::Null)
    }

    fn save(&self, _filepath: &str, _data: &Value) -> Result<(), GameToolError> {
        Ok(())
    }

    fn find_data_dir(&self, _game_dir: &str) -> Option<String> {
        self.data_dir.clone()
    }

    fn get_summary(&self, _data: &Value) -> SaveSummary {
        SaveSummary::default()
    }

    fn scan_fields(&self, _data: &Value, _game_dir: &str) -> Vec<ModifiableField> {
        Vec::new()
    }

    fn apply_field(
        &self,
        _data: &mut Value,
        _field: &ModifiableField,
    ) -> Result<(), GameToolError> {
        Ok(())
    }
}
