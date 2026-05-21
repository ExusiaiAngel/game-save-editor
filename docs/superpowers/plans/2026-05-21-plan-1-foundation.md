# Plan 1: P0 Foundation Restructuring

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure core trait design, remove engine-specific code from core, create infra and engine crate scaffoldings, make workspace compile clean.

**Architecture:** Redesign `GameBridge` trait to use `BridgeCommand` enum (eliminating RPG-Maker-specific methods), add `extensions` map to `GameState`, extract `jsonex` module, add shared `backup` utility, create `infra` crate with feature gates, scaffold 4 engine crates, remove old `formats`/`bridges` stubs.

**Tech Stack:** Rust stable, serde, thiserror, tokio (workspace deps)

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| MODIFY | `crates/core/src/types.rs` | Add BridgeCommand, redesign GameBridge trait, update GameState |
| CREATE | `crates/core/src/backup.rs` | Shared save_backup() utility |
| MODIFY | `crates/core/src/lib.rs` | Remove jsonex module, add backup module |
| DELETE | `crates/core/src/jsonex.rs` | Migrated to engines/rpgmaker in Plan 2 |
| MODIFY | `crates/core/Cargo.toml` | Remove zip dependency, add chrono |
| MODIFY | `crates/core/tests/golden_lzstring.rs` | Update CARGO_MANIFEST_DIR path after jsonex move |
| MODIFY | `crates/core/tests/jsonex_roundtrip.rs` → MOVE | Move to engines/rpgmaker in Plan 2 (P0: delete) |
| CREATE | `crates/infra/Cargo.toml` | Feature-gated dependency config |
| CREATE | `crates/infra/src/lib.rs` | Module declarations (feature-gated) |
| CREATE | `crates/infra/src/net.rs` | TcpLineConnection stub |
| CREATE | `crates/infra/src/memory.rs` | ProcessHandle stub |
| CREATE | `crates/engines/rpgmaker/Cargo.toml` | RPG Maker crate deps |
| CREATE | `crates/engines/rpgmaker/src/lib.rs` | Placeholder |
| CREATE | `crates/engines/renpy/Cargo.toml` | Ren'Py crate deps |
| CREATE | `crates/engines/renpy/src/lib.rs` | Placeholder |
| CREATE | `crates/engines/unreal/Cargo.toml` | Unreal crate deps |
| CREATE | `crates/engines/unreal/src/lib.rs` | Placeholder |
| CREATE | `crates/engines/generic/Cargo.toml` | Generic crate deps |
| CREATE | `crates/engines/generic/src/lib.rs` | Placeholder |
| MODIFY | `Cargo.toml` (workspace root) | Update members: remove formats/bridges, add new crates |
| MODIFY | `crates/detector/Cargo.toml` | Add infra dependency (feature=memory) |
| MODIFY | `crates/detector/src/lib.rs` | Update module comment |
| MODIFY | `crates/app/Cargo.toml` | Remove formats/bridges refs, add engine/infra refs |
| MODIFY | `crates/app/src/lib.rs` | Update module comment |
| DELETE | `crates/formats/` | Entire directory (replaced by engines/*) |
| DELETE | `crates/bridges/` | Entire directory (replaced by engines/*) |

---

### Task 1: Create `infra` crate with feature-gated scaffolding

**Files:**
- Create: `crates/infra/Cargo.toml`
- Create: `crates/infra/src/lib.rs`
- Create: `crates/infra/src/net.rs`
- Create: `crates/infra/src/memory.rs`

- [ ] **Step 1: Write Cargo.toml**

```toml
[package]
name = "game-tool-infra"
version = "0.1.0"
edition = "2021"
description = "Game Save Editor — 共享基础设施（网络 + 内存操作）"

[features]
default = []
net = ["tokio"]
memory = ["windows"]

[dependencies]
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true

tokio = { workspace = true, optional = true }
windows = { version = "0.58", optional = true, features = [
    "Win32_System_Threading",
    "Win32_System_Diagnostics_ToolHelp",
    "Win32_System_ProcessStatus",
    "Win32_System_Memory",
    "Win32_Foundation",
] }
```

- [ ] **Step 2: Write lib.rs**

```rust
// game-tool-infra: 共享基础设施

#[cfg(feature = "net")]
pub mod net;

#[cfg(feature = "memory")]
pub mod memory;
```

- [ ] **Step 3: Write net.rs (stub)**

```rust
//! TCP 行协议连接 + WebSocket 基础工具
//!
//! RPG Maker TCP 桥接和 Ren'Py TCP 桥接共用此模块。

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

/// TCP 行协议连接
///
/// 封装 connect / send_line / recv_line，供各引擎桥接复用。
pub struct TcpLineConnection {
    stream: Option<TcpStream>,
    reader: Option<BufReader<TcpStream>>,
}

impl TcpLineConnection {
    /// 连接到指定地址（如 "127.0.0.1:19999"）
    pub fn connect(addr: &str) -> Result<Self, std::io::Error> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(false)?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self {
            stream: Some(stream),
            reader: Some(reader),
        })
    }

    /// 发送一行文本（自动追加 \n）
    pub fn send_line(&mut self, line: &str) -> Result<(), std::io::Error> {
        if let Some(ref mut stream) = self.stream {
            stream.write_all(line.as_bytes())?;
            stream.write_all(b"\n")?;
            stream.flush()?;
        }
        Ok(())
    }

    /// 读取一行文本（去除尾部换行符）
    pub fn recv_line(&mut self) -> Result<String, std::io::Error> {
        if let Some(ref mut reader) = self.reader {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            Ok(line.trim_end_matches(|c| c == '\n' || c == '\r').to_string())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "not connected",
            ))
        }
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn disconnect(&mut self) {
        self.stream = None;
        self.reader = None;
    }
}
```

- [ ] **Step 4: Write memory.rs (stub)**

```rust
//! Windows 进程内存操作
//!
//! 提供 ReadProcessMemory / WriteProcessMemory 封装 + 进程枚举。

/// 进程信息
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: String,
}

/// 模块信息
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub base_address: usize,
    pub size: usize,
}

/// 进程句柄
#[derive(Debug)]
pub struct ProcessHandle {
    pid: u32,
    // 实际实施时使用 windows crate 的 HANDLE
}

impl ProcessHandle {
    /// 通过进程名打开（取第一个匹配）
    pub fn open_by_name(_name: &str) -> Result<Self, std::io::Error> {
        todo!("P4 实施")
    }

    /// 通过 PID 打开
    pub fn open_by_pid(pid: u32) -> Result<Self, std::io::Error> {
        Ok(Self { pid })
    }

    pub fn read<T: Sized>(&self, _address: usize) -> Result<T, std::io::Error> {
        todo!("P4 实施")
    }

    pub fn write<T: Sized>(&self, _address: usize, _value: &T) -> Result<(), std::io::Error> {
        todo!("P4 实施")
    }

    pub fn read_bytes(&self, _address: usize, _len: usize) -> Result<Vec<u8>, std::io::Error> {
        todo!("P4 实施")
    }

    pub fn write_bytes(&self, _address: usize, _data: &[u8]) -> Result<(), std::io::Error> {
        todo!("P4 实施")
    }
}

/// 枚举所有进程
pub fn enumerate_processes(_name_filter: &str) -> Result<Vec<ProcessInfo>, std::io::Error> {
    todo!("P6 实施")
}

/// 枚举进程的所有模块
pub fn enumerate_modules(_pid: u32) -> Result<Vec<ModuleInfo>, std::io::Error> {
    todo!("P6 实施")
}
```

- [ ] **Step 5: Verify `cargo check -p game-tool-infra` passes**

---

### Task 2: Add `BridgeCommand` enum and redesign `GameBridge` trait in core

**Files:**
- Modify: `crates/core/src/types.rs`

- [ ] **Step 1: Add BridgeCommand enum**

Insert before the `ISaveFormat` trait definition (after GameInfo, before // Trait 接口):

```rust
/// 桥接命令枚举 — 引擎无关的读写指令
///
/// 所有 IGameBridge 实现通过此枚举进行统一调度，
/// 替代原先的 set_gold/set_switch/set_variable 等方法。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeCommand {
    /// 读取指定字段 (field_id: 如 "gold", "switch_12", "actor_1_hp")
    ReadField(String),
    /// 写入指定字段
    WriteField(String, Value),
    /// 读取完整游戏状态
    ReadAll,
}
```

- [ ] **Step 2: Replace `IGameBridge` trait with `GameBridge` trait**

Replace the entire `IGameBridge` trait block (lines 278-320) with:

```rust
/// 游戏桥接器接口（命令驱动）
///
/// 所有连接后端（TCP、CDP、进程内存）必须实现此接口。
/// 使用 BridgeCommand 枚举替代引擎特定方法，实现引擎无关。
///
/// 对应 Python 版 `IGameBridge`（重新设计）。
pub trait GameBridge: Send + Sync {
    /// 建立连接
    fn connect(&mut self) -> Result<(), crate::error::GameToolError>;

    /// 断开连接
    fn disconnect(&mut self);

    /// 是否已连接
    fn is_connected(&self) -> bool;

    /// 执行桥接命令
    ///
    /// 各引擎实现自行将 BridgeCommand 翻译为协议指令：
    /// - RPG Maker TCP: "set_gold N" 文本
    /// - Ren'Py: {"action": "set_var", ...} JSON
    /// - Windows Memory: WriteProcessMemory(addr, value)
    fn execute(&self, cmd: &BridgeCommand) -> Result<Value, crate::error::GameToolError>;

    /// 引擎名称（如 "rpg_maker", "renpy", "unreal", "generic"）
    fn engine_name(&self) -> &str;

    /// 优先级（数字越小越优先尝试）
    fn priority(&self) -> i32;
}
```

- [ ] **Step 3: Update `GameState` struct — remove RPG Maker fields, add extensions**

Replace the `GameState` struct (lines 92-124) with:

```rust
/// 统一的游戏状态快照
///
/// 通用字段直接放在结构体上，引擎特定字段存入 extensions。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    /// 引擎类型
    #[serde(default = "default_engine")]
    pub engine: String,
    /// 当前地图名称
    #[serde(default)]
    pub map_name: String,
    /// 游戏时间文本
    #[serde(default)]
    pub play_time: String,
    /// 存档次数
    #[serde(default)]
    pub save_count: i32,
    /// 引擎特定扩展数据
    ///
    /// - RPG Maker: extensions["switches"], extensions["variables"],
    ///              extensions["self_switches"], extensions["party"],
    ///              extensions["items"], extensions["gold"], extensions["steps"]
    /// - Ren'Py: extensions["store"] (所有 store 变量)
    /// - Unreal: extensions["properties"] (内存读取属性)
    /// - Generic: extensions["memory_values"] (内存值快照)
    #[serde(default)]
    pub extensions: HashMap<String, Value>,
}
```

- [ ] **Step 4: Update GameState tests**

Replace the GameState tests (lines 409-465) with:

```rust
    // ── GameState roundtrip ──

    #[test]
    fn test_game_state_roundtrip() {
        let mut extensions = HashMap::new();
        extensions.insert("switches".into(), json!({"1": true, "2": false}));
        extensions.insert("gold".into(), json!(9999));

        let state = GameState {
            engine: "rpg_mv".into(),
            map_name: "城镇".into(),
            play_time: "01:23:45".into(),
            save_count: 7,
            extensions,
        };

        let json = serde_json::to_string(&state).expect("序列化失败");
        let decoded: GameState = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(decoded.engine, "rpg_mv");
        assert_eq!(decoded.map_name, "城镇");
        assert_eq!(decoded.save_count, 7);
        assert_eq!(decoded.extensions.get("gold").and_then(|v| v.as_i64()), Some(9999));
        assert_eq!(
            decoded.extensions.get("switches")
                .and_then(|v| v.get("1"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_game_state_defaults() {
        let json = "{}";
        let state: GameState = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(state.engine, "unknown");
        assert_eq!(state.map_name, "");
        assert!(state.extensions.is_empty());
    }

    #[test]
    fn test_game_state_extensions_roundtrip() {
        use serde_json::json;
        let mut extensions = HashMap::new();
        extensions.insert("raw".into(), json!({"extra": "data"}));

        let state = GameState {
            engine: "generic".into(),
            map_name: "".into(),
            play_time: "".into(),
            save_count: 0,
            extensions,
        };

        let json = serde_json::to_string(&state).expect("序列化失败");
        let decoded: GameState = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(decoded.engine, "generic");
        assert!(decoded.extensions.contains_key("raw"));
    }
```

- [ ] **Step 5: Remove unused `HashMap` import if no longer needed by GameState tests**

No change needed — `HashMap` still used in extensions.

- [ ] **Step 6: Add `BridgeCommand` test**

```rust
    // ── BridgeCommand serialization ──

    #[test]
    fn test_bridge_command_serialization() {
        let cmd = BridgeCommand::ReadField("gold".into());
        let json = serde_json::to_string(&cmd).expect("序列化失败");
        assert!(json.contains("ReadField"));
        assert!(json.contains("gold"));

        let cmd = BridgeCommand::WriteField("switch_12".into(), json!(true));
        let json = serde_json::to_string(&cmd).expect("序列化失败");
        assert!(json.contains("WriteField"));
        assert!(json.contains("switch_12"));

        let cmd = BridgeCommand::ReadAll;
        let json = serde_json::to_string(&cmd).expect("序列化失败");
        assert!(json.contains("ReadAll"));
    }

    #[test]
    fn test_bridge_command_deserialization() {
        let json = r#"{"ReadField":"gold"}"#;
        let cmd: BridgeCommand = serde_json::from_str(json).expect("反序列化失败");
        match cmd {
            BridgeCommand::ReadField(id) => assert_eq!(id, "gold"),
            _ => panic!("expected ReadField"),
        }

        let json = r#"{"WriteField":["switch_1",true]}"#;
        let cmd: BridgeCommand = serde_json::from_str(json).expect("反序列化失败");
        match cmd {
            BridgeCommand::WriteField(id, val) => {
                assert_eq!(id, "switch_1");
                assert_eq!(val, json!(true));
            }
            _ => panic!("expected WriteField"),
        }

        let json = r#""ReadAll""#;
        let cmd: BridgeCommand = serde_json::from_str(json).expect("反序列化失败");
        match cmd {
            BridgeCommand::ReadAll => {}
            _ => panic!("expected ReadAll"),
        }
    }
```

- [ ] **Step 7: Run core tests to verify**

```powershell
cargo test -p game-tool-core
```

Expected: all tests pass. Old IGameBridge trait is removed, no compilation errors.

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/types.rs
git commit -m "refactor(core): redesign GameBridge with BridgeCommand enum, update GameState with extensions"
```

---

### Task 3: Extract `jsonex` from core and update module declarations

**Files:**
- Modify: `crates/core/src/lib.rs`
- Delete: `crates/core/src/jsonex.rs` (temporarily, will be recreated in engines/rpgmaker in Plan 2)
- Modify: `crates/core/src/types.rs` — remove jsonex re-export if any

- [ ] **Step 1: Update lib.rs — remove jsonex module**

Replace content with:

```rust
// game-tool-core: 核心数据模型与通用工具

pub mod backup;
pub mod config;
pub mod error;
pub mod lzstring;
pub mod types;

pub use types::*;
```

- [ ] **Step 2: Remove jsonex.rs**

```powershell
Remove-Item -LiteralPath "crates/core/src/jsonex.rs"
```

- [ ] **Step 3: Remove jsonex integration test**

```powershell
Remove-Item -LiteralPath "crates/core/tests/jsonex_roundtrip.rs"
```

(This test will be recreated in engines/rpgmaker in Plan 2.)

- [ ] **Step 4: Verify `cargo check -p game-tool-core` passes**

core crate no longer imports jsonex — should compile clean.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/lib.rs
git add crates/core/src/jsonex.rs
git add crates/core/tests/jsonex_roundtrip.rs
git commit -m "refactor(core): extract jsonex module (migrated to engines/rpgmaker)"
```

---

### Task 4: Create `backup.rs` shared utility

**Files:**
- Create: `crates/core/src/backup.rs`

- [ ] **Step 1: Write test (TDD)**

Append to `crates/core/src/backup.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

/// 创建存档备份（timestamped .bak 副本）并清理旧备份
///
/// # 参数
/// - `original`: 原始存档文件路径
/// - `keep`: 保留最近几个备份（0 = 不清理）
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

    let backup_name = format!("{}.{}.bak.{}", stem, timestamp, ext);
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
    use std::io::Write;

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
    fn test_save_backup_keeps_limit() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("save.rpgsave");
        fs::write(&original, b"data").unwrap();

        // Create 5 backups
        for _ in 0..5 {
            save_backup(&original, 3).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1100)); // ensure unique timestamp
        }

        // Count .bak files
        let count = std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .ok()
                    .and_then(|e| e.file_name().to_str().map(|n| n.contains(".bak.")))
                    .unwrap_or(false)
            })
            .count();

        assert!(count <= 3, "expected <= 3 backups, got {}", count);
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
```

- [ ] **Step 2: Add chrono dependency to core/Cargo.toml**

```toml
chrono = "0.4"
```

- [ ] **Step 3: Run tests**

```powershell
cargo test -p game-tool-core -- backup
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/backup.rs crates/core/Cargo.toml
git commit -m "feat(core): add shared backup utility"
```

---

### Task 5: Create engine crate scaffoldings

**Files:**
- Create: `crates/engines/rpgmaker/Cargo.toml`
- Create: `crates/engines/rpgmaker/src/lib.rs`
- Create: `crates/engines/renpy/Cargo.toml`
- Create: `crates/engines/renpy/src/lib.rs`
- Create: `crates/engines/unreal/Cargo.toml`
- Create: `crates/engines/unreal/src/lib.rs`
- Create: `crates/engines/generic/Cargo.toml`
- Create: `crates/engines/generic/src/lib.rs`

- [ ] **Step 1: Create directory structure**

```powershell
New-Item -ItemType Directory -Force -Path "crates/engines/rpgmaker/src"
New-Item -ItemType Directory -Force -Path "crates/engines/renpy/src"
New-Item -ItemType Directory -Force -Path "crates/engines/unreal/src"
New-Item -ItemType Directory -Force -Path "crates/engines/generic/src"
```

- [ ] **Step 2: Write rpgmaker Cargo.toml**

```toml
[package]
name = "game-tool-rpgmaker"
version = "0.1.0"
edition = "2021"
description = "Game Save Editor — RPG Maker MV/MZ 存档解析、桥接、插件注入"

[dependencies]
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true
lz-str.workspace = true

game-tool-core = { path = "../../core" }
game-tool-infra = { path = "../../infra", features = ["net"] }
```

- [ ] **Step 3: Write rpgmaker lib.rs**

```rust
// game-tool-rpgmaker: RPG Maker MV/MZ 引擎支持
//
// 模块（按 Plan 2-5 逐步实施）:
// - jsonex:  JsonEx @c/@a 格式解析
// - format:  SaveFormat 实现 (.rpgsave/.rmmzsave)
// - gamedata: Game data 扫描 (System.json 等)
// - scanner: 字段合并 (config + save + live)
// - tcp:     TCP 桥接 (文本命令协议)
// - cdp:     CDP 桥接 (Chrome DevTools Protocol)
// - injector: NW.js 插件注入
```

- [ ] **Step 4: Write renpy Cargo.toml**

```toml
[package]
name = "game-tool-renpy"
version = "0.1.0"
edition = "2021"
description = "Game Save Editor — Ren'Py 存档解析、桥接、插件注入"

[dependencies]
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true
zip.workspace = true

game-tool-core = { path = "../../core" }
game-tool-infra = { path = "../../infra", features = ["net"] }
```

- [ ] **Step 5: Write renpy lib.rs**

```rust
// game-tool-renpy: Ren'Py 引擎支持
//
// 模块（按 Plan 3 逐步实施）:
// - format:  SaveFormat 实现 (ZIP metadata 编辑)
// - bridge:  TCP JSON 桥接
// - injector: Python 插件注入
```

- [ ] **Step 6: Write unreal Cargo.toml**

```toml
[package]
name = "game-tool-unreal"
version = "0.1.0"
edition = "2021"
description = "Game Save Editor — Unreal Engine GVAS 解析与内存操作"

[dependencies]
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true

game-tool-core = { path = "../../core" }
game-tool-infra = { path = "../../infra", features = ["memory"] }
```

- [ ] **Step 7: Write unreal lib.rs**

```rust
// game-tool-unreal: Unreal Engine 支持
//
// 模块（按 Plan 4 逐步实施）:
// - format:  SaveFormat 实现 (GVAS 二进制解析)
// - bridge:  Windows 内存 R/W 桥接
```

- [ ] **Step 8: Write generic Cargo.toml**

```toml
[package]
name = "game-tool-generic"
version = "0.1.0"
edition = "2021"
description = "Game Save Editor — 通用 JSON 存档解析与内存操作"

[dependencies]
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true

game-tool-core = { path = "../../core" }
game-tool-infra = { path = "../../infra", features = ["memory"] }
```

- [ ] **Step 9: Write generic lib.rs**

```rust
// game-tool-generic: 通用/Unity/Godot 引擎支持
//
// 模块（按 Plan 5 逐步实施）:
// - format:  SaveFormat 实现 (JSON flatten/unflatten)
// - bridge:  内存扫描 + R/W 桥接
```

- [ ] **Step 10: Verify each crate compiles**

```powershell
cargo check -p game-tool-rpgmaker
cargo check -p game-tool-renpy
cargo check -p game-tool-unreal
cargo check -p game-tool-generic
```

Expected: all compile (empty crates, no errors).

- [ ] **Step 11: Commit**

```bash
git add crates/engines/
git commit -m "feat: scaffold engine crates (rpgmaker/renpy/unreal/generic)"
```

---

### Task 6: Update workspace Cargo.toml and remove old stubs

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Delete: `crates/formats/` (entire directory)
- Delete: `crates/bridges/` (entire directory)
- Modify: `crates/detector/Cargo.toml`
- Modify: `crates/detector/src/lib.rs`
- Modify: `crates/app/Cargo.toml`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Update workspace Cargo.toml members**

Replace the `members` array:

```toml
[workspace]
resolver = "2"
members = [
    "crates/core",
    "crates/infra",
    "crates/engines/rpgmaker",
    "crates/engines/renpy",
    "crates/engines/unreal",
    "crates/engines/generic",
    "crates/detector",
    "crates/app",
]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
zip = "2"
walkdir = "2"
tempfile = "3"
lz-str = "0.2"
toml = "0.8"
chrono = "0.4"
```

- [ ] **Step 2: Remove old formats crate**

```powershell
Remove-Item -Recurse -Force -LiteralPath "crates/formats"
```

- [ ] **Step 3: Remove old bridges crate**

```powershell
Remove-Item -Recurse -Force -LiteralPath "crates/bridges"
```

- [ ] **Step 4: Update detector Cargo.toml — add infra dependency**

```toml
[package]
name = "game-tool-detector"
version = "0.1.0"
edition = "2021"
description = "Game Save Editor — 游戏存档类型自动检测引擎"

[dependencies]
serde.workspace = true
serde_json.workspace = true
walkdir.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true
game-tool-core = { path = "../core" }
game-tool-infra = { path = "../infra", features = ["memory"] }
```

- [ ] **Step 5: Update detector lib.rs**

```rust
// game-tool-detector: 引擎自动检测
//
// 模块（按 Plan 8 逐步实施）:
// - process:   进程模块枚举检测
// - filesystem: 文件系统签名检测
```

- [ ] **Step 6: Update app Cargo.toml — remove old refs, add engine/infra refs**

```toml
[package]
name = "game-tool-app"
version = "0.1.0"
edition = "2021"
description = "Game Save Editor — 应用入口与编排层"

[dependencies]
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true
tracing-subscriber = { workspace = true, features = ["json", "env-filter"] }
tracing-log = "0.2"
walkdir.workspace = true
tempfile.workspace = true

game-tool-core = { path = "../core" }
game-tool-infra = { path = "../infra", features = ["net", "memory"] }
game-tool-rpgmaker = { path = "../engines/rpgmaker" }
game-tool-renpy = { path = "../engines/renpy" }
game-tool-unreal = { path = "../engines/unreal" }
game-tool-generic = { path = "../engines/generic" }
game-tool-detector = { path = "../detector" }
```

- [ ] **Step 7: Update app lib.rs**

```rust
// game-tool-app: 应用入口与编排层
//
// 模块（按 Plan 9 逐步实施）:
// - main:       应用入口 + tokio runtime
// - registry:   FormatRegistry + BridgeRegistry
// - profile:    配置持久化 (ProfileManager)
// - ui/app:     egui eframe::App
// - ui/layout:  界面布局 (各面板)
```

- [ ] **Step 8: Clean up target directory for removed crates**

```powershell
cargo clean
```

- [ ] **Step 9: Full workspace build check**

```powershell
cargo check --workspace
```

Expected: all 8 crates compile (engines are empty, app main.rs has tracing init only).

- [ ] **Step 10: Run all tests**

```powershell
cargo test --workspace
```

Expected: all core tests pass (types, config, error, lzstring, backup).

- [ ] **Step 11: Commit**

```bash
git add Cargo.toml Cargo.lock
git add crates/detector/Cargo.toml crates/detector/src/lib.rs
git add crates/app/Cargo.toml crates/app/src/lib.rs
git rm -r crates/formats
git rm -r crates/bridges
git commit -m "refactor: restructure workspace — remove formats/bridges stubs, add engines+infra crates"
```

---

### Task 7: Update golden_lzstring test path after jsonex move

**Files:**
- Modify: `crates/core/tests/golden_lzstring.rs`

- [ ] **Step 1: Verify golden_dir path**

The `golden_dir()` function references `../../tests/golden/lzstring`. Since `CARGO_MANIFEST_DIR` is `crates/core/`, the path resolves to `tests/golden/lzstring` from workspace root. This path is correct and does not need to change (jsonex move does not affect it).

- [ ] **Step 2: Verify test still passes**

```powershell
cargo test -p game-tool-core -- golden
```

Expected: all golden tests pass.

- [ ] **Step 3: Commit** (skip — no changes needed)

---

### Task 8: Final verification

- [ ] **Step 1: Full workspace test**

```powershell
cargo test --workspace
```

Expected output: all tests pass across all crates.

- [ ] **Step 2: Clippy check**

```powershell
cargo clippy --workspace -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 3: Verify workspace structure**

```powershell
cargo metadata --format-version=1 --no-deps | ConvertFrom-Json | Select-Object -ExpandProperty packages | Select-Object -ExpandProperty name
```

Expected output:
```
game-tool-core
game-tool-infra
game-tool-rpgmaker
game-tool-renpy
game-tool-unreal
game-tool-generic
game-tool-detector
game-tool-app
```

- [ ] **Step 4: Final commit (if any clippy fixes)**

```bash
git add -A
git commit -m "chore: clippy fixes and final cleanup for Plan 1"
```
