//! GUI 面板模块集合。
//!
//! 每个面板负责渲染一个特定的功能区域，通过返回 Action 枚举向应用层传递用户操作。
//! 面板之间通过 `AppState` 共享状态，互不直接通信。

pub mod backup;
pub mod realtime_editor;
pub mod save_editor;
pub mod settings;
pub mod startup;
pub mod status_bar;
pub mod tab_bar;
pub mod toolbox;
pub mod top_bar;
