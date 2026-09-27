// beta.rs — 私有 mod (在 lib.rs 声明), 但 Rust 2024 = pub(crate)
// 整个 crate 内部都能访问

pub fn beta_fn() -> &'static str { "beta" }

// 试试取消这行注释 — alpha 里直接 `use beta::beta_fn()` 能不能行?
// (应该能, 因为 Rust 2024 私有 = pub(crate))
