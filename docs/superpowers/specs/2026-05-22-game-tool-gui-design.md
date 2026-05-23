# GameSaveEditor GUI 设计方案

日期: 2026-05-22 | 版本: v3 | 状态: 已确认

---

## 一、目标

为 `game_tool` 项目增加 egui 图形界面，支持双面板布局：
- **左侧**: 存档文件编辑（多引擎自适应）
- **右侧**: 实时内存修改（TCP 桥接）

---

## 二、架构

```
crates/gui/                        # 新增 crate
├── Cargo.toml
└── src/
    ├── main.rs                    # eframe 入口 + 启动时文件夹选择
    ├── app.rs                     # eframe::App impl, AppState, 双面板布局
    ├── factory.rs                 # 格式/Bridge 工厂 + GameState→Field 转换 + 只读标记
    ├── discovery.rs               # 存档文件发现 + 引擎检测封装
    ├── connection.rs              # RealtimeConnection + 后台线程 + channel
    ├── panels/
    │   ├── save_panel.rs          # 存档编辑
    │   ├── realtime_panel.rs      # 实时修改
    │   └── top_bar.rs             # 顶部: 游戏路径/引擎/切换按钮
    ├── widgets/
    │   ├── field_table.rs         # 通用可编辑字段表格
    │   ├── category_tree.rs       # RPG Maker 分类树
    │   ├── search_bar.rs          # 实时搜索过滤
    │   └── summary_card.rs        # 存档摘要卡片
    └── state.rs                   # AppState, SavePanelState, RtPanelState
```

**原则**: 现有 crates（core/app/engines）不修改代码，仅 `crates/app/Cargo.toml` 二进制名改 `game-tool-cli` 避免 workspace 冲突。

---

## 三、主界面布局

```
┌── GameSaveEditor ───────────────────────────────────────────────────┐
│ [游戏: D:\Games\MyGame] [引擎: RPG Maker MV] [切换游戏...]           │
├────────────────────────────────┬─────────────────────────────────────┤
│  存档编辑                       │  实时修改                            │
│ ┌─ 存档文件 ──────────────────┐│ ┌─ 连接状态 ───────────────────────┐ │
│ │ 存档: [file1.rpgsave v] [↻]││ │ ○ 已连接  端口 [19999]  [断开]  │ │
│ └────────────────────────────┘│ └─────────────────────────────────┘ │
│ ┌─ 存档摘要 ──────────────────┐│ ┌─ 视图切换 ───────────────────────┐ │
│ │ 金币 5000  队伍 4人          ││ │ [精简监控]  [完整字段]            │ │
│ │ 物品 27种  时长 01:23:45     ││ └─────────────────────────────────┘ │
│ └────────────────────────────┘│ ┌─ 精简视图 ───────────────────────┐ │
│ [搜索_________________]        ││ │ 金币 [5000  ][+][-][🔒]         │ │
│ ┌──────┬──────────────────────┐│ │ 药水 [12    ][+][-][🔒]         │ │
│ │ 分类  │ 名称          值    ││ └─────────────────────────────────┘ │
│ │ 金币 │ 金币        5000─── ││ ┌─ 插件 ────────────────────────────┐ │
│ │ 开关 │ 开关1:门    ON───── ││ │ [注入插件到游戏目录]              │ │
│ │ 变量 │ 步数        42───── ││ │ 状态: 插件已安装 ✓               │ │
│ │ 角色 │ Alice HP    100──── ││ └──────────────────────────────────┘ │
│ │ 物品 │ 药水        10───── ││                                      │
│ └──────┴──────────────────────┘│                                      │
│                                │                                      │
│ [保存更改]  ⚠ 有 3 处未保存     │                                      │
├────────────────────────────────┴──────────────────────────────────────┤
│ 存档已加载 | 共 156 个字段                                              │
└───────────────────────────────────────────────────────────────────────┘
```

---

## 四、数据流

### 4.1 存档编辑 —— 主线程同步

