pub fn alpha_fn() -> &'static str { "alpha" }

mod private;  // 私有子模块, 只有 alpha 内部能直接访问

#[allow(dead_code)]
fn uses_private() {
    let _ = private::private_fn();  // ✅ 父看子私有, 直接可见
}
