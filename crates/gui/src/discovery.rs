//! 存档文件发现模块：在游戏目录中递归搜索匹配的存档文件。
//!
//! 搜索策略：
//! 1. 优先查找格式自身定义的 data_dir（如 RPG Maker 的 www/save）
//! 2. 回退扫描常见的存档子目录名称
//! 3. 按文件修改时间降序排列（最新的在前）

use game_tool_core::ISaveFormat;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// 在游戏目录中搜索匹配格式的所有存档文件。
///
/// 搜索逻辑：
/// 1. 如果格式定义了 find_data_dir()，将该目录加入搜索列表
/// 2. 扫描常见存档目录：www/save, save, saves, game/saves, Saved/SaveGames 等
/// 3. 过滤排除项：备份文件、配置/全局文件
/// 4. 按扩展名匹配，使用 HashSet 去重
/// 5. 按修改时间降序排列（最近修改的排最前）
pub fn find_save_files(game_dir: &str, format: &dyn ISaveFormat) -> Vec<String> {
    let exts = format.extensions();
    let mut seen = HashSet::new();  // 去重集合，避免同一文件被重复添加
    let mut files = Vec::new();

    // 收集所有可能的搜索目录
    let mut search_dirs = Vec::new();
    if let Some(d) = format.find_data_dir(game_dir) {
        search_dirs.push(d);
    }

    // 扫描常见的存档子目录名称（大小写分别尝试）
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

    // 遍历每个搜索目录，收集匹配的存档文件
    for dir in &search_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // 排除备份文件（.bak.xxx 或 .bak 后缀）
                    if name.contains(".bak.")
                        || name.to_lowercase().ends_with(".bak")
                        // 排除 RPG Maker 的配置文件和全局存档（不是玩家存档）
                        || name.to_lowercase() == "config.rpgsave"
                        || name.to_lowercase() == "global.rpgsave"
                    {
                        continue;
                    }
                    // 按扩展名匹配（大小写不敏感）
                    for ext in &exts {
                        if name.to_lowercase().ends_with(&ext.to_lowercase()) {
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

    // 按修改时间降序排列，最近修改的存档排在最前面
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
