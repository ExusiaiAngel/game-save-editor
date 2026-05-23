# 实时编辑器 7 项修复计划

日期: 2026-05-23 | 版本: v1

---

## Fix 1: 移除 app.rs 重复搜索栏

**文件：** `crates/gui/src/app.rs`，删除实时编辑器区域中 lines ~704-719 的搜索+跳转ID块：

```rust
// 删除这个完整块
ui.horizontal(|ui| {
    ui.label("🔍");
    ui.add(TextEdit::singleline(&mut self.rt_panel.search_query)...);
    ...
    ui.label("跳转 ID:");
    ui.text_edit_singleline(&mut self.rt_panel.jump_id);
});
ui.separator();
```

只保留 `realtime_editor::render()` 内部自己管理的搜索栏。

---

## Fix 2+3: 添加分类筛选 + selected_category 字段

**文件 A — `crates/gui/src/state.rs`：** `RtPanelState` 添加：
```rust
pub selected_category: Option<String>,
```
`AppState::new()` 中初始化：`selected_category: None,`

**文件 B — `crates/gui/src/panels/realtime_editor.rs`：** 在搜索栏上方添加：
```rust
category_tree::render_horizontal(ui, &rt_panel.fields, &mut rt_panel.selected_category);
```
`realtime_editor.rs` 需新增 `use crate::widgets::category_tree;`

---

## Fix 4: 按分类筛选字段分组

**文件：** `crates/gui/src/panels/realtime_editor.rs`

当前 by_category 构造不受 `selected_category` 影响。在分组前添加过滤：若有 selected_category 则只保留该分类的字段：

```rust
let filtered_indices: Vec<usize> = (0..rt_panel.fields.len())
    .filter(|i| {
        if let Some(ref cat) = rt_panel.selected_category {
            rt_panel.fields[*i].category == *cat
        } else {
            true
        }
    })
    .collect();
```

然后用 `filtered_indices` 替代 `0..rt_panel.fields.len()` 构建 by_category 分组。

---

## Fix 5: ReadAll 死代码处理

可简单从 `RtAction` 枚举移除 `ReadAll` 变体（已由 app.rs 的自动刷新和手动刷新按钮覆盖）。或保留但注释说明由 app.rs 直接调用 `rt_send_command`。

保守处理：移除 `ReadAll` 变体（无功能损失），清理对应 match 分支。

---

## Fix 6: 反馈倒计时改用真实时间

**文件 A — `crates/gui/src/state.rs`：** `RtPanelState` 中：
```rust
// 改前
pub error_remaining: u32,
pub write_feedback_remaining: u32,
// 改后
pub error_shown_at: Option<std::time::Instant>,
pub write_feedback_shown_at: Option<std::time::Instant>,
```

**文件 B — `crates/gui/src/app.rs`：** 初始化 `None`。在 `drain_rt_results()` 中设值：
```rust
// 设置时
self.rt_panel.error_shown_at = Some(std::time::Instant::now());
self.rt_panel.write_feedback_shown_at = Some(std::time::Instant::now());
```

**清除逻辑：**
```rust
// 改前（帧计数）
if self.rt_panel.error_remaining > 0 { self.rt_panel.error_remaining -= 1; }

// 改后（真实时间，5秒后清除）
if let Some(at) = self.rt_panel.error_shown_at {
    if at.elapsed() > Duration::from_secs(5) {
        self.rt_panel.error_message.clear();
        self.rt_panel.error_shown_at = None;
    }
}
```

同理处理 `write_feedback_shown_at`（3 秒）。

---

## Fix 7: 状态栏添加差异统计

**文件：** `crates/gui/src/panels/status_bar.rs`

保存字段来自 `state.save_panel.fields`，实时字段来自 `state.rt_panel.fields`。计算差异：

```rust
let diff_count = state.rt_panel.fields.iter()
    .filter(|lf| {
        state.save_panel.fields.iter()
            .any(|sf| sf.field_id == lf.field_id && sf.save_value != lf.live_value)
    })
    .count();

if diff_count > 0 {
    ui.colored_label(colors::WARNING, format!("\u{5dee}\u{5f02} {}\u{9879}", diff_count));
}
```

---

## 验证

1. `cargo build -p game-tool-gui 2>&1` — 零错误
2. `cargo test -p game-tool-gui 2>&1` — 全通过
3. 手动检查：实时编辑器只显示一个搜索栏、分类筛选可用、状态栏显示差异数