```
启动/切换游戏
  → factory::create_format(engine) → Box<dyn ISaveFormat>
  → discovery::find_save_files(game_dir, &*format) → Vec<String>
  → 用户选择存档文件
  → format.load(path) → Value (存入 save_data)
  → format.get_summary(&save_data) → SaveSummary (显示摘要卡片)
  → format.scan_fields(&save_data, game_dir) → Vec<ModifiableField>
  → 用户编辑字段 → field.dirty = true
  → 用户点击"保存更改"
     → 对每个 dirty field: format.apply_field(&mut save_data, &field)
     → format.save(filepath, &save_data)  // 自动备份
     → 清理 dirty 标记
```

**切换存档保护**:
```
用户切换存档时:
  if has_dirty_fields:
    → 弹窗 "有 N 处未保存的修改。"
      [保存并切换] [丢弃修改] [取消]
```

### 4.2 实时修改 —— 后台线程 + Channel

```
GUI 线程                          后台线程
─────────                         ────────
BridgeJob::Connect
  → cmd_tx.send() ────────────→  recv()
                                   create_bridge(engine)
                                   bridge.connect()
                                   result_tx.send(Connected) ──┐
  ← try_recv() ──────────────────────────────────────────────┘
  状态 → Connected
  ctx.request_repaint()

BridgeJob::Execute(ReadAll)
  → cmd_tx.send() ────────────→  recv()
                                   bridge.execute(ReadAll)
                                   result_tx.send(CommandResult(state))
  ← try_recv() ──────────────────────────────────────────────┘
  将 GameState 转换为 rt_fields
  ctx.request_repaint()

BridgeJob::Execute(WriteField("gold", 9999))
  → cmd_tx.send() ────────────→  recv()
                                   bridge.execute(WriteField)
                                   result_tx.send(CommandResult(ok))
  ← try_recv() ──────────────────────────────────────────────┘
  更新本地 rt_field 值

窗口关闭:
  drop(cmd_tx) ────────────────→  recv() → Err → break
                                   bridge.disconnect()
                                   thread exits (最多 5s)
```

**关键**: 后台线程从不调用 `request_repaint()`，由 GUI 线程 drain channel 后自行调用。GUI 模式将 `TcpLineConnection` 读超时覆盖为 5 秒（默认 30 秒）。

### 4.3 插件注入流程

```
实时面板首次使用:
  检测插件存在?
    ├─ 未安装 → "插件未安装" + [注入插件] 按钮
    │            点击 → inject_plugin(game_dir) → [连接] 按钮激活
    └─ 已安装 → "插件已就绪" + [连接] 按钮
```

---

## 五、核心数据结构

```rust
struct AppState {
    game_dir: Option<String>,
    game_title: String,
    engine: EngineType,
    status_message: String,
    config: AppConfig,
    save_panel: SavePanelState,
    rt_panel: RtPanelState,
}

struct SavePanelState {
    format: Option<Box<dyn ISaveFormat>>,
    save_files: Vec<String>,
    selected_save: Option<String>,
    save_data: Option<Value>,
    summary: Option<SaveSummary>,
    fields: Vec<ModifiableField>,
    dirty_count: usize,
    selected_category: Option<String>,
    search_query: String,
    panel_mode: SavePanelMode,
    readonly: bool,                       // true for Unreal
}

enum SavePanelMode {
    RpgMaker,   // 分类树 + 表格
    RenPy,      // 摘要卡片 + 少量字段
    Unreal,     // GVAS 属性表格 (只读)
    Generic,    // 扁平键值表格
}

struct RtPanelState {
    conn: Option<RealtimeConnection>,
    fields: Vec<ModifiableField>,
    view_mode: RtViewMode,
    plugin_installed: bool,
    host: String,
    port: u16,
}

enum RtViewMode { Compact, Full }

struct RealtimeConnection {
    cmd_tx: Sender<BridgeJob>,
    result_rx: Receiver<BridgeResult>,
    status: ConnectionStatus,
}

enum BridgeJob {
    Connect { host: String, port: u16 },
    Disconnect,
    Execute(BridgeCommand),
}

enum BridgeResult {
    Connected,
    Disconnected,
    CommandResult(serde_json::Value),
    Error(String),
}

enum ConnectionStatus { Disconnected, Connecting, Connected }
```

---

## 六、自适应引擎面板

