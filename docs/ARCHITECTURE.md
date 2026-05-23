# GameSaveEditor 技术架构

## Workspace 结构

```
game_tool/
├── crates/
│   ├── core/                    # game-tool-core — 核心数据模型与工具
│   ├── engines/
│   │   ├── rpgmaker/            # game-tool-rpgmaker — RPG Maker 引擎适配
│   │   ├── renpy/               # game-tool-renpy — Ren'Py 引擎适配
│   │   ├── unreal/              # game-tool-unreal — Unreal Engine 引擎适配
│   │   └── generic/             # game-tool-generic — 通用 JSON 格式适配
│   ├── gui/                     # game-tool-gui — egui 桌面 GUI
│   └── app/                     # GameSaveEditor — CLI 入口 (rust-embed)
├── docs/                        # 技术文档
└── Cargo.toml                   # Workspace 配置
```

## Crate 职责

### game-tool-core

核心层，定义数据模型和通用工具。

| 模块 | 文件 | 职责 |
|------|------|------|
| **types** | `types.rs` | `ModifiableField`, `SaveSummary`, `GameState`, `ISaveFormat` trait, `GameBridge` trait, `BridgeCommand` |
| **config** | `config.rs` | `AppConfig` 结构 + 分层加载（默认值 → JSON → TOML → 环境变量） |
| **detector** | `detector.rs` | 文件系统引擎检测 (RPG Maker MV/MZ, Ren'Py, Unreal, Unity, Godot) |
| **net** | `net.rs` | `TcpLineConnection` — 行分隔文本协议的 TCP 连接封装 |
| **lzstring** | `lzstring.rs` | RPG Maker MV 兼容的 LZString 压缩/解压 |
| **base64** | `base64.rs` | Base64 编解码 |
| **backup** | `backup.rs` | 存档备份文件管理 (create/restore/cleanup) |

### 引擎适配层 (engines/)

每个引擎 crate 实现 `ISaveFormat`（存档读写）和/或 `GameBridge`（实时连接）。

| Crate | ISaveFormat | GameBridge | 关键文件 |
|-------|:---:|:---:|------|
| **rpgmaker** | ✅ | ✅ | `format.rs` (JSON+LZString), `tcp.rs` (GameBridgeServer.js 插件), `scanner.rs` (游戏目录扫描) |
| **renpy** | ✅ | ✅ | `format.rs` (Pickle+JSON), `bridge.rs` (TCP 桥接) |
| **unreal** | ✅ (只读) | ❌ | `format.rs` (GVAS 解析) |
| **generic** | ✅ | ❌ | `format.rs` (通用 JSON 扁平化) |

### game-tool-gui

egui/eframe 桌面 GUI。

| 模块 | 文件 | 职责 |
|------|------|------|
| **app** | `app.rs` | 主状态机：`AppState`, `update()`, 选项卡路由、连接生命周期 |
| **state** | `state.rs` | `AppState`, `TabMode`, `SavePanelState`, `RtPanelState`, `ConfirmDialog` |
| **theme** | `theme.rs` | 暗色/亮色主题、颜色常量、`engine_display_name` |
| **panels/** | | |
| | `top_bar.rs` | 顶部标题栏 |
| | `tab_bar.rs` | 选项卡导航栏 (存档/实时/备份/工具/设置) |
| | `startup.rs` | 启动页（游戏目录选择） |
| | `save_editor.rs` | 存档编辑标签（文件选择/分类筛选/字段表） |
| | `realtime_editor.rs` | 实时修改标签（连接管理/分类筛选/字段编辑器） |
| | `backup.rs` | 备份管理标签（创建/恢复/删除/批量/差异对比） |
| | `toolbox.rs` | 工具箱标签（LZString/Base64/完整性检查/目录扫描） |
| | `settings.rs` | 设置标签（主题/端口/最近游戏/配置/关于） |
| | `status_bar.rs` | 底部状态栏（选项卡感知） |
| **widgets/** | | |
| | `field_table.rs` | 可编辑字段网格（含实时值列/差异高亮/跳转） |
| | `category_tree.rs` | 分类筛选器（水平/垂直布局） |
| | `search_bar.rs` | 搜索输入框（含清除按钮） |
| | `summary_card.rs` | 存档摘要卡片 |
| **其他** | | |
| | `factory.rs` | 引擎 → 格式/桥接工厂 |
| | `connection.rs` | Bridge 线程管理 (spawn/drain) |
| | `discovery.rs` | 存档文件递归发现 |
| | `main.rs` | 入口：字体加载、窗口创建 |

### GameSaveEditor (app)

CLI 入口，使用 `rust-embed` 内嵌核心数据。

## 核心数据模型

### ModifiableField

```rust
pub struct ModifiableField {
    pub field_id: String,        // "gold", "switch_12", "actor_1_hp"
    pub category: String,        // "gold", "switch", "variable", "actor"
    pub display_name: String,    // "金币", "大门开关", "艾里克"
    pub field_type: String,      // "int", "bool", "float", "str"
    pub save_value: Value,       // 存档中的原始值
    pub live_value: Value,       // 运行中游戏的实时值
    pub dirty: bool,             // 是否已修改
    pub locked: bool,            // 实时刷新时是否锁定
    pub item_id: i32,            // RPG Maker 中用于分页
    pub min_val: i32,
    pub max_val: i32,
}
```

### 字端提取流程

```
GameState (bridge response 或 save data)
    → factory::game_state_to_fields()
        → 每个引擎各自的 extract_fields():
            gold → ModifiableField { category: "gold", field_type: "int", ... }
            switches → ModifiableField { category: "switch", field_type: "bool", ... }
            variables → ModifiableField { category: "variable", field_type: "int", ... }
            actors → ModifiableField { category: "actor", field_type: "int", ... }
            items → ModifiableField { category: "item", field_type: "int", ... }
```

## 实时连接协议

采用**行分隔文本协议**（非 JSON），用于桥接插件和编辑器：

### 命令（客户端 → 游戏插件）

```
get_state              — 读取全部游戏状态
set_gold 5000          — 设置金币
set_switch 12 1        — 设置开关 (ID, 0/1)
set_variable 10 500    — 设置变量 (ID, VALUE)
set_hp 1 100           — 设置角色 HP (ACTOR_ID, VALUE)
set_mp 1 50            — 设置角色 MP
set_level 1 15         — 设置角色等级
set_item 5 10          — 设置物品数量
set_self_switch A 1 1  — 设置自开关
```

### 响应（游戏插件 → 客户端）

```
STATE:{"gold":5000,"switches":{...},"variables":{...},...}  — 状态数据
OK    — 写入成功
ERR   — 命令错误
```

### 连接生命周期

```
[连接按钮] → rt_connect() → spawn_bridge_thread()
    → TCP connect → BridgeResult::Connected
    → 自动发送 ReadAll
    → [drain_rt_results() 每帧拉取]
    → 定时自动刷新 (configurable 1/2/3/5s)
    → [断开按钮] → rt_disconnect()
```

## GUI 架构

### 布局模型

```
┌──────────────────────────────────────────┐
│ TOP BAR    GameSaveEditor    游戏: 冒险传说  │  TopBottomPanel::top
├──────────────────────────────────────────┤
│ [📂存档] [⚡实时] [🗄备份] [🧰工具] [⚙设置]  │  TopBottomPanel::top (tab bar)
├──────────────────────────────────────────┤
│                                          │
│   当前选项卡内容 (CentralPanel)                │
│                                          │
├──────────────────────────────────────────┤
│ STATUS    修改数 | 字段数 | 文件名              │  TopBottomPanel::bottom
└──────────────────────────────────────────┘
```

### 选项卡模式

| TabMode | 不需游戏 | 说明 |
|---------|:---:|------|
| SaveEditor | ❌ | 存档文件选择 + 字段网格编辑 |
| RealtimeEditor | ❌ | TCP 连接管理 + 实时字段编辑 |
| BackupManager | ❌ | 备份创建/恢复/删除/差异对比 |
| Toolbox | ✅ | LZString/Base64 独立工具 |
| Settings | ✅ | 主题/端口/最近游戏配置 |

### 数据流

```
用户编辑字段 → field.dirty = true → dirty_count++
    → [保存] → format.apply_field() → format.save() → 写入磁盘
    → dirty_count = 0

用户连接 → bridge thread → BridgeResult::Connected
    → ReadAll → GameState → game_state_to_fields() → rt_panel.fields
    → [编辑实时值] → WriteField → TCP 发送 → 游戏即时更新
    → [锁定] → 刷新时跳过该字段
```
