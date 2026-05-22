//! 引擎自动检测
//!
//! 通过文件系统签名和进程模块枚举检测游戏引擎类型。

use std::path::Path;

/// 检测到的引擎类型
#[derive(Debug, Clone, PartialEq)]
pub enum EngineType {
    RpgMakerMv,
    RpgMakerMz,
    NwJs,
    RenPy,
    UnityMono,
    UnityIl2Cpp,
    Unreal,
    Godot,
    Unknown,
}

impl EngineType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::RpgMakerMv => "rpg_mv",
            Self::RpgMakerMz => "rpg_mz",
            Self::NwJs => "nwjs",
            Self::RenPy => "renpy",
            Self::UnityMono => "unity_mono",
            Self::UnityIl2Cpp => "unity_il2cpp",
            Self::Unreal => "unreal",
            Self::Godot => "godot",
            Self::Unknown => "unknown",
        }
    }
}

/// 通过文件系统检测游戏引擎
pub fn detect_by_filesystem(game_dir: &str) -> EngineType {
    let dir = Path::new(game_dir);

    // RPG Maker MV: www/data/System.json
    if dir.join("www/data/System.json").is_file() {
        return EngineType::RpgMakerMv;
    }

    // RPG Maker MZ: data/System.json + data/Traits.json
    if dir.join("data/System.json").is_file() && dir.join("data/Traits.json").is_file() {
        return EngineType::RpgMakerMz;
    }

    // NW.js: package.json + nw.dll
    if dir.join("package.json").is_file() && dir.join("nw.dll").is_file() {
        return EngineType::NwJs;
    }

    // Ren'Py: game/ directory with .rpy or .rpyc files
    let game_dir_rp = dir.join("game");
    if game_dir_rp.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&game_dir_rp) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".rpy") || name.ends_with(".rpyc") {
                    return EngineType::RenPy;
                }
            }
        }
    }

    // Unreal: */Saved/SaveGames/ 存在
    if dir.join("Saved/SaveGames").is_dir() {
        return EngineType::Unreal;
    }

    // Unity Mono: *_Data/Managed/
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with("_Data") {
                let managed = entry.path().join("Managed");
                if managed.is_dir() {
                    return EngineType::UnityMono;
                }
                let il2cpp = entry.path().join("il2cpp_data");
                if il2cpp.is_dir() {
                    return EngineType::UnityIl2Cpp;
                }
            }
        }
    }

    EngineType::Unknown
}

/// 完整游戏检测：优先级 文件系统 > brute force 向上遍历
pub fn detect_game(save_path: Option<&str>, game_dir: Option<&str>) -> Option<String> {
    if let Some(dir) = game_dir {
        let engine = detect_by_filesystem(dir);
        if engine != EngineType::Unknown {
            return Some(dir.to_string());
        }
    }

    if let Some(save) = save_path {
        let mut current = Path::new(save);
        // 向上遍历最多5层
        for _ in 0..5 {
            if let Some(parent) = current.parent() {
                let engine = detect_by_filesystem(&parent.to_string_lossy());
                if engine != EngineType::Unknown {
                    return Some(parent.to_string_lossy().to_string());
                }
                current = parent;
            } else {
                break;
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_rpg_maker_mv() {
        let dir = tempfile::tempdir().unwrap();
        let www_data = dir.path().join("www/data");
        fs::create_dir_all(&www_data).unwrap();
        fs::write(www_data.join("System.json"), "{}").unwrap();

        assert_eq!(
            detect_by_filesystem(&dir.path().to_string_lossy()),
            EngineType::RpgMakerMv
        );
    }

    #[test]
    fn test_detect_unknown() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            detect_by_filesystem(&dir.path().to_string_lossy()),
            EngineType::Unknown
        );
    }

    #[test]
    fn test_detect_game_from_save_path() {
        let dir = tempfile::tempdir().unwrap();
        let www_data = dir.path().join("www/data");
        fs::create_dir_all(&www_data).unwrap();
        fs::write(www_data.join("System.json"), "{}").unwrap();

        let save_dir = dir.path().join("www/save");
        fs::create_dir_all(&save_dir).unwrap();
        let save_path = save_dir.join("file1.rpgsave");

        let result = detect_game(
            Some(&save_path.to_string_lossy()),
            None,
        );
        assert!(result.is_some());
    }
}
