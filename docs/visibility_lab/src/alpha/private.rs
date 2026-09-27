// alpha/private.rs — alpha 的私有子模块

pub(super) fn private_fn() -> &'static str { "alpha_private" }

// pub(super) = 只有 alpha 父模块能访问
// 改成 pub(crate) → 整个 crate 内部都能访问
// 改成 pub → 外部 crate 也能访问