| 引擎 | `SavePanelMode` | 字段特征 | UI 组件 | 可写 |
|------|----------------|---------|---------|------|
| RPG Maker MV/MZ | `RpgMaker` | 6类（金/开关/变量/角色/物品/自开关） | CategoryTree + FieldTable | ✓ |
| Ren'Py | `RenPy` | 约3个 meta 字段 | SummaryCard + FieldTable | ✓ |
| Unreal | `Unreal` | GVAS 属性列表 | FieldTable | ✗只读(1) |
| Generic JSON | `Generic` | 扁平 dot-notation 键值 | SearchBar + FieldTable | ✓ |

(1) Unreal 的 `save()` 写回原始 `_raw` 字节，`apply_field()` 修改的 `_props` 不会持久化。待后续修复格式层后改为可写。

---

## 七、实时引擎兼容性

| 引擎 | TCP 桥接 | 插件注入 | 读写 |
|------|---------|---------|------|
| RPG Maker MV/MZ | ✓ RpgMakerTcpBridge | GameBridgeServer.js | 完整 |
| Ren'Py | ✓ RenPyBridge | tcp_bridge/\_\_init\_\_.py | 完整 |
| Unreal | ✗ | - | 灰显 |
| Generic/Unity/Godot | ✗ | - | 灰显 |

---

## 八、GameState → Field 转换

```rust
// factory.rs
fn game_state_to_fields(state: &GameState, engine: &EngineType) -> Vec<ModifiableField> {
    match engine {
        EngineType::RpgMakerMv | EngineType::RpgMakerMz | EngineType::NwJs => {
            // extensions["gold"] → gold field
            // extensions["switches"] → switch_* fields
            // extensions["variables"] → var_* fields
            // extensions["party"] → actor_* fields
            // extensions["items"] → item_* fields
        }
        EngineType::RenPy => {
            // extensions["store"] → 遍历所有 store 变量为 field
        }
        _ => vec![]
    }
}
```

---

## 九、错误处理

| 场景 | 处理 |
|------|------|
| 引擎检测失败 | 弹窗 "未识别游戏引擎，是否尝试通用模式？" |
| 存档解析失败 | 红色状态栏 + 存档文件旁红叉 |
| TCP 连接超时/拒绝 | 状态变红 + 5秒恢复 |
| TCP 读取超时 | 状态变黄 "响应超时"，可重试 |
| 字段写入失败 | 单元格红色闪烁 1s + 状态栏 |
| 窗口关闭时 TCP 活跃 | drop(cmd_tx) → 线程退出 (最多 5s) |
| 无存档文件 | "未找到存档文件" |
| 无实时支持 | 实时面板灰显 "该引擎暂不支持实时修改" |
| 金币变量不一致 | tooltip: "游戏可能使用变量 #N 存储金币" |
| 切换存档未保存 | 弹窗 [保存并切换] [丢弃] [取消] |
| Unreal 编辑操作 | 禁用编辑控件 + tooltip "当前引擎仅提供只读预览" |

---

## 十、依赖

### 新增 workspace 依赖

```toml
[workspace.dependencies]
eframe = "0.31"
egui = "0.31"
rfd = { version = "0.15", default-features = false, features = ["file-handle-inner"] }
```

### 新增 crate

```toml
# crates/gui/Cargo.toml
[package]
name = "game-tool-gui"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "GameSaveEditor"
path = "src/main.rs"

[dependencies]
eframe.workspace = true
egui.workspace = true
rfd.workspace = true
serde_json.workspace = true
game-tool-core = { path = "../core" }
game-tool-rpgmaker = { path = "../engines/rpgmaker" }
game-tool-renpy = { path = "../engines/renpy" }
game-tool-unreal = { path = "../engines/unreal" }
game-tool-generic = { path = "../engines/generic" }
```

### Workspace 变更

```toml
# Cargo.toml
members = [
    ...
    "crates/gui",
]
```

### 现有文件修改

仅 `crates/app/Cargo.toml` 二进制名改为 `game-tool-cli` 以避免 workspace 构建冲突。

---

## 十一、构建

```bat
:: build.bat 更新
cargo build --release -p game-tool-gui
copy target\release\GameSaveEditor.exe dist\GameSaveEditor.exe
```

---

## 十二、延期特性 (v2)

- 撤销/重做
- 另存为 (Save As)
- 原始 JSON 编辑器
- 虚拟滚动（万级字段）
- Unreal 实时连接
- 多存档同时打开
- 中英文切换 UI
- 自定义窗口图标
