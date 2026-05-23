# GameSaveEditor

跨引擎游戏存档编辑器 — 支持 RPG Maker MV/MZ、Ren'Py、Unreal Engine、Unity、Godot。

## 特性

- **存档编辑** — 加载存档文件，可视化编辑金币、开关、变量、角色、物品等字段
- **实时修改** — 通过 TCP 桥接插件连接运行中游戏，实时读写内存数据
- **备份管理** — 一键创建/恢复/删除存档备份，支持差异对比
- **工具箱** — LZString 压缩/解压、Base64 编解码、存档完整性检查
- **多引擎支持** — 自动检测引擎类型，适配不同存档格式
- **暗色/亮色主题** — GitHub 风格配色，即时切换

## 支持引擎

| 引擎 | 存档编辑 | 实时修改 | 格式 |
|------|:---:|:---:|------|
| RPG Maker MV | ✅ | ✅ | JSON + LZString |
| RPG Maker MZ | ✅ | ✅ | JSON + LZString |
| NW.js | ✅ | ✅ | JSON + LZString |
| Ren'Py | ✅ | ✅ | Pickle + JSON |
| Unreal Engine | ✅ (只读) | ❌ | GVAS |
| Unity (Mono) | ✅ | ❌ | 通用 JSON |
| Unity (IL2CPP) | ✅ | ❌ | 通用 JSON |
| Godot | ✅ | ❌ | 通用 JSON |

## 构建

**环境要求：** Rust 1.85+ (stable)

```bash
# 开发构建
cargo build -p game-tool-gui

# Release 构建
cargo build --release -p game-tool-gui

# 或使用构建脚本
build.bat
```

**运行：** `cargo run -p game-tool-gui` 或直接运行 `target/release/GameSaveEditor.exe`

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
dark_mode = true           # 暗色模式
recent_games = []          # 最近打开的游戏目录
```

首次运行无需配置文件，所有选项有默认值。

## 技术栈

- **语言：** Rust (edition 2021)
- **GUI：** egui 0.31 + eframe 0.31
- **序列化：** serde / serde_json / toml
- **存档解析：** RPG Maker JSON+LZString、Ren'Py Pickle、Unreal GVAS
