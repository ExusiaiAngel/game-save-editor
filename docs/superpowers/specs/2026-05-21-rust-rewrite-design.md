# Rust 重写规格：Game Save Editor

- **日期：** 2026-05-21
- **版本：** 1.0
- **目标：** 将 Python/PySide6 游戏存档编辑器整体重写为 Rust + egui
- **决策记录：** 全功能移植 / egui GUI / Windows-only / 纯 Rust 内存读写 / 单文件 exe

---

## 1. 总体架构

```
                    ┌────────────────────┐
                    │        app         │  egui GUI + 编排
                    │  registry/profile  │
                    └────────┬───────────┘
                             │
          ┌──────────────────┼──────────────────────┐
          │                  │                      │
    ┌─────┴──────┐   ┌──────┴──────┐       ┌──────┴──────┐
    │  rpgmaker  │   │    renpy    │       │   unreal    │
    │ format+TCP │   │ format+TCP  │       │ format+mem  │
    │ +CDP+注入  │   │ +注入       │       │             │
    └─────┬──────┘   └──────┬──────┘       └──────┬──────┘
          │                  │                      │
          │  ┌───────────────┼──────────────────────┤
          │  │               │                      │
    ┌─────┴──┴───────┐  ┌───┴─────────┐   ┌───────┴──────┐
    │    generic     │  │  detector   │   │    infra     │
    │ format+mem     │  │ process+fs  │   │  net+memory  │
    └───────┬────────┘  └──────┬──────┘   └──────┬───────┘
            │                  │                  │
            └──────────────────┼──────────────────┘
                               │
                         ┌─────┴──────┐
                         │    core    │
                         │ types+cfg  │
                         │ error+lzjs │
                         └────────────┘
```

**8 个 crate，单向无环依赖。**

---

## 2. Crate 详细定义

### 2.1 `core` — 纯抽象层（大部分已有）

```
crates/core/src/
├── types.rs       # ModifiableField, SaveSummary, GameState, GameInfo, BridgeCommand
├── traits.rs      # SaveFormat, GameBridge
├── config.rs      # AppConfig
├── error.rs       # GameToolError
├── lzstring.rs    # LZ-String
└── backup.rs      # 通用存档备份（消除 4 处重复的 .bak 逻辑）
```

**关键类型变更：**

```rust
// ═══ 命令枚举（替代方法-per-概念） ═══
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeCommand {
    ReadField(String),
    WriteField(String, Value),
    ReadAll,
}

// ═══ GameBridge 接口（重新设计） ═══
pub trait GameBridge: Send + Sync {
    fn connect(&mut self) -> Result<(), GameToolError>;
    fn disconnect(&mut self);
    fn is_connected(&self) -> bool;
    fn execute(&self, cmd: BridgeCommand) -> Result<Value, GameToolError>;
    fn engine_name(&self) -> &str;
    fn priority(&self) -> i32;
}

// ═══ GameState（engine 特定字段 → extensions） ═══
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub engine: String,
    pub map_name: String,
    pub play_time: String,
    pub save_count: i32,
    pub extensions: HashMap<String, Value>,
}

// ═══ SaveFormat（保持基本不变） ═══
pub trait SaveFormat: Send + Sync {
    fn name(&self) -> &str;
    fn extensions(&self) -> Vec<String>;
    fn engine_type(&self) -> &str;
    fn magic_bytes(&self) -> Option<&[u8]>;
    fn load(&self, filepath: &str) -> Result<Value, GameToolError>;
    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError>;
    fn detect(&self, filepath: &str) -> bool { /* 默认实现 */ }
    fn find_data_dir(&self, game_dir: &str) -> Option<String>;
    fn get_summary(&self, data: &Value) -> SaveSummary;
    fn scan_fields(&self, data: &Value, game_dir: &str) -> Vec<ModifiableField>;
    fn apply_field(&self, data: &mut Value, field: &ModifiableField) -> Result<(), GameToolError>;
}
```

**backup.rs 接口：**

```rust
pub fn save_backup(original: &Path, keep: usize) -> Result<()>;
// 生成 timestamped .bak 副本 + 清理旧备份（保留最近 keep 个）
```

---

### 2.2 `infra` — 共享基础设施

```
crates/infra/src/
├── lib.rs
├── net.rs        # TcpLineConnection + WebSocket 基础
└── memory.rs     # ProcessHandle + 进程枚举（Windows 限定）
```

**Feature gating：**

```toml
[features]
net = ["tokio"]
memory = ["windows"]
```

**net.rs 接口：**

