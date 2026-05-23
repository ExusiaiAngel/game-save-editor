# GameSaveEditor 技术架构

## Workspace 结构

```
game_tool/                        # Cargo workspace root
├── Cargo.toml                    # 共享依赖 + release 配置
├── rust-toolchain.toml           # 工具链版本锁定 (stable)
├── build.bat                     # Windows 构建脚本
├── .github/workflows/ci.yml      # CI/CD 配置
│
├── crates/
│   ├── core/                     # game-tool-core
│   ├── engines/
│   │   ├── rpgmaker/             # game-tool-rpgmaker
│   │   ├── renpy/                # game-tool-renpy
│   │   ├── unreal/               # game-tool-unreal
│   │   ├── generic/              # game-tool-generic
│   │   └── memory/               # game-tool-memory
│   ├── gui/                      # game-tool-gui
│   └── app/                      # GameSaveEditor
│
├── docs/                         # 技术文档
│   └── ARCHITECTURE.md           # 本文件
├── profiles/                     # 游戏内存扫描配置
├── tests/                        # 集成测试
│   └── golden/lzstring/          # LZString golden 测试数据
└── dist/                         # 构建产物（仅本地）
```

## Crate 依赖关系

```
                    ┌─────────────┐
                    │  GameSave   │  CLI 入口 (rust-embed)
                    │  Editor     │
                    └──────┬──────┘
                           │ depends on
                    ┌──────▼──────┐
                    │ game-tool-  │  GUI (egui/eframe)
                    │ gui         │
                    └──┬───┬───┬──┘
                       │   │   │
          ┌────────────┘   │   └────────────┐
          ▼                ▼                ▼
   ┌────────────┐  ┌──────────────┐  ┌───────────┐
   │ game-tool- │  │ game-tool-   │  │ game-tool-│
   │ core       │  │ rpgmaker     │  │ memory    │
   └────────────┘  └──────────────┘  └───────────┘
          │                │                │
          │         ┌──────┴──────┐         │
          │         ▼             ▼         │
          │   ┌──────────┐ ┌──────────┐     │
          │   │ game-tool│ │ game-tool│     │
          │   │ -renpy   │ │ -unreal  │     │
          │   └──────────┘ └──────────┘     │
          │                │                │
          └────────────────┼────────────────┘
                           ▼
                    ┌──────────────┐
                    │ game-tool-   │
                    │ generic      │
                    └──────────────┘
```

所有引擎 crate（rpgmaker、renpy、unreal、generic）依赖 core 提供的 trait 接口。
GUI crate 依赖 core + 所有引擎 crate。
Memory crate 直接使用 windows-sys 进行 Win32 API 调用。

## Crate 职责

### game-tool-core

核心层，定义数据模型和通用工具，不依赖任何引擎特定代码。

| 模块 | 文件 | 职责 |
|------|------|------|
| **types** | `types.rs` | 数据模型（`ModifiableField`, `SaveSummary`, `GameState`, `GameInfo`）+ Trait 接口（`ISaveFormat`, `GameBridge`, `BridgeCommand`,`MemoryCommand`）|
| **config** | `config.rs` | `AppConfig` 结构 + 4 层配置加载（默认值→JSON→TOML→环境变量）|
| **detector** | `detector.rs` | 文件系统引擎检测（RPG Maker MV/MZ, Ren'Py, Unreal, Unity Mono/IL2CPP, Godot）|
| **net** | `net.rs` | `TcpLineConnection` — 行分隔文本协议的 TCP 连接封装 |
| **lzstring** | `lzstring.rs` | RPG Maker MV 兼容的 LZ-String 压缩/解压（封装 lz-str crate）|
| **base64** | `base64.rs` | 纯 Rust Base64 编解码（无外部依赖）|
| **backup** | `backup.rs` | 存档备份管理（时间戳 .bak + 自动清理）|
| **error** | `error.rs` | `GameToolError` 统一错误枚举 + From 转换 |

### 引擎适配层 (engines/)

每个引擎 crate 实现 `ISaveFormat` trait（存档读写）和/或 `GameBridge` trait（实时连接）。

