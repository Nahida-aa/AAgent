//! settings-macros — 过程宏 crate，定义 settings 相关的 derive macros。
//!
//! 对齐 Zed `crates/settings_macros/src/settings_macros.rs`。
//!
//! 目前导出：
//! - `RegisterSetting` — 把一个类型注册到 `settings::SettingsStore`。
//!
//! 运行时机制：
//! - 宏展开时生成 `settings::private::inventory::submit! { ... }`，
//!   inventory crate 在编译期收集所有注册条目
//! - `settings::SettingsStore::init()` 时通过 `inventory::collect!` 拉取

use proc_macro::TokenStream;

use quote::quote;
use syn::DeriveInput;
use syn::parse_macro_input;

/// 注册一个设置类型。
///
/// # 用法
/// ```ignore
/// use settings_macros::RegisterSetting;
///
/// #[derive(Clone, Debug, Default, RegisterSetting)]
/// pub struct MySettings {
///     pub foo: String,
///     pub bar: Option<u32>,
/// }
///
/// impl settings::Settings for MySettings {
///     fn from_settings(content: &settings_content::SettingsContent) -> Self { ... }
/// }
/// ```
///
/// 编译期通过 `inventory` crate 提交一个 `settings::private::RegisteredSetting`，
/// `SettingsStore` 初始化时遍历 inventory 完成所有 setting 类型的注册。
#[proc_macro_derive(RegisterSetting)]
pub fn derive_register_setting(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let type_name = &input.ident;

    let expanded = quote! {
        settings::private::inventory::submit! {
            settings::private::RegisteredSetting {
                settings_value: || {
                    Box::new(settings::private::SettingValue::<#type_name> {
                        global_value: None,
                    }),
                },
                from_settings: |content| {
                    Box::new(<#type_name as settings::Settings>::from_settings(content))
                },
                id: || std::any::TypeId::of::<#type_name>(),
            }
        }
    };
    TokenStream::from(expanded)
}