```rust
pub struct TcpLineConnection { ... }
impl TcpLineConnection {
    pub fn connect(addr: &str) -> Result<Self>;
    pub fn send_line(&mut self, line: &str) -> Result<()>;
    pub fn recv_line(&mut self) -> Result<String>;
    pub fn is_connected(&self) -> bool;
    pub fn disconnect(&mut self);
}

pub struct WsConnection { ... }
impl WsConnection {
    pub fn connect(url: &str) -> Result<Self>;
    pub fn send(&mut self, msg: &str) -> Result<()>;
    pub fn recv(&mut self) -> Result<String>;
    pub fn close(&mut self) -> Result<()>;
}
```

**memory.rs 接口（仅 Windows）：**

```rust
pub struct ProcessHandle { ... }
impl ProcessHandle {
    pub fn open_by_name(name: &str) -> Result<Self>;
    pub fn open_by_pid(pid: u32) -> Result<Self>;
    pub fn read<T: Sized>(&self, address: usize) -> Result<T>;
    pub fn write<T: Sized>(&self, address: usize, value: &T) -> Result<()>;
    pub fn read_bytes(&self, address: usize, len: usize) -> Result<Vec<u8>>;
    pub fn write_bytes(&self, address: usize, data: &[u8]) -> Result<()>;
}

pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: String,
}

pub struct ModuleInfo {
    pub name: String,
    pub base_address: usize,
    pub size: usize,
}

pub fn enumerate_processes(name_filter: &str) -> Result<Vec<ProcessInfo>>;
pub fn enumerate_modules(pid: u32) -> Result<Vec<ModuleInfo>>;
```

---

### 2.3 `engines/rpgmaker` — RPG Maker MV/MZ

```
crates/engines/rpgmaker/src/
├── lib.rs
├── jsonex.rs       # JsonEx @c/@a 解析（从 core 移入，已完成）
├── format.rs       # RpgMakerFormat: impl SaveFormat
├── gamedata.rs     # scan_game_directory() 扫描 System.json 等
├── scanner.rs      # scan_all_modifiable() 合并 config+save+live
├── tcp.rs          # RpgMakerTcpBridge: impl GameBridge（文本命令协议）
├── cdp.rs          # RpgMakerCdpBridge: impl GameBridge（CDP WebSocket）
└── injector.rs     # 注入/检测/移除 GameBridgeServer.js
```

**RpgMakerFormat 核心逻辑：**

```rust
impl SaveFormat for RpgMakerFormat {
    fn load(&self, filepath: &str) -> Result<Value, GameToolError> {
        // 1. 读文件 → 2. base64 解码 → 3. LZ-String 解压 → 4. JSON 解析
        // 5. jsonex::resolve_array() 解析 @c/@a
    }
    fn save(&self, filepath: &str, data: &Value) -> Result<(), GameToolError> {
        // 1. backup::save_backup() → 2. JSON 序列化 → 3. LZ-String 压缩
        // 4. base64 编码 → 5. 写文件
    }
    fn scan_fields(&self, data: &Value, game_dir: &str) -> Vec<ModifiableField> {
        // delegate to scanner::scan_all_modifiable()
    }
}
```

**RpgMakerTcpBridge 协议转换：**

```rust
impl GameBridge for RpgMakerTcpBridge {
    fn execute(&self, cmd: BridgeCommand) -> Result<Value, GameToolError> {
        match cmd {
            BridgeCommand::ReadField(id) => {
                // "switch_12" → TCP send "get_switch 12"
                // "gold" → TCP send "get_state" → parse gold
            }
            BridgeCommand::WriteField(id, val) => {
                // "gold" → TCP send "set_gold 9999"
                // "switch_12" → TCP send "set_switch 12 true"
                // "actor_1_hp" → TCP send "set_actor_hp 1 500"
            }
            BridgeCommand::ReadAll => {
                // TCP send "get_state" → parse → GameState
            }
        }
    }
    fn engine_name(&self) -> &str { "rpg_maker" }
    fn priority(&self) -> i32 { 10 }
}
```

---

### 2.4 `engines/renpy` — Ren'Py

```
crates/engines/renpy/src/
├── lib.rs
├── format.rs    # RenPyFormat: impl SaveFormat（ZIP metadata 编辑）
├── bridge.rs    # RenPyBridge: impl GameBridge（JSON-over-TCP 协议）
└── injector.rs  # 注入/检测/移除 __init__.py
```

**RenPyBridge 协议转换：**

```rust
impl GameBridge for RenPyBridge {
    fn execute(&self, cmd: BridgeCommand) -> Result<Value, GameToolError> {
        // BridgeCommand::WriteField("gold", 9999)
        //   → {"action":"eval", "code":"store.money = 9999"}
        // BridgeCommand::ReadField("gold")
        //   → {"action":"get_var", "name":"money"}
        // BridgeCommand::ReadAll
        //   → {"action":"get_state"} → parse → GameState
    }
}
```

---

### 2.5 `engines/unreal` — Unreal Engine

