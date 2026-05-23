# 工具箱完整性检查 + 代码优化 设计文档

**日期**: 2026-05-24  
**状态**: 待实现  
**关联**: 替换 `toolbox.rs` 中的两个空占位功能 + 4 项代码优化

---

## 一、功能需求

### 1.1 存档信息查看器

快速识别存档格式和元数据，不深入解析内容。

| 项目 | 说明 |
|------|------|
| 输入 | 点击"选择文件"按钮，通过文件对话框选择存档 |
| 输出 | 格式名称、引擎标识、文件大小、修改时间、是否有效、错误信息（如有） |
| 自动检测 | 根据文件扩展名和魔数自动识别引擎类型 |

### 1.2 存档完整性检查

深度格式校验 + 数据逻辑检查。

| 项目 | 说明 |
|------|------|
| 输入 | 选择存档文件 |
| 输出 | 格式校验结果（通过/失败）、字段数量、摘要信息、警告/错误列表 |
| 格式校验 | 各引擎独立校验：RPG Maker(Base64→LZString→JSON)、Ren'Py(ZIP→JSON)、Unreal(GVAS魔数+头部)、Generic(JSON) |
| 逻辑检查 | gold >= 0、hp 0-9999、level 1-99、switches 为 bool、variables 为 number |

**数据逻辑检查规则**：

| 规则 | 引擎 | 严重性 |
|------|------|:---:|
| gold < 0 | 全部 | 错误 |
| gold > 99999999 | 全部 | 警告 |
| hp < 0 或 hp > 9999 | RPG Maker | 警告 |
| mp < 0 或 mp > 9999 | RPG Maker | 警告 |
| level < 1 或 level > 99 | RPG Maker | 警告 |
| switch 值非 bool | RPG Maker | 错误 |
| variable 值非 number | RPG Maker | 错误 |
| 必需字段缺失 (party/switches/variables) | RPG Maker | 错误 |

### 1.3 批量完整性检查

| 项目 | 说明 |
|------|------|
| 输入 | 选择目录 |
| 输出 | 递归扫描所有存档文件 → 表格列出：文件名、格式、是否有效、问题描述 |
| 过滤 | 跳过 .bak 文件、系统文件（config.rpgsave、global.rpgsave） |

### 1.4 存档修复工具

| 项目 | 说明 |
|------|------|
| 输入 | 选择损坏的 RPG Maker 存档文件 |
| 输出 | 修复成功/失败，生成 `{原名}_repaired.{ext}` 不覆盖原文件 |
| 策略 | (1)Base64 padding 补齐 (2)去除非 Base64 字符后重试 (3)尝试修复不完整 JSON |

---

## 二、架构设计

### 2.1 核心逻辑：`crates/core/src/integrity.rs`（新建）

核心数据类型：

```rust
/// 完整性检查结果
pub struct IntegrityResult {
    pub file_path: String,
    pub format_name: String,       // "RPG Maker MV/MZ"
    pub engine: String,            // "rpg_mv"
    pub is_valid: bool,
    pub file_size: u64,
    pub modified: String,          // ISO 8601
    pub errors: Vec<String>,       // 格式/逻辑错误
    pub warnings: Vec<String>,     // 数据逻辑警告
    pub summary: Option<SaveSummary>,
    pub field_count: usize,
}

/// 存档文件快速信息
pub struct SaveFileInfo {
    pub file_path: String,
    pub format_name: String,
    pub engine: String,
    pub file_size: u64,
    pub modified: String,
    pub is_valid: bool,
    pub error: Option<String>,
}

/// 修复结果
pub struct RepairResult {
    pub success: bool,
    pub repaired_path: Option<String>,
    pub original_errors: Vec<String>,
    pub remaining_errors: Vec<String>,
}
```

公开 API：

```rust
pub fn get_save_info(filepath: &str) -> SaveFileInfo;
pub fn check_save_integrity(filepath: &str) -> IntegrityResult;
pub fn batch_check_saves(dir: &str) -> Vec<IntegrityResult>;
pub fn attempt_repair(filepath: &str) -> RepairResult;
```

检测逻辑：复用 `detector.rs` 的 `detect_by_filesystem()` 判断引擎，各引擎校验内联实现。

### 2.2 GUI 状态：`crates/gui/src/state.rs`

`ToolboxState` 新增字段：

```rust
pub struct ToolboxState {
    // --- 现有字段 ---
    pub lz_input: String,
    pub lz_output: String,
    pub lz_error: String,
    pub b64_input: String,
    pub b64_output: String,

    // --- 存档信息查看器 ---
    pub info_path: String,
    pub info_result: Option<SaveFileInfo>,

    // --- 完整性检查 ---
    pub check_path: String,
    pub check_result: Option<IntegrityResult>,
    pub check_running: bool,

    // --- 批量检查 ---
    pub batch_dir: String,
    pub batch_results: Vec<IntegrityResult>,
    pub batch_running: bool,

    // --- 存档修复 ---
    pub repair_path: String,
    pub repair_result: Option<RepairResult>,
    pub repair_running: bool,
}
```

新增 `ToolboxAction` 枚举：

```rust
pub enum ToolboxAction {
    GetSaveInfo(String),
    IntegrityCheck(String),
    BatchCheck(String),
    RepairSave(String),
    ClearCheck,
    ClearBatch,
    ClearRepair,
}
```

### 2.3 GUI 面板：`crates/gui/src/panels/toolbox.rs`

**函数签名变更**：
```rust
// 旧: pub fn render(ui: &mut egui::Ui, state: &mut ToolboxState)
// 新: pub fn render(ui: &mut egui::Ui, state: &mut ToolboxState) -> Vec<ToolboxAction>
```

