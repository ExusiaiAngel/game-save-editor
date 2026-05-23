use game_tool_core::ISaveFormat;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub fn find_save_files(game_dir: &str, format: &dyn ISaveFormat) -> Vec<String> {
    let exts = format.extensions();
    let mut seen = HashSet::new();
    let mut files = Vec::new();

    let mut search_dirs = Vec::new();
    if let Some(d) = format.find_data_dir(game_dir) {
        search_dirs.push(d);
    }

    let base = Path::new(game_dir);
    for sub in &[
        "www/save",
        "www/Save",
        "save",
        "Save",
        "saves",
        "game/saves",
        "Saved/SaveGames",
    ] {
        let d = base.join(sub);
        if d.is_dir() {
            let s = d.to_string_lossy().to_string();
            if !search_dirs.contains(&s) {
                search_dirs.push(s);
            }
        }
    }

    for dir in &search_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.contains(".bak.")
                        || name.ends_with(".bak")
                        || name == "config.rpgsave"
                        || name == "global.rpgsave"
                    {
                        continue;
                    }
                    for ext in &exts {
                        if name.ends_with(ext.as_str()) {
                            let canonical = path.to_string_lossy().to_string();
                            if seen.insert(canonical.clone()) {
                                files.push(canonical);
                            }
                        }
                    }
                }
            }
        }
    }

    files.sort_by(|a, b| {
        let ma = fs::metadata(a).and_then(|m| m.modified()).ok();
        let mb = fs::metadata(b).and_then(|m| m.modified()).ok();
        mb.cmp(&ma)
    });

    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_tool_core::error::GameToolError;
    use game_tool_core::{ISaveFormat, ModifiableField, SaveSummary};
    use serde_json::Value;
    use std::fs;

    struct TestFormat {
        exts: Vec<String>,
        data_dir: Option<String>,
    }

    impl ISaveFormat for TestFormat {
        fn name(&self) -> &str {
            "TestFormat"
        }
        fn extensions(&self) -> Vec<String> {
            self.exts.clone()
        }
        fn engine_type(&self) -> &str {
            "test"
        }
        fn magic_bytes(&self) -> Option<&[u8]> {
            None
        }
        fn load(&self, _: &str) -> Result<Value, GameToolError> {
            Ok(Value::Null)
        }
        fn save(&self, _: &str, _: &Value) -> Result<(), GameToolError> {
            Ok(())
        }
        fn find_data_dir(&self, _game_dir: &str) -> Option<String> {
            self.data_dir.clone()
        }
        fn get_summary(&self, _: &Value) -> SaveSummary {
            SaveSummary::default()
        }
        fn scan_fields(&self, _: &Value, _: &str) -> Vec<ModifiableField> {
            Vec::new()
        }
        fn apply_field(&self, _: &mut Value, _: &ModifiableField) -> Result<(), GameToolError> {
            Ok(())
        }
    }

    #[test]
    fn test_find_save_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let fmt = TestFormat {
            exts: vec![".rpgsave".into()],
            data_dir: None,
        };
        let files = find_save_files(dir.path().to_str().unwrap(), &fmt);
        assert!(files.is_empty());
    }

    #[test]
    fn test_find_save_files_matches_extension() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file1.rpgsave"), b"data").unwrap();
        fs::write(dir.path().join("readme.txt"), b"text").unwrap();

        let fmt = TestFormat {
            exts: vec![".rpgsave".into()],
            data_dir: Some(dir.path().to_str().unwrap().into()),
        };
        let files = find_save_files(dir.path().to_str().unwrap(), &fmt);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("file1.rpgsave"));
    }

    #[test]
    fn test_find_save_files_excludes_backup() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("savedata")).unwrap();
        fs::write(dir.path().join("savedata").join("file1.rpgsave"), b"data").unwrap();
        fs::write(
            dir.path().join("savedata").join("file1.bak.rpgsave"),
            b"backup",
        )
        .unwrap();

        let fmt = TestFormat {
            exts: vec![".rpgsave".into()],
            data_dir: Some(dir.path().join("savedata").to_str().unwrap().into()),
        };
        let files = find_save_files(dir.path().to_str().unwrap(), &fmt);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_find_save_files_excludes_bak_suffix() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("savedata")).unwrap();
        fs::write(dir.path().join("savedata").join("save1.rpgsave"), b"data").unwrap();
        fs::write(dir.path().join("savedata").join("backup.bak"), b"old").unwrap();

        let fmt = TestFormat {
            exts: vec![".rpgsave".into()],
            data_dir: Some(dir.path().join("savedata").to_str().unwrap().into()),
        };
        let files = find_save_files(dir.path().to_str().unwrap(), &fmt);
        assert_eq!(files.len(), 1);
        assert!(files[0].contains("save1"));
    }

    #[test]
    fn test_find_save_files_excludes_config_and_global() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("savedata")).unwrap();
        fs::write(dir.path().join("savedata").join("save1.rpgsave"), b"data").unwrap();
        fs::write(dir.path().join("savedata").join("config.rpgsave"), b"cfg").unwrap();
        fs::write(dir.path().join("savedata").join("global.rpgsave"), b"glb").unwrap();

        let fmt = TestFormat {
            exts: vec![".rpgsave".into()],
            data_dir: Some(dir.path().join("savedata").to_str().unwrap().into()),
        };
        let files = find_save_files(dir.path().to_str().unwrap(), &fmt);
        assert_eq!(files.len(), 1);
        assert!(files[0].contains("save1"));
    }

    #[test]
    fn test_find_save_files_dedup() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("common.rpgsave"), b"x").unwrap();

        let fmt = TestFormat {
            exts: vec![".rpgsave".into()],
            data_dir: Some(dir.path().to_str().unwrap().into()),
        };
        let files = find_save_files(dir.path().to_str().unwrap(), &fmt);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_find_save_files_unknown_dir() {
        let fmt = TestFormat {
            exts: vec![".rpgsave".into()],
            data_dir: None,
        };
        let files = find_save_files("nonexistent_dir_xyz", &fmt);
        assert!(files.is_empty());
    }

    #[test]
    fn test_find_save_files_with_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("custom").join("saves")).unwrap();
        fs::write(
            dir.path().join("custom").join("saves").join("game.sav"),
            b"data",
        )
        .unwrap();

        let fmt = TestFormat {
            exts: vec![".sav".into()],
            data_dir: Some(
                dir.path()
                    .join("custom")
                    .join("saves")
                    .to_str()
                    .unwrap()
                    .into(),
            ),
        };
        let files = find_save_files(dir.path().to_str().unwrap(), &fmt);
        assert_eq!(files.len(), 1);
    }
}
