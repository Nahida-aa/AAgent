//! 全局 init 链 — 对齐 Zed `crates/zed/src/main.rs`。
//!
//! 注意：当前所有初始化步骤直接内联在 `main.rs` 的 `app.run` 闭包内。
//! 此模块暂为占位，后续可把与 AppState 无关的纯 crate init 链提取到此。

pub fn init(_cx: &mut gpui::App) {
    // 预留给独立 init 链，当前 main.rs 直接内联。
}
