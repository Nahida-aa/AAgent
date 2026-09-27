// gamma — 私有 mod, 但 Rust 2024 = pub(crate), 整个 crate 可见

pub fn gamma_fn() -> &'static str { "gamma" }

pub mod inner;  // pub mod inner → crate 内外都能访问