| Crate | ISaveFormat | GameBridge | 关键文件 |
|-------|:---:|:---:|------|
| **rpgmaker** | ✅ | ✅ | `format.rs` (JSON+LZString), `tcp.rs` (GameBridgeServer.js JS 插件), `scanner.rs` (游戏目录扫描), `jsonex.rs` (JSONEx 格式) |
| **renpy** | ✅ | ✅ | `format.rs` (.save ZIP 格式), `bridge.rs` (JSON-over-TCP Python 插件) |
| **unreal** | ✅ | ❌ | `format.rs` (GVAS 二进制读写) |
| **generic** | ✅ | ❌ | `format.rs` (通用 JSON 扁平化/反扁平化) |
| **memory** | ❌ | ✅ | `scanner.rs` (值扫描引擎), `region.rs` (VirtualQueryEx), `bridge.rs` (UniversalMemoryBridge), `process.rs` (进程枚举), `module.rs` (模块枚举) |

#### game-tool-rpgmaker

RPG Maker MV/MZ 引擎适配，处理 `.rpgsave` / `.rmmzsave` 格式：

- **存档格式**：Base64 编码的 LZ-String 压缩 JSON
- **实时通信**：TCP 文本协议，通过 NW.js JavaScript 插件注入游戏进程
- **插件注入**：写入 `GameBridgeServer.js` 到 `www/js/plugins/`，修改 `plugins.js`
- **数据扫描**：从 `System.json`、`Actors.json`、`Items.json` 读取名称映射
- **JSONEx**：RPG Maker 的扩展 JSON 格式（`@a` 稀疏数组、`@c` 压缩数组、`_data` 包装）

#### game-tool-renpy

Ren'Py 视觉小说引擎适配：

- **存档格式**：`.save` 文件是 ZIP 归档，内含 Pickle 序列化的游戏状态和 JSON 元数据
- **实时通信**：JSON-over-TCP 文本协议通过 Python 插件桥接
- **字段映射**：直接从 Python store 对象读取/写入变量

#### game-tool-unreal

Unreal Engine 存档适配：

- **存档格式**：GVAS 二进制格式（Unreal 的 `FBufferArchive` + `FObjectAndNameAsStringProxyArchive`）
- **当前状态**：支持读写，解析支持 IntProperty、FloatProperty、StrProperty、BoolProperty、StructProperty 等常见属性类型；写入基于原始二进制替换属性段（保留头部和尾部不变）
- **无实时编辑**：不支持 TCP 桥接，仅通过内存扫描进行修改

#### game-tool-generic

通用 JSON 格式适配器，用于 Unity (Mono/IL2CPP) 和 Godot 引擎：

- **存档格式**：普通 JSON 文件（`.json`）
- **读/写**：JSON 解析 → 扁平化（所有嵌套路径变为点分隔的 key）→ 编辑 → 反扁平化保存
- **依赖**：由应用层负责检测具体引擎类型

#### game-tool-memory

Windows 进程内存扫描与编辑引擎：

- **进程枚举**：使用 `CreateToolhelp32Snapshot` 枚举系统正在运行的进程
- **模块枚举**：使用 `Module32First/Next` 枚举指定进程的加载模块
- **内存区域**：使用 `VirtualQueryEx` 枚举进程内存区域，筛选可读写区域
- **内存读写**：使用 `ReadProcessMemory` / `WriteProcessMemory`
- **扫描算法**：首次扫描匹配值 → 二次扫描（精确值/增大/减小/未变/已变）
- **存档种子**：从存档中提取字段值作为扫描种子，提高扫描精确度
- **交叉验证**：通过重新扫描确认候选地址的可靠性

### game-tool-gui

egui/eframe 桌面 GUI。

