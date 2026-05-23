//! 游戏引擎自动检测模块
//!
//! 通过文件系统目录结构签名和进程模块枚举两种方式
//! 检测游戏使用的引擎类型（RPG Maker、Ren'Py、Unity、Unreal 等）。

use std::path::Path;

/// 检测到的游戏引擎类型
///
/// 支持主流游戏引擎的识别，`Unknown` 作为未识别时的兜底值。
#[derive(Debug, Clone, PartialEq)]
pub enum EngineType {
    /// RPG Maker MV：以 `www/data/System.json` 为特征文件
    RpgMakerMv,
    /// RPG Maker MZ：以 `data/System.json` + `data/Traits.json` 为特征文件
    RpgMakerMz,
    /// NW.js 封装游戏：以 `package.json` + `nw.dll` 为特征
    NwJs,
    /// Ren'Py 视觉小说引擎：以 `game/*.rpy` 或 `*.rpyc` 文件为特征
    RenPy,
    /// Unity Mono 后端：以 `*_Data/Managed/` 目录为特征
    UnityMono,
    /// Unity IL2CPP 后端：以 `*_Data/il2cpp_data/` 目录为特征
    UnityIl2Cpp,
    /// Unreal Engine：以 `Saved/SaveGames/` 目录为特征
    Unreal,
    /// Godot 引擎
    Godot,
    /// 无法识别的未知引擎
    Unknown,
}

impl EngineType {
    /// 返回引擎类型的字符串标识符
    ///
    /// 用于序列化、日志记录和跨模块传递。
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

/// 通过文件系统目录结构签名检测游戏引擎
///
/// 按特定顺序检查游戏根目录下的特征文件/目录：
/// 1. RPG Maker MV: 检查 `www/data/System.json` 是否存在
/// 2. RPG Maker MZ: 检查 `data/System.json` + `data/Traits.json`
/// 3. NW.js: 检查 `package.json` + `nw.dll`
/// 4. Ren'Py: 检查 `game/` 目录下是否有 `.rpy` 或 `.rpyc` 文件
/// 5. Unreal: 检查 `Saved/SaveGames/` 目录是否存在
/// 6. Unity: 检查 `*_Data/` 下是否有 `Managed/`（Mono）或 `il2cpp_data/`（IL2CPP）
/// 7. 均未匹配返回 `Unknown`
///
/// # 检测优先级设计
/// 检测顺序按引擎流行度和特征可区分性排列：
/// - RPG Maker MV/MZ 的特征最精确（特定 JSON 文件），优先匹配以减少误判
/// - NW.js 需要同时检查 package.json 和 nw.dll，避免与普通 Web 应用混淆
/// - Ren'Py 通过 game/ 目录下的脚本文件扩展名识别
/// - Unity 和 Unreal 的特征较宽泛，放在最后作为兜底检测
pub fn detect_by_filesystem(game_dir: &str) -> EngineType {
    let dir = Path::new(game_dir);

    // ── RPG Maker MV ──
    // 特征：www/data/System.json 文件存在
    // MV 版本将游戏数据放置在 www 子目录下
    if dir.join("www/data/System.json").is_file() {
        return EngineType::RpgMakerMv;
    }

    // ── RPG Maker MZ ──
    // 特征：data/System.json + data/Traits.json 同时存在
    // MZ 版本不再使用 www 目录，数据文件直接在游戏根目录下的 data/ 中
    // Traits.json 是 MZ 新增的配置文件，用于区分 MV（MV 无此文件）
    if dir.join("data/System.json").is_file() && dir.join("data/Traits.json").is_file() {
        return EngineType::RpgMakerMz;
    }

    // ── NW.js 封装 ──
    // 特征：package.json + nw.dll 同时存在根目录
    // package.json 描述 NW.js 应用的入口和配置
    // nw.dll 是 NW.js 运行时核心库，两者共同标识 NW.js 封装的游戏
    if dir.join("package.json").is_file() && dir.join("nw.dll").is_file() {
        return EngineType::NwJs;
    }

    // ── Ren'Py ──
    // 特征：game/ 目录下存在 .rpy 或 .rpyc 脚本文件
    // Ren'Py 将所有游戏脚本和资源放在 game/ 目录下
    // .rpy 是源代码文件，.rpyc 是编译后的字节码文件
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

    // ── Unreal Engine ──
    // 特征：Saved/SaveGames/ 目录存在
    // Unreal Engine 将存档文件存储在项目目录的 Saved/SaveGames/ 下
    if dir.join("Saved/SaveGames").is_dir() {
        return EngineType::Unreal;
    }

    // ── Unity ──
    // 特征：根目录下存在 *_Data/ 子目录
    //   - *_Data/Managed/ 存在 → Mono 后端（使用 .NET IL 字节码）
    //   - *_Data/il2cpp_data/ 存在 → IL2CPP 后端（预编译为原生代码）
    // 遍历根目录下所有 *_Data 结尾的目录名，检查其内部子目录结构
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with("_Data") {
                // 检查 Managed/ 子目录（Mono 后端标志）
                let managed = entry.path().join("Managed");
                if managed.is_dir() {
                    return EngineType::UnityMono;
                }
                // 检查 il2cpp_data/ 子目录（IL2CPP 后端标志）
                let il2cpp = entry.path().join("il2cpp_data");
                if il2cpp.is_dir() {
                    return EngineType::UnityIl2Cpp;
                }
            }
        }
    }

    // 所有引擎特征均未匹配时，返回 Unknown 表示无法识别
    EngineType::Unknown
}

/// 完整游戏检测
///
/// 检测优先级：
/// 1. 如果指定了 `game_dir`，直接检测该目录
/// 2. 如果只指定了 `save_path`，从存档路径向上遍历最多 5 层父目录，
///    依次尝试检测每个父目录是否为游戏根目录
///
/// 返回检测到的游戏根目录路径，未检测到返回 `None`。
pub fn detect_game(save_path: Option<&str>, game_dir: Option<&str>) -> Option<String> {
    // 优先使用显式指定的游戏目录
    if let Some(dir) = game_dir {
        let engine = detect_by_filesystem(dir);
        if engine != EngineType::Unknown {
            return Some(dir.to_string());
        }
    }

    // 从存档路径向上搜索游戏根目录（最多 5 层）
    if let Some(save) = save_path {
        let mut current = Path::new(save);
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

        let result = detect_game(Some(&save_path.to_string_lossy()), None);
        assert!(result.is_some());
    }
}
