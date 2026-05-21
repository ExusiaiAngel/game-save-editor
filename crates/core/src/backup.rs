//! 通用存档备份工具
//!
//! 提供 timestamped .bak 副本生成 + 旧备份清理。
//! RPG Maker、Ren'Py、Unreal、Generic JSON 格式共用此模块。

use std::fs;
use std::path::{Path, PathBuf};

/// 创建存档备份（timestamped .bak 副本）并清理旧备份
///
/// # 参数
/// - `original`: 原始存档文件路径
/// - `keep`: 保留最近几个备份（0 = 不清理）
///
/// # 返回
/// - `Ok(PathBuf)`: 创建的备份文件路径
/// - `Err(io::Error)`: 文件不存在或 I/O 错误
///
/// # 示例
/// ```ignore
/// save_backup(Path::new("save.rpgsave"), 10)?;
/// // → save.rpgsave.20260521_223000.bak
/// ```
pub fn save_backup(original: &Path, keep: usize) -> Result<PathBuf, std::io::Error> {
    if !original.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("文件不存在: {}", original.display()),
        ));
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let stem = original.file_stem().unwrap_or_default().to_string_lossy();
    let ext = original.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();

    let backup_name = if ext.is_empty() {
        format!("{}.{}.bak", stem, timestamp)
    } else {
        format!("{}.{}.bak.{}", stem, timestamp, ext)
    };
    let backup_path = original.with_file_name(&backup_name);

    fs::copy(original, &backup_path)?;

    if keep > 0 {
        cleanup_old_backups(original, keep)?;
    }

    Ok(backup_path)
}

fn cleanup_old_backups(original: &Path, keep: usize) -> Result<(), std::io::Error> {
    let parent = original.parent().unwrap_or(Path::new("."));
    let stem = original.file_stem().unwrap_or_default().to_string_lossy();

    let mut backups: Vec<PathBuf> = fs::read_dir(parent)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(&*stem) && n.contains(".bak."))
                .unwrap_or(false)
        })
        .collect();

    backups.sort_by_key(|p| p.metadata().and_then(|m| m.modified()).ok());

    while backups.len() > keep {
        if let Some(oldest) = backups.first() {
            let _ = fs::remove_file(oldest);
            backups.remove(0);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_backup_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("save.rpgsave");
        fs::write(&original, b"test data").unwrap();

        let backup = save_backup(&original, 0).unwrap();
        assert!(backup.exists());
        assert!(backup.file_name().unwrap().to_string_lossy().contains(".bak."));
        assert_eq!(fs::read_to_string(&backup).unwrap(), "test data");
    }

    #[test]
    fn test_save_backup_nonexistent_file() {
        let result = save_backup(Path::new("nonexistent.rpgsave"), 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_backup_keep_zero_no_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("save.rpgsave");
        fs::write(&original, b"data").unwrap();

        for _ in 0..5 {
            save_backup(&original, 0).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1100));
        }

        let count = std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .ok()
                    .and_then(|e| e.file_name().to_str().map(|n| n.contains(".bak.")))
                    .unwrap_or(false)
            })
            .count();

        assert_eq!(count, 5, "keep=0 should not cleanup");
    }
}
