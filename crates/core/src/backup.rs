//! 通用存档备份工具
//!
//! 提供带时间戳的 .bak 备份副本生成 + 旧备份自动清理功能。
//! RPG Maker、Ren'Py、Unreal、Generic JSON 等所有格式共用此模块。

use std::fs;
use std::path::{Path, PathBuf};

/// 创建存档备份文件并清理旧备份
///
/// 在原始存档文件所在目录创建一个带时间戳的 .bak 副本，
/// 然后根据 `keep` 参数清理超出数量限制的旧备份。
///
/// # 参数
/// - `original`: 原始存档文件的路径
/// - `keep`: 保留的最近备份数量（0 = 不清理旧备份，允许无限累积）
///
/// # 返回
/// - `Ok(PathBuf)`: 创建的备份文件路径
/// - `Err(io::Error)`: 原文件不存在或 I/O 操作失败
///
/// # 备份文件命名规则
/// - 有扩展名: `{stem}.{yyyyMMdd_HHmmss}.bak.{ext}`
/// - 无扩展名: `{name}.{yyyyMMdd_HHmmss}.bak`
///
/// # 示例
/// ```ignore
/// // 创建备份，保留最近 10 个
/// save_backup(Path::new("save.rpgsave"), 10)?;
/// // → 创建 save.20260521_223000.bak.rpgsave
/// // → 如果超过 10 个备份，删除最旧的
/// ```
pub fn save_backup(original: &Path, keep: usize) -> Result<PathBuf, std::io::Error> {
    // 源文件必须存在
    if !original.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("文件不存在: {}", original.display()),
        ));
    }

    // 生成时间戳: yyyyMMdd_HHmmss
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();

    // 提取文件名主干（不含扩展名）
    let stem = original.file_stem().unwrap_or_default().to_string_lossy();
    // 防止空 stem（如 .gitignore 这种点文件）匹配所有文件的 bug
    let stem_str = if stem.is_empty() {
        original
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    } else {
        stem.to_string()
    };

    // 提取扩展名
    let ext = original
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    // 构造带时间戳的备份文件名
    let backup_name = if ext.is_empty() {
        format!("{}.{}.bak", stem_str, timestamp)
    } else {
        format!("{}.{}.bak.{}", stem_str, timestamp, ext)
    };
    let backup_path = original.with_file_name(&backup_name);

    // 执行文件复制
    fs::copy(original, &backup_path)?;

    // 清理超出保留数量的旧备份
    if keep > 0 {
        cleanup_old_backups(original, keep)?;
    }

    Ok(backup_path)
}

/// 清理超出保留数量的旧备份文件
///
/// # 清理策略
/// 1. 扫描原始文件所在目录，收集所有匹配的备份文件
/// 2. 按文件修改时间升序排列（最旧的在前）
/// 3. 删除超出 `keep` 数量的最旧文件
///
/// # 匹配规则
/// 备份文件必须：以原始文件 stem 开头 + 包含 ".bak." 子串
fn cleanup_old_backups(original: &Path, keep: usize) -> Result<(), std::io::Error> {
    let parent = original.parent().unwrap_or(Path::new("."));
    let stem = original.file_stem().unwrap_or_default().to_string_lossy();
    // 与 save_backup 中相同的空 stem 保护
    let stem_str = if stem.is_empty() {
        original
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    } else {
        stem.to_string()
    };

    // 收集所有匹配的备份文件路径
    let mut backups: Vec<PathBuf> = fs::read_dir(parent)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(&*stem_str) && n.contains(".bak."))
                .unwrap_or(false)
        })
        .collect();

    // 按修改时间升序排列（最旧的在前）
    backups.sort_by_key(|p| p.metadata().and_then(|m| m.modified()).ok());

    // 循环删除最旧的文件，直到数量 ≤ keep
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
        assert!(backup
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains(".bak."));
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
