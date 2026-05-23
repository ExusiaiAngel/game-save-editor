# 插件注入安全修复计划

日期: 2026-05-23 | 版本: v1

---

## 问题

插件注入到游戏后，游戏启动黑屏。根因是 `plugins.js` 修改逻辑使用字符串 `replace("];", ...)` 和脆弱的行匹配策略，在特定情况下损坏文件结构。

## 修复范围：5 项改进

| # | 修复 | 文件 | 影响 |
|---|------|------|------|
| 1 | 安全 JSON 方式修改 plugins.js | `tcp.rs:324-341` | 根因修复 |
| 2 | 注入前备份 + 失败自动还原 | `tcp.rs:315-345` | 防损坏 |
| 3 | 插件 JS 顶层 try-catch | `tcp.rs:225-295` | 防 require 失败黑屏 |
| 4 | UI 添加"移除插件"按钮 | `app.rs` | 用户可撤销注入 |
| 5 | 连接前检查插件已注入 | `app.rs:609` | 防误操作 |

---

## Fix 1: 安全修改 plugins.js（JSON 解析方式）

**文件：** `crates/engines/rpgmaker/src/tcp.rs`，函数 `inject_plugin()`，lines 324-341

**当前（危险）：**
```rust
let content = fs::read_to_string(&plugins_js).map_err(|e| e.to_string())?;
if !content.contains(PLUGIN_FILENAME) {
    let mut new_content = String::new();
    for line in content.lines() {
        new_content.push_str(line);
        new_content.push('\n');
        if line.trim().starts_with("//") && line.contains("end of") {
            new_content.push_str(r#"{"name":"GameBridgeServer",...},"#);
            new_content.push('\n');
        }
    }
    if !new_content.contains(PLUGIN_FILENAME) {
        new_content = content.replace("];", r#"{...},\n];"#);
    }
    fs::write(&plugins_js, &new_content).map_err(|e| e.to_string())?;
}
```

**修复（安全）：**
```rust
let content = fs::read_to_string(&plugins_js).map_err(|e| e.to_string())?;
if !content.contains(PLUGIN_FILENAME) {
    // 找到 var $plugins = [...] 的数组边界
    let left = content.find('[')
        .ok_or_else(|| "plugins.js 格式不支持：找不到数组开始 '['".to_string())?;
    let right = content.rfind(']')
        .ok_or_else(|| "plugins.js 格式不支持：找不到数组结束 ']'".to_string())?;

    // 提取前缀和尾缀
    let prefix = &content[..=left];   // 包括 '['
    let suffix = &content[right..];   // 从 ']' 开始

    // 解析插件数组
    let array_json = &content[left+1..right];
    let mut plugins: Vec<serde_json::Value> =
        serde_json::from_str(&format!("[{}]", array_json))
        .map_err(|e| format!("plugins.js JSON 解析失败: {}", e))?;

    // 追加 GameBridgeServer 插件
    let entry = serde_json::json!({
        "name": "GameBridgeServer",
        "status": true,
        "description": "TCP Bridge",
        "parameters": {}
    });
    plugins.push(entry);

    // 重新序列化
    let entries: Vec<String> = plugins.iter()
        .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
        .collect();
    let new_content = format!("{}\n{}\n{}", prefix, entries.join(",\n"), suffix);
    fs::write(&plugins_js, &new_content).map_err(|e| e.to_string())?;
}
```

---

## Fix 2: 注入前备份 + 失败自动还原

**文件：** `crates/engines/rpgmaker/src/tcp.rs`，函数 `inject_plugin()`，line 315 之后

**变更 A — 修改前创建备份：**

```rust
// 在读取并准备修改 plugins.js 前，创建备份
let plugins_js_bak = plugins_js.with_extension("js.bak");
if plugins_js.is_file() && !plugins_js_bak.exists() {
    fs::copy(&plugins_js, &plugins_js_bak).map_err(|e| e.to_string())?;
}
```

**变更 B — 如果修改失败，自动还原：**