```
crates/engines/unreal/src/
├── lib.rs
├── format.rs    # UnrealGVASFormat: impl SaveFormat（GVAS 二进制解析）
└── bridge.rs    # UnrealMemoryBridge: impl GameBridge（ProcessHandle R/W）
```

**GVAS 解析逻辑（Python 对应 unreal_gvas.py 294 行）：**

```rust
// 解析流程：
// 1. 验证 GVAS magic（4 字节）
// 2. _parse_header() 解析版本/引擎/分支等元数据
// 3. _extract_properties() 扫描 Int/Float/Str/Bool 属性
// 4. save() 写回原始 raw bytes（当前阶段属性编辑仅内存级）
```

---

### 2.6 `engines/generic` — 通用/Unity/Godot

```
crates/engines/generic/src/
├── lib.rs
├── format.rs    # GenericJsonFormat: impl SaveFormat（JSON flatten/unflatten）
└── bridge.rs    # GenericMemoryBridge: impl GameBridge（内存扫描+R/W）
```

**JSON flatten 逻辑（Python 对应 generic_json.py 267 行）：**

```rust
// _flatten_json(obj, prefix) → HashMap<String, Value>
// _unflatten_json(flat) → Value
// 输入 {"player": {"hp": 100, "mp": 50}} 
// → {"player.hp": 100, "player.mp": 50}
```

---

### 2.7 `detector` — 引擎检测

```
crates/detector/src/
├── lib.rs
├── process.rs      # detect_by_process(pid) → EngineType
└── filesystem.rs   # detect_by_filesystem(dir) → EngineType
```

**公共接口：**

```rust
pub fn detect_game(
    save_path: Option<&str>,
    game_dir: Option<&str>,
    process_pid: Option<u32>,
) -> Result<GameInfo>;
```

**检测优先级：** 进程模块枚举 → save_path 推断 → game_dir 文件系统签名 → brute force 向上遍历

---

### 2.8 `app` — egui 应用

```
crates/app/src/
├── main.rs
├── lib.rs
├── registry.rs      # FormatRegistry + BridgeRegistry
├── profile.rs       # ProfileManager（JSON 持久化）
└── ui/
    ├── mod.rs
    ├── app.rs       # struct GameSaveEditor: eframe::App
    ├── file_panel.rs    # 文件操作栏
    ├── game_panel.rs    # 游戏连接栏
    ├── edit_panel.rs    # 编辑面板
    └── table.rs         # 字段表格组件
```

**registry.rs 注册逻辑：**

```rust
// 注册所有格式处理器
FormatRegistry::register(Box::new(RpgMakerFormat::new()));
FormatRegistry::register(Box::new(RenPyFormat::new()));
FormatRegistry::register(Box::new(UnrealGVASFormat::new()));
FormatRegistry::register(Box::new(GenericJsonFormat::new()));

// 注册所有桥接后端（按 priority 排序）
BridgeRegistry::register(Box::new(RpgMakerTcpBridge::new()));
BridgeRegistry::register(Box::new(RpgMakerCdpBridge::new()));
BridgeRegistry::register(Box::new(RenPyBridge::new()));
BridgeRegistry::register(Box::new(UnrealMemoryBridge::new()));
BridgeRegistry::register(Box::new(GenericMemoryBridge::new()));
```

---

## 3. egui UI 布局

对应 Python `ModifyPanel` 的 5 行结构：

```
┌────────────────────────────────────────────────────────┐
│ File: [_____________] [Browse] [Load] [Save]            │  ← file_panel
├────────────────────────────────────────────────────────┤
│ Game: [Scan] [Connect] [Inject] [Refresh Live]          │  ← game_panel
├────────────────────────────────────────────────────────┤
│ Category: [All ▼] Search: [________]      12 / 45 items │  ← toolbar
├────────────────────────────────────────────────────────┤
│ Gold: 3000 | Party: 4 | Items: 27 | Play Time: 01:23:45 │  ← summary bar
├────────────────────────────────────────────────────────┤
│ ID  │ Category │ Name         │ Save │ Live │ Edit     │  ← table
│ 1   │ Gold     │ Gold         │ 3000 │ 3000 │ [Edit]   │
│ 12  │ Switch   │ Door Open    │ ON   │ OFF  │ [Edit]   │
│ 42  │ Variable │ Steps        │ 100  │ --   │ [Edit]   │
│ 3   │ Actor    │ Alice HP     │ 500  │ 480  │ [Edit]   │
│ ... │ ...      │ ...          │ ...  │ ...  │ ...      │
├────────────────────────────────────────────────────────┤
│ [Save All] [Push All] [Refresh Live]    Status: Ready   │  ← bottom bar
└────────────────────────────────────────────────────────┘
```

