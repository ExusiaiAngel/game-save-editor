# GameSaveEditor

跨引擎游戏存档编辑器 — 支持 RPG Maker MV/MZ、Ren'Py、Unreal Engine、Unity、Godot。

## 特性

- **存档编辑** — 加载存档文件，可视化编辑金币、开关、变量、角色、物品等字段
- **实时修改** — 通过 TCP 桥接插件连接运行中游戏，实时读写内存数据
- **备份管理** — 一键创建/恢复/删除存档备份，支持差异对比
- **工具箱** — LZString 压缩/解压、Base64 编解码、存档信息查看、完整性检查、批量扫描、存档修复
- **多引擎支持** — 自动检测引擎类型，适配不同存档格式
- **内存扫描** — 支持 Unreal/Unity/Godot 进程内存扫描与编辑
- **暗色/亮色主题** — GitHub 风格配色，即时切换

## 支持引擎

| 引擎 | 存档编辑 | 实时修改 | 内存扫描 | 格式 |
|------|:---:|:---:|:---:|------|
| RPG Maker MV | ✅ | ✅ | ❌ | JSON + LZString |
| RPG Maker MZ | ✅ | ✅ | ❌ | JSON + LZString |
| NW.js | ✅ | ✅ | ❌ | JSON + LZString |
| Ren'Py | ✅ | ✅ | ❌ | Pickle + JSON |
| Unreal Engine | ✅ | ❌ | ✅ | GVAS |
| Unity (Mono) | ✅ | ❌ | ✅ | 通用 JSON |
| Unity (IL2CPP) | ✅ | ❌ | ✅ | 通用 JSON |
| Godot | ✅ | ❌ | ✅ | 通用 JSON |

## 构建

**环境要求：** Rust 1.85+ (stable)

```bash
# 开发构建
cargo build -p game-tool-gui

# Release 构建
cargo build --release -p game-tool-gui

# 或使用 Windows 构建脚本
build.bat
```

**运行：** `cargo run -p game-tool-gui` 或直接运行 `target/release/GameSaveEditor.exe`

### CI/CD

项目使用 GitHub Actions 自动构建和测试：

- **Push/PR**: 自动运行 `build`、`test`、`clippy`、`fmt` 检查
- **Tag v\***: 自动 release 构建，上传 Windows 可执行文件

详细配置见 `.github/workflows/ci.yml`。

## 使用方法

### 存档编辑

1. 启动程序，点击 **"打开游戏目录"** 选择游戏文件夹
2. 切换到 **存档** 标签页
3. 下拉选择存档文件，自动加载字段列表
4. 编辑字段值 → 点击 **"💾 保存"** 写入磁盘

### 实时修改

1. 在 **实时** 标签页点击 **"注入插件"**（首次使用）
2. 启动游戏（确保游戏加载了注入的插件）
3. 点击 **"● 连接"**，连接成功后自动获取游戏状态
4. 编辑实时值即时生效，可锁定字段防止自动刷新覆盖

### 备份管理

1. 加载存档后切换到 **备份** 标签页
2. 点击 **"创建备份"** 保存当前存档副本
3. 可恢复、删除、批量操作、对比两个备份的差异

## 配置

配置文件位于 `%APPDATA%/GameSaveEditor/config.toml`：

```toml
tcp_port = 19999          # 实时连接端口
cdp_port = 9222           # Chrome DevTools 协议端口
backup_keep = 10          # 备份保留数量
language = "zh-CN"        # 界面语言
plugin_auto_connect = true # 自动连接插件
dark_mode = false          # 暗色模式
recent_games = []          # 最近打开的游戏目录
```

配置加载优先级（从低到高）：
1. **默认值** — 硬编码在 `AppConfig::default()` 中
2. **config.json** — Python 兼容的 JSON 配置文件（工作目录）
3. **config.toml** — 主配置文件（`%APPDATA%/GameSaveEditor/`）
4. **环境变量** — `GAME_TOOL_*` 前缀（最高优先级）

支持的环境变量：

| 变量 | 类型 | 说明 |
|------|------|------|
| `GAME_TOOL_TCP_PORT` | u16 | TCP 桥接端口 |
| `GAME_TOOL_CDP_PORT` | u16 | CDP 调试端口 |
| `GAME_TOOL_BACKUP_KEEP` | usize | 备份保留数量 |
| `GAME_TOOL_LANGUAGE` | string | 界面语言 |
| `GAME_TOOL_PLUGIN_AUTO_CONNECT` | bool | 自动连接开关 |

首次运行无需配置文件，所有选项有默认值。

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
STATE:{...}   — 完整游戏状态 JSON
OK            — 写入成功
ERR           — 命令错误
```

## 项目架构

```
game_tool/
├── crates/
│   ├── core/                    # 核心数据模型与工具库
│   ├── engines/
│   │   ├── rpgmaker/            # RPG Maker MV/MZ 引擎适配
│   │   ├── renpy/               # Ren'Py 引擎适配
│   │   ├── unreal/              # Unreal Engine 引擎适配
│   │   ├── generic/             # 通用 JSON 格式适配
│   │   └── memory/              # Windows 进程内存扫描
│   ├── gui/                     # egui/eframe 桌面 GUI
│   └── app/                     # CLI 测试入口
├── docs/                        # 技术文档
├── profiles/                    # 游戏内存扫描配置文件
├── tests/                       # 集成测试
└── Cargo.toml                   # Workspace 配置
```

详细技术文档见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 技术栈

- **语言：** Rust (edition 2021)
- **GUI：** egui 0.31 + eframe 0.31（即时模式）
- **序列化：** serde / serde_json / toml
- **存档解析：** RPG Maker JSON+LZString、Ren'Py Pickle+JSON、Unreal GVAS
- **内存操作：** windows-sys 0.59 (Win32 API)
- **压缩：** lz-str 0.2、zip 2
- **网络：** tokio (full)、TcpStream
- **并行：** rayon 1
- **日志：** tracing / tracing-subscriber

## 许可证

本项目仅供学习研究使用。
