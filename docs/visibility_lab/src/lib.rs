//! visibility_lab — Rust 2024 edition 模块可见性验证
//!
//! 跑 `cargo check` 验证所有 ✅ 注释行能编译。
//! 把 ✅ 改成 ❌ 的行取消注释会看到对应的 E0603/E0432 错误。

pub mod alpha;              // pub 到 crate 内外都可见
mod beta;                   // Rust 2024: 私有 = pub(crate), 整个 crate 可见
mod gamma;                  // 同上

// ============================================================================
// 1. Rust 2024 lib.rs 里私有 mod = pub(crate) mod
// ============================================================================
//
// 上面写的 `mod beta;` 和 `mod gamma;` 是私有 mod,
// 但因为在 lib.rs (crate 根) + Rust 2024, 它们等价于 `pub(crate) mod beta;`
// 整个 crate 内部任何地方都能 `crate::beta::xxx` 访问。
//
// 把下面这行取消注释 → rustc 会警告多余的 pub(crate):
// pub(crate) mod beta_dup;   // ❌ warning: unnecessary visibility modifier

// ============================================================================
// 2. lib.rs (crate 根) 里访问兄弟
// ============================================================================
//
// 在 lib.rs 里没有父节点, 所以不能写 super::xxx (会 E0433)。
// 只能用隐式路径或 crate::xxx。

#[allow(dead_code)]
fn lib_uses_anybody() {
    let _ = alpha::alpha_fn();             // ✅ 隐式路径 (crate 根能看到所有 pub 和 Rust 2024 私有 mod)
    let _ = beta::beta_fn();               // ✅ 同上, 私有 mod 但 Rust 2024 根可见
    let _ = gamma::gamma_fn();             // ✅ 同上
    let _ = crate::alpha::alpha_fn();      // ✅ crate:: 路径
    let _ = crate::beta::beta_fn();        // ✅ crate:: 路径
    // let _ = super::beta::beta_fn();     // ❌ E0433: crate 根没有 super
}

// ============================================================================
// 3. 子模块内部用 super:: 访问兄弟
// ============================================================================
//
// 看 alpha/mod.rs 和 beta.rs 里的例子:
//   alpha 里访问 beta → 隐式 `beta::beta_fn()` 或 `super::beta::beta_fn()` 或 `crate::beta::beta_fn()`
//   三种都通, 语义上 super:: 最清晰 (从父节点出发找兄弟)

// ============================================================================
// 4. 子模块内部的私有 mod — 父可见, 兄弟不可见
// ============================================================================
//
// alpha 有子模块 alpha/private.rs (mod private; 私有):
//   - alpha 内部 ✅ 直接可见 (父看子私有)
//   - beta 内部 ❌ 不可见 (兄弟看兄弟私有子模块)
//     → beta 要访问 alpha_private 里的东西, alpha 必须先 pub use 出去

// ============================================================================
// 5. 外部 crate 访问 — 必须 pub
// ============================================================================
//
// 外部 crate 写 `use visibility_lab::...`:
//   - `pub mod alpha`      → ✅ 外部能访问 alpha::*
//   - `mod beta` (私有)    → ❌ 外部不能访问 (Rust 2024 pub(crate) 也只对内部)
//
//   如果想让外部能访问 beta 的内容但不暴露 beta 模块本身:
//   pub use beta::beta_fn;  // lib.rs 里加这行, 外部就能 visibility_lab::beta_fn()

// ============================================================================
// 6. 核心教训 (Rust 2024 edition)
// ============================================================================
//
//   在 lib.rs (crate 根):
//     - 私有 mod = pub(crate) mod, 整个 crate 可见
//     - pub(crate) 前缀多余, 可能触发警告
//     - 不能用 super:: (没有父节点), 只能用隐式路径或 crate::
//     - 不需要 crate 根共享导出块 — 调用点自己写 use crate::xxx::yyy
//
//   在子模块 mod.rs:
//     - 私有 mod xxx 只给父模块 + 子模块可见
//     - 兄弟要互访 → 把 xxx 标 pub(crate), 或调用点写 use crate::xxx
//     - super::xxx 是子模块里最清晰的兄弟访问方式
//
//   跨 crate 访问:
//     - 必须 pub (pub(crate) 对外部不可见)
