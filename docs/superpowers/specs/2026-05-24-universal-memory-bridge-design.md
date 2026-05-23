# 通用内存桥 + 存档辅助定位 设计文档

**日期**: 2026-05-24
**目标**: 为 Unreal Engine、Unity (Mono/IL2CPP)、Godot 补齐实时内存修改能力

---

## 1. 现状与问题

### 1.1 当前能力矩阵

| 引擎 | 存档编辑 | 实时修改 | 实时方式 |
|------|:---:|:---:|------|
| RPG Maker MV/MZ/NW.js | ✅ | ✅ | TCP + JS 插件注入 |
| Ren'Py | ✅ | ✅ | TCP + Python 插件注入 |
| Unreal Engine | ✅* | ❌ | — |
| Unity (Mono) | ✅ | ❌ | — |
| Unity (IL2CPP) | ✅ | ❌ | — |
| Godot | ✅ | ❌ | — |

\* Unreal 底层 `save()` 已实现，但 `is_readonly()` 错误返回 true，GUI 层阻止了写入。

### 1.2 待解决的 3 个问题

1. **Unreal 存档被锁定为只读** — factory.rs `is_readonly()` 硬编码 true
2. **三引擎缺失实时修改** — 无 `GameBridge` 实现
3. **内存值缺乏语义** — RPM 读到的是裸字节，无法自动对应"金币""HP"等字段名

---

## 2. 设计决策

### 2.1 技术路线：内存直读直写

通过 Windows API `ReadProcessMemory` / `WriteProcessMemory` 直接操作游戏进程内存。

选择理由：
- **通用性** — 不依赖引擎内部结构、不依赖脚本运行时
- **无侵入** — 不需要修改游戏文件或注入插件
- **跨引擎** — 一套实现覆盖 Unreal、Unity IL2CPP、Godot 及任意原生游戏

### 2.2 架构：混合桥接模型

```
                        GameBridge (trait)
                       /        |        \
             RpgMakerTcp   RenPyBridge   UniversalMemoryBridge  ← 新增
              (TCP+JS)     (TCP+Python)   (RPM/WPM)
              不变          不变            覆盖 Unreal/Unity/Godot
```

- RPG Maker / Ren'Py 保持现有 TCP 桥不变（脚本引擎不适合内存方案）
- Unreal / Unity / Godot 使用统一的内存桥
- 两者共用 `GameBridge` trait，GUI 层通过 `BridgeMode` 枚举区分模式

### 2.3 存档辅助定位

纯内存扫描只能找到"地址上的值是 5000"，无法知道是"金币"。利用项目已有的存档解析能力（各引擎 `scan_fields()` 都完善），实现半自动字段名称标注。

---

## 3. 新增 Crate: game_tool_memory

### 3.1 目录结构

```
crates/engines/memory/
├── Cargo.toml
└── src/
    ├── lib.rs          # 模块导出
    ├── process.rs      # 进程枚举 + 句柄管理
    ├── region.rs       # 内存区域 + 类型化读写
    ├── module.rs       # 模块信息
    ├── bridge.rs       # UniversalMemoryBridge
    └── scanner.rs      # 值扫描 + 存档辅助
```

### 3.2 模块设计

见代码实现。

---

## 4. 编辑现有文件

| 文件 | 改动 |
|------|------|
| `Cargo.toml` (workspace) | 添加 `crates/engines/memory` 到 members |
| `core/src/types.rs` | 新增 `ProcessInfo`、`ScannedAddr`、`ScanQuery`、`ValueType`、`FieldScanSeed` 等类型；`BridgeCommand` 扩展 |
| `gui/src/factory.rs` | `is_readonly()` 返回 false；`supports_realtime()` 全引擎；`create_bridge()` 为 Unreal/Unity/Godot 返回内存桥 |
| `gui/src/connection.rs` | `BridgeJob`/`BridgeResult` 扩展内存模式变体 |
| `gui/src/state.rs` | `RtPanelState` 新增 `bridge_mode`、进程列表、扫描器状态 |
| `gui/src/panels/realtime_editor.rs` | 双模式 UI：TCP 模式保持原样，内存模式新增 |
| `gui/src/app.rs` | `drain_rt_results()` 处理新结果变体 |

---

## 5. 最终能力矩阵

| 引擎 | 存档编辑 | 实时修改 | 实时方式 |
|------|:---:|:---:|------|
| RPG Maker MV/MZ/NW.js | ✅ | ✅ | TCP + JS 插件 |
| Ren'Py | ✅ | ✅ | TCP + Python 插件 |
| Unreal Engine | ✅ | ✅ | 内存 RPM/WPM |
| Unity (Mono) | ✅ | ✅ | 内存 RPM/WPM |
| Unity (IL2CPP) | ✅ | ✅ | 内存 RPM/WPM |
| Godot | ✅ | ✅ | 内存 RPM/WPM |