| 模块 | 文件 | 职责 |
|------|------|------|
| **app** | `app.rs` | 主状态机：`AppState`, `update()` 渲染循环、选项卡路由、连接生命周期 |
| **state** | `state.rs` | 所有 UI 状态类型定义 |
| **theme** | `theme.rs` | 暗色/亮色主题（GitHub 风格）、颜色常量、辅助函数 |
| **panels/** | | 界面面板模块 |
| | `top_bar.rs` | 顶部标题栏（游戏标题、引擎类型、目录路径） |
| | `tab_bar.rs` | 选项卡导航栏（存档/实时/备份/工具/设置 + 切换游戏） |
| | `startup.rs` | 启动页（选择游戏目录或从最近列表中打开） |
| | `save_editor.rs` | 存档编辑面板（文件选择/分类筛选/字段编辑表） |
| | `realtime_editor.rs` | 实时修改面板（TCP 连接管理/内存扫描/字段编辑） |
| | `backup.rs` | 备份管理面板（创建/恢复/删除/批量/差异对比） |
| | `toolbox.rs` | 工具箱面板（LZString/Base64/完整性检查） |
| | `settings.rs` | 设置面板（主题/端口/最近游戏/配置/关于） |
| | `status_bar.rs` | 底部状态栏（选项卡感知的上下文信息） |
| **widgets/** | | 可复用 UI 控件 |
| | `field_table.rs` | 可编辑字段表格（类别分组、实时值列、差异高亮、字段跳转） |
| | `category_tree.rs` | 分类筛选树（水平/垂直布局、可折叠分组） |
| | `search_bar.rs` | 搜索输入框（含清除按钮、实时过滤） |
| | `summary_card.rs` | 存档摘要卡片（金币、队伍、道具统计） |
| **其他** | | |
| | `factory.rs` | 引擎类型 → 格式处理器/桥接器/面板模式 的工厂映射 |
| | `connection.rs` | 桥接线程生命周期管理（spawn/drain/cleanup） |
| | `discovery.rs` | 存档文件递归发现（引擎特定扩展名过滤、去重） |
| | `main.rs` | 入口点：CJK 字体加载、PDB 复制、窗口配置 |

### GameSaveEditor (app)

CLI 测试入口，使用 `rust-embed` 内嵌游戏配置文件（profiles）。

## 核心数据模型

### ModifiableField

```rust
pub struct ModifiableField {
    pub category: String,        // 字段分类: "gold", "switch", "variable", "actor", "item"
    pub field_id: String,        // 唯一标识: "gold", "switch_12", "actor_1_hp"
    pub display_name: String,    // UI 显示名称
    pub field_type: String,      // 值类型: "int", "bool", "float", "str"
    pub save_value: Value,       // 存档中的当前值
    pub live_value: Value,       // 游戏进程中的实时值
    pub dirty: bool,             // 用户是否已编辑
    pub locked: bool,            // 是否锁定（实时刷新时跳过）
    pub item_id: i32,            // 数据项 ID（RPG Maker 分页用）
    pub min_val: i32,            // 最小值
    pub max_val: i32,            // 最大值
}
```

### 字段提取流程

```
GameState (桥接响应 或 存档文件)
    → factory::game_state_to_fields()
        → 各引擎的 extract_fields() 实现:
            gold        → ModifiableField { category: "gold",     field_type: "int",  ... }
            switches    → ModifiableField { category: "switch",   field_type: "bool", ... }
            variables   → ModifiableField { category: "variable", field_type: "int",  ... }
            actors      → ModifiableField { category: "actor",    field_type: "int",  ... }
            items       → ModifiableField { category: "item",     field_type: "int",  ... }
```

## 实时连接协议

采用**行分隔文本协议**（非 JSON），用于桥接插件和编辑器。

### 命令（客户端 → 游戏插件）

| 命令 | 参数 | 说明 |
|------|------|------|
| `get_state` | 无 | 读取全部游戏状态 |
| `set_gold` | `<amount>` | 设置金币数量 |
| `set_switch` | `<id> <0\|1>` | 设置开关状态 |
| `set_variable` | `<id> <value>` | 设置变量值 |
| `set_hp` | `<actor_id> <value>` | 设置角色 HP |
| `set_mp` | `<actor_id> <value>` | 设置角色 MP |
| `set_level` | `<actor_id> <value>` | 设置角色等级 |
| `set_item` | `<id> <count>` | 设置物品数量 |
| `set_self_switch` | `<key> <0\|1>` | 设置独立开关 |
| `close` | 无 | 优雅关闭连接 |

### 响应（游戏插件 → 客户端）

| 响应格式 | 说明 |
|----------|------|
| `STATE:{"gold":5000,"switches":{...},"variables":{...},...}` | 完整游戏状态 JSON |
| `OK` | 命令执行成功 |
| `ERR` | 命令无法识别或执行失败 |

### Ren'Py 桥接差异

Ren'Py 桥接使用 JSON 格式的命令而非纯文本：

```json
// 请求
{"action": "read_all"}
{"action": "set_var", "name": "gold", "value": 9999}

// 响应
{"status": "ok", "state": {"gold": 9999, ...}}
{"status": "error", "message": "..."}
```

### 连接生命周期

```
用户点击连接 → rt_connect()
    → spawn_bridge_thread()    创建桥接线程
    → BridgeJob::Connect       TCP 连接
    → BridgeResult::Connected  连接成功
    → 自动发送 ReadAll        获取首次状态快照
    → [每帧 drain_rt_results()  处理异步结果]
    → [定时自动刷新]           可配置 1/2/3/5 秒间隔
    → 用户点击断开 → Disconnect
```

## GUI 架构

### 布局模型

```
┌──────────────────────────────────────────┐
│ TOP BAR: GameSaveEditor    游戏: 冒险传说   │  TopBottomPanel::top
├──────────────────────────────────────────┤
│ [📂存档] [⚡实时] [🗄备份] [🧰工具] [⚙设置]  │  TopBottomPanel::top (tab bar)
├──────────────────────────────────────────┤
│                                          │
│   当前选项卡内容 (CentralPanel)                │
│   根据 active_tab 分发到对应面板              │
│                                          │
├──────────────────────────────────────────┤
│ STATUS: 修改数 | 字段数 | 文件名              │  TopBottomPanel::bottom
└──────────────────────────────────────────┘
```

### 选项卡模式

| TabMode | 不需游戏 | 说明 |
|---------|:---:|------|
| SaveEditor | ❌ | 存档文件选择 + 字段网格编辑 |
| RealtimeEditor | ❌ | TCP 连接管理 / 内存扫描 + 实时字段编辑 |
| BackupManager | ❌ | 备份创建/恢复/删除/差异对比 |
| Toolbox | ✅ | LZString/Base64 独立工具 |
| Settings | ✅ | 主题/端口/最近游戏/配置 |

### 数据流

```
存档编辑:
  用户编辑字段 → field.dirty = true → dirty_count++
    → [保存] → format.apply_field() → format.save() → 写入磁盘
    → dirty_count = 0

实时修改 (TCP):
  用户连接 → bridge thread → BridgeResult::Connected
    → ReadAll → GameState → game_state_to_fields() → rt_panel.fields
    → [编辑实时值] → WriteField → TCP 发送 → 游戏即时更新
    → [锁定字段] → 自动刷新时保留该字段当前值

实时修改 (Memory):
  用户附加进程 → Attach(pid) → BridgeResult::Attached
    → 首次扫描 → FirstScan(value, type) → 返回候选地址列表
    → 二次扫描 → NextScan(mode, value) → 缩小候选范围
    → 从存档获取种子 → SeedFromSave(fields) → 匹配地址
    → 写入值 → WriteField → WriteProcessMemory(addr, value)

备份管理:
  创建备份 → backup::save_backup(original, keep=5)
    → 生成时间戳文件名: {stem}.{yyyyMMdd_HHmmss}.bak.{ext}
    → 超出 keep 时自动清理最旧的备份
```

## 备份命名规范

```
原始文件: save.rpgsave
备份文件: save.20260524_143000.bak.rpgsave

无扩展名: mygame
备份文件: mygame.20260524_143000.bak

清理策略: 按文件修改时间升序排列，删除超出 keep 数量的最旧文件
匹配规则: 以原始文件 stem 开头 + 包含 ".bak." 子串
```

## 配置系统

### 加载优先级

```
1. AppConfig::default()    ← 最低优先级（源码硬编码）
2. config.json             ← Python 兼容（工作目录）
3. config.toml             ← 主配置（%APPDATA%/GameSaveEditor/）
4. GAME_TOOL_*             ← 最高优先级（环境变量）
```

### merge_from 策略

- 数字类型（u16, usize）：仅当值 ≠ 0 且 ≠ 类型默认值时覆盖
- 字符串类型（String）：仅当非空时覆盖
- 布尔类型（bool）：始终接受覆盖（false 也是有效值）

## 内存扫描引擎

### 扫描流程

```
首次扫描:
  用户输入值 → FirstScan(value, type_id)
    → 枚举所有可读写内存区域 (VirtualQueryEx)
    → 读取每个区域的内容 (ReadProcessMemory)
    → 匹配与输入值类型/大小相同的字节模式
    → 返回所有候选地址

二次扫描:
  选择模式 → NextScan(mode_id, value)
    → 重新读取所有候选地址的当前值
    → 根据模式过滤: 精确值/增大/减小/未变/已变
    → 返回过滤后的候选地址

存档种子扫描:
  SeedFromSave(save_fields)
    → 遍历存档中的字段值
    → 在内存中搜索匹配的值
    → 返回 FieldScanSeed (含候选地址和置信度)
```

### 值类型编码

| ValueType | 字节大小 | 说明 |
|-----------|:---:|------|
| I32 | 4 | 32 位有符号整数 |
| I64 | 8 | 64 位有符号整数 |
| F32 | 4 | 32 位浮点数 |
| F64 | 8 | 64 位浮点数 |
| String(N) | N | 固定长度字符串 |
| Bytes(N) | N | 固定长度字节数组 |