```rust
// 整个 plugins.js 修改逻辑用 match 包裹
let modify_result = (|| -> Result<(), String> {
    // ... JSON 解析 + 修改逻辑 ...
    Ok(())
})();

if let Err(e) = modify_result {
    // 还原备份
    if plugins_js_bak.exists() {
        fs::copy(&plugins_js_bak, &plugins_js).map_err(|e2| e2.to_string())?;
    }
    return Err(format!("修改 plugins.js 失败: {}，已自动还原", e));
}
```

**变更 C — remove_plugin 也还原备份：**

在 `remove_plugin()` 函数（line 348）中，如果备份存在，还原 plugins.js：

```rust
pub fn remove_plugin(game_dir: &str) -> Result<(), String> {
    let plugin_path = find_plugin_file(game_dir);
    if plugin_path.is_file() {
        fs::remove_file(&plugin_path).map_err(|e| e.to_string())?;
    }
    // 还原 plugins.js 备份
    let plugins_js = Path::new(game_dir).join("www/js/plugins.js");
    let plugins_js_bak = plugins_js.with_extension("js.bak");
    if plugins_js_bak.exists() {
        fs::copy(&plugins_js_bak, &plugins_js).map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

---

## Fix 3: 插件 JS 顶层 try-catch

**文件：** `crates/engines/rpgmaker/src/tcp.rs`，`PLUGIN_SOURCE` 常量，line 224-295

**当前：** `require('net')` 和 `server.listen()` 在顶层无保护。若 `require('net')` 失败，未捕获异常导致后续插件不执行 → 黑屏。

**修复：** 包裹整个插件顶层调用：

```rust
pub const PLUGIN_SOURCE: &str = r#"
(function() {
try {
var net = require('net');
var server = net.createServer(function(socket) {
    socket.setEncoding('utf8');
    // ... (rest of plugin unchanged) ...
});
server.listen(__PORT__, '127.0.0.1');
} catch(e) {
    // silently fail — don't crash the game
}
})();
"#;
```

将插件从全局作用域改为 IIFE（立即执行函数），顶层错误不会中断其他插件加载。

---

## Fix 4: UI 添加"移除插件"按钮

**文件：** `crates/gui/src/app.rs`，实时修改标签 TabMode::RealtimeEditor 区域

**变更：** 在"注入插件"按钮旁边，当插件已注入时显示"移除插件"按钮：

```rust
// 在 app.rs 插件注入按钮区域（约 line 614-623）
if !self.rt_panel.plugin_installed {
    if ui.button("注入插件").clicked() { self.inject_plugin(); }
} else {
    ui.colored_label(colors::SUCCESS, "✓ 插件已注入");
    if ui.button("移除插件").clicked() {
        if let Some(ref dir) = self.game_dir {
            match game_tool_rpgmaker::tcp::remove_plugin(dir) {
                Ok(()) => {
                    self.rt_panel.plugin_installed = false;
                    self.status_message = "插件已移除".into();
                }
                Err(e) => {
                    self.status_message = format!("移除失败: {}", e);
                }
            }
        }
    }
}
```

---

## Fix 5: 连接前检查插件已注入

**文件：** `crates/gui/src/app.rs`，连接按钮区域（约 line 609）

**变更：** 未注入插件时，连接按钮置灰并在 hover 时提示：

```rust
let can_connect = self.rt_panel.plugin_installed;
let connect_resp = ui.add_enabled_ui(can_connect, |ui| {
    ui.button("● 连接")
}).inner;

if !can_connect {
    connect_resp.on_hover_text("请先点击「注入插件」，然后启动游戏");
}
if connect_resp.clicked() && can_connect {
    self.rt_connect();
}
```

---

## 验证步骤

1. `cargo build -p game-tool-gui 2>&1` — 零错误
2. `cargo test -p game-tool-rpgmaker 2>&1` — 通过
3. `cargo test 2>&1` — 全工作区通过
4. 手动测试：选游戏目录 → 注入插件 → 检查 plugins.js 生成正确 → 启动游戏正常
