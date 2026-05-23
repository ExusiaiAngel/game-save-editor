//! GameSaveEditor 图形用户界面（GUI）crate。
//!
//! 本 crate 基于 [`eframe`] / [`egui`] 构建，提供跨平台的桌面 GUI 界面。
//!
//! # 模块结构
//!
//! - [`app`] — 主状态机与布局管理器，核心的 `AppState` 与 `eframe::App` 实现
//! - [`state`] — 所有 UI 枚举与状态结构体定义
//! - [`theme`] — 主题样式（明/暗色）与颜色常量
//! - [`factory`] — 引擎类型到格式处理器/网桥/面板模式的工厂映射
//! - [`connection`] — 实时连接桥接线程的创建与管理
//! - [`discovery`] — 游戏目录中存档文件的自动发现与扫描
//! - [`panels`] — 各功能面板（存档编辑、实时修改、备份、工具箱、设置、状态栏）
//! - [`widgets`] — 可复用 UI 组件（字段表格、分类树、搜索栏、摘要卡片）
//!
//! # 架构概览
//!
//! GUI 采用单状态树架构：`AppState` 聚合所有面板状态，`eframe::App::update()` 每帧
//! 驱动渲染循环。异步操作（如实时连接）通过独立线程 + mpsc 通道与主线程通信。

pub mod app;
pub mod connection;
pub mod discovery;
pub mod factory;
pub mod panels;
pub mod state;
pub mod theme;
pub mod widgets;