**UI 布局**（6个区块）：

```
工具箱
├── LZString 压缩/解压        ← 保持现有 UI
├── Base64 编解码             ← 保持现有 UI
├── 存档信息查看器             ← 新增：[选择文件] [查看] → 格式/大小/时间/有效状态
├── 存档完整性检查             ← 替换空占位：[选择文件] [检查] → 通过/失败 + 错误/警告列表
├── 批量完整性检查             ← 替换空占位：[选择目录] [扫描] → 结果表格
└── 存档修复工具               ← 新增：[选择文件] [修复] → 成功/失败 + 修复后路径
```

### 2.4 App 集成：`crates/gui/src/app.rs`

在 `update()` 的 `TabMode::Toolbox` 分支添加动作分发：

```rust
TabMode::Toolbox => {
    let actions = toolbox::render(ui, &mut self.toolbox);
    for action in actions {
        match action {
            ToolboxAction::GetSaveInfo(path) => {
                self.toolbox.info_result = Some(get_save_info(&path));
            }
            ToolboxAction::IntegrityCheck(path) => {
                self.toolbox.check_result = Some(check_save_integrity(&path));
            }
            ToolboxAction::BatchCheck(dir) => {
                self.toolbox.batch_results = batch_check_saves(&dir);
            }
            ToolboxAction::RepairSave(path) => {
                self.toolbox.repair_result = Some(attempt_repair(&path));
            }
            ToolboxAction::ClearCheck => { self.toolbox.check_result = None; }
            ToolboxAction::ClearBatch => { self.toolbox.batch_results.clear(); }
            ToolboxAction::ClearRepair => { self.toolbox.repair_result = None; }
        }
    }
}
```

---

## 三、代码优化

### 3.1 修复 Unicode 转义 → 实际中文

**文件**：`crates/gui/src/app.rs`

**问题**：约 50 处使用 `\u{xxxx}` 表示中文字符，严重降低可读性。

**方案**：全部替换为实际中文字符。示例：

```rust
// 旧:
"\u{52a0}\u{8f7d}\u{5b58}\u{6863}\u{5931}\u{8d25}: {}"

// 新:
"加载存档失败: {}"
```

**影响文件**：仅 `app.rs`，约 50 处替换。

### 3.2 修复缩进问题

**文件**：`crates/gui/src/app.rs:114`

**问题**：
```rust
// 当前（缩进错位）
                    engine: engine.clone(),
```

**方案**：修正为正常缩进：
```rust
            engine: engine.clone(),
```

### 3.3 清理 is_readonly() 死代码

**文件**：`crates/gui/src/factory.rs`

**问题**：`is_readonly()` 函数总是返回 `false`，参数使用 `_` 前缀但无文档说明。

**方案**：移除该函数，同步移除所有调用点和测试用例。将所有 `is_readonly()` 调用替换为 `false`。保留 `engine` 到 `SavePanelMode` 的映射作为替代方案（如需未来启用只读）。

### 3.4 拆分 AppState::new()

**文件**：`crates/gui/src/app.rs`

**问题**：`new()` 方法 175 行，含深度嵌套的 if/else，难以阅读和维护。

**方案**：提取 3 个私有辅助函数：

```rust
impl AppState {
    pub fn new(game_dir: Option<String>) -> Self {
        let config = load_config().unwrap_or_default();
        let engine = detect_engine(&game_dir);
        let game_config = load_game_config(&game_dir, &engine);
        let save_panel = init_save_panel(&game_dir, &engine);
        let rt_panel = init_rt_panel(&game_dir, &engine, &config);
        // ... 组装
    }

    fn detect_engine(game_dir: &Option<String>) -> EngineType { ... }
    fn load_game_config(game_dir: &Option<String>, engine: &EngineType) -> Option<GameConfig> { ... }
    fn init_save_panel(game_dir: &Option<String>, engine: &EngineType) -> SavePanelState { ... }
    fn init_rt_panel(game_dir: &Option<String>, engine: &EngineType, config: &AppConfig) -> RtPanelState { ... }
}
```

**效果**：`new()` 从 175 行缩减到 ~30 行。

---

## 四、涉及文件清单

| 文件 | 操作 | 说明 |
|------|:---:|------|
| `crates/core/src/integrity.rs` | 新建 | 完整性检查/修复/信息核心逻辑 |
| `crates/core/src/lib.rs` | 修改 | 新增 `pub mod integrity` |
| `crates/gui/src/state.rs` | 修改 | 扩展 `ToolboxState` + 新增 `ToolboxAction` |
| `crates/gui/src/panels/toolbox.rs` | 重写 | 新 UI + 动作返回 |
| `crates/gui/src/app.rs` | 修改 | 工具箱动作分发 + Unicode→中文 + 拆分 new() |
| `crates/gui/src/factory.rs` | 修改 | 移除 `is_readonly()` |
| `README.md` | 修改 | 更新功能描述 |
| `docs/ARCHITECTURE.md` | 修改 | 更新工具箱描述 |

---

## 五、测试策略

- `integrity.rs`: 单元测试覆盖各引擎格式的有效/无效存档、边界情况、修复逻辑
- `toolbox.rs`: 不涉及单元测试（纯 UI 代码），通过手动测试验证
- `app.rs` 优化: 现有测试套件 `cargo test -p game-tool-gui` 全部通过
- 回归测试: `cargo build --release -p game-tool-gui` 确保编译通过

---

## 六、命令参考

```bash
# 构建验证
cargo build -p game-tool-gui
cargo test -p game-tool-core
cargo test -p game-tool-gui
cargo clippy --workspace
cargo fmt --check --all
```