**egui 渲染要点：**
- `egui::Table` 或 `egui::Grid` 实现字段表格，支持排序和过滤
- `egui::ComboBox` 实现分类过滤
- `egui::TopBottomPanel` 实现顶部/底部工具栏
- `egui::CentralPanel` 包含表格主区域
- tokio runtime 驱动异步网络 I/O（TCP/CDP 连接）
- `Arc<Mutex<>>` 管理共享状态（bridge 连接、scan_result）

---

## 4. Workspace Cargo.toml

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
egui = "0.31"
eframe = "0.31"
lz-str = "0.2"
toml = "0.8"
```

---

## 5. 依赖图

```
                ┌─────────────┐
                │    core     │  types, traits, config, error, lzstring, backup
                └──┬───┬──────┘
                   │   │
          ┌────────┘   └────────┐
          ▼                     ▼
    ┌──────────┐          ┌──────────┐
    │  infra   │          │ detector │
    │ net+mem  │          └────┬─────┘
    └────┬─────┘               │
         │                     │
    ┌────┴─────┬───────┬───────┤
    ▼          ▼       ▼       ▼
  rpgmaker   renpy   unreal  generic
    └────┬─────┴───────┴───────┘
         │                     │
         └─────────┬───────────┘
                   ▼
               ┌──────┐
               │ app  │
               └──────┘
```

**Feature gating 按需引入：**

| 使用方 | infra 功能 |
|--------|-----------|
| `rpgmaker` | `net` |
| `renpy` | `net` |
| `unreal` | `memory` |
| `generic` | `memory` |
| `detector` | `memory` |
| `app` | `net` + `memory` |

---

## 6. 实施阶段

| 阶段 | 内容 | 预估 | 依赖 |
|------|------|------|------|
| **P0** | Trait 重构（BridgeCommand）+ jsonex 迁移 + `infra` crate | 小 | 无 |
| **P1** | `engines/rpgmaker` format.rs + gamedata.rs + scanner.rs + 测试 | 大 | P0 |
| **P2** | `engines/renpy` / `unreal` / `generic` format.rs | 中 | P0 |
| **P3** | `infra/net` + `rpgmaker` tcp.rs + cdp.rs 桥接 | 大 | P0 |
| **P4** | `infra/memory` + `unreal` / `generic` bridge.rs | 中 | P0 |
| **P5** | `rpgmaker` injector.rs + `renpy` bridge.rs + injector.rs | 中 | P3 |
| **P6** | `detector` process.rs + filesystem.rs | 中 | P0 |
| **P7** | `app` egui 界面 + registry.rs + profile.rs + main.rs | 最大 | P1-P6 |

---

## 7. 测试策略

| 层 | 类型 | 对应 Python 测试 |
|----|------|------------------|
| `core` | 87 单元测试（已有）+ backup 新增 | — |
| `rpgmaker/jsonex` | 47+ 测试（已有，随模块迁移） | `test_rpgmv_save.py` 部分 |
| `rpgmaker/format` | 21 测试（save I/O roundtrip + JsonEx 兼容） | `test_rpgmv_save.py` |
| `rpgmaker/gamedata` | 测试 System.json 解析 + 名称映射 | — |
| `renpy/format` | ZIP metadata 读写 + roundtrip | — |
| `unreal/format` | GVAS magic + header + property 解析 | — |
| `generic/format` | JSON flatten/unflatten roundtrip | — |
| `infra/net` | mock TCP 服务端测试 | — |
| `infra/memory` | 模块编译验证 + 条件集成测试 | — |
| `detector` | 测试目录签名识别 + 进程枚举 | `test_bridge_architecture.py` 部分 |
| `rpgmaker` 桥接 | TCP/CDP 协议解析测试 | `test_bridge_architecture.py` |
| `app/ui` | egui 集成测试框架 | — |
| 黄金测试 | LZ-String 103 golden files 保持 | `golden/lzstring/` |

---

## 8. 构建与发布

- **Rust 工具链：** `channel = "stable"`（保持现有 `rust-toolchain.toml`）
- **单文件 exe：** `cargo build --release` → 静态链接 → `target/release/game-save-editor.exe`
- **profiles 嵌入：** `rust-embed` crate 将 `profiles/*.json` 编译进二进制
- **CI/CD：** GitHub Actions
  - `cargo test --workspace`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo build --release`
  - 产物：single .exe artifact

---

## 9. 约定与原则

- **代码风格：** `rustfmt` + `cargo clippy` 零警告
- **错误处理：** `anyhow` 用于应用层，`thiserror` 用于库层
- **异步：** `tokio` runtime 驱动所有网络 I/O
- **状态管理：** `Arc<Mutex<T>>` 管理共享可变状态
- **日志：** `tracing` + `tracing-subscriber`（保持现有）
- **无 unsafe：** 除 `infra/memory.rs`（Windows FFI 必须外，最小化 unsafe 块
- **注释语言：** 中文模块文档 + 英文行注释（保持现有风格）
