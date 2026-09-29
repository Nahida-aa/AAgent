//! 随应用内嵌的主题 JSON 的装载（对齐 zed `theme_settings.rs:218-360`）。
//!
//! zed 的流程：`theme_settings::init` → `load_bundled_themes(&registry)`：
//! 列出资产里 `themes/` 下的 JSON → 解析成 [`ThemeFamilyContent`] →
//! [`refine_theme_family`]（以通用 dark/light 基色 refine 上 JSON 的内容）→
//! 插进注册表。
//!
//! **内置主题的语法高亮就是这么来的**：Catppuccin 的 102 个 syntax capture
//! 全在 JSON 里，Rust 侧不需要手写任何色值——手写的
//! `theme::default_colors::catppuccin_*` 只承担「一个主题都装载不到」时的兜底
//! （对应 zed 的 `fallback_themes::zed_default_themes` 角色）。
//!
//! 与 zed 的差异只有一处：zed 的 `Theme::id` 用 uuid，我们用主题名
//! （注册表按 name 查找，id 只用于展示；与 gpui_learn 自己的加载器一致）。

use std::sync::Arc;

use refineable::Refineable as _;
use settings_content::theme::theme_style::{AccentContent, PlayerColorContent};
// `theme_settings` 内部有同名 `pub mod settings` 遮蔽外部 settings crate，
// 故这里用绝对路径（本文件注释里那套约定的同款写法）。
use ::settings::IntoGpui as _;
use theme::{
    AccentColors, Appearance, AppearanceContent, PlayerColor, PlayerColors, StatusColors,
    SyntaxTheme, SystemColors, Theme, ThemeColors, ThemeFamily, ThemeRegistry, ThemeStyles,
    try_parse_color,
};

use crate::schema::{
    ThemeContent, ThemeFamilyContent, status_colors_refinement, syntax_overrides,
    theme_colors_refinement,
};

/// 把资产里 `themes/` 下的所有主题 JSON 装进注册表。
///
/// 单个文件解析失败只告警不中断（对齐 zed 的 `log_err` 语义）。
pub fn load_bundled_themes(registry: &ThemeRegistry) {
    let Ok(theme_paths) = registry.assets().list("themes/") else {
        tracing::warn!("failed to list bundled theme assets");
        return;
    };

    for path in theme_paths.into_iter().filter(|p| p.ends_with(".json")) {
        let Ok(Some(bytes)) = registry.assets().load(&path) else {
            continue;
        };
        match serde_json::from_slice::<ThemeFamilyContent>(&bytes) {
            Ok(theme_family) => {
                let family = refine_theme_family(theme_family);
                tracing::info!(
                    "bundled theme family: {} ({} themes)",
                    family.name,
                    family.themes.len()
                );
                registry.insert_theme_families([family]);
            }
            Err(err) => tracing::warn!("failed to parse theme at \"{path}\": {err:#}"),
        }
    }
}

/// 把 [`ThemeFamilyContent`] 及其 [`ThemeContent`] refine 成 [`ThemeFamily`]。
pub fn refine_theme_family(theme_family_content: ThemeFamilyContent) -> ThemeFamily {
    let name = theme_family_content.name.clone();
    let author = theme_family_content.author.clone();
    let themes = theme_family_content.themes.iter().map(refine_theme).collect();

    ThemeFamily {
        id: name.clone(),
        name,
        author,
        themes,
    }
}

/// 把 [`ThemeContent`] refine 成 [`Theme`]。
///
/// 基色按主题明暗取通用 dark/light（与 zed 同款），再用 JSON 给的内容
/// refine 覆盖；JSON 没给的字段就留在基色上。
pub fn refine_theme(theme: &ThemeContent) -> Theme {
    let appearance = match theme.appearance {
        AppearanceContent::Light => Appearance::Light,
        AppearanceContent::Dark => Appearance::Dark,
    };
    let is_light = theme.appearance == AppearanceContent::Light;

    let mut status = if is_light {
        StatusColors::light()
    } else {
        StatusColors::dark()
    };
    let mut status_refinement = status_colors_refinement(&theme.style.status);
    theme::fallback_themes::apply_status_color_defaults(&mut status_refinement);
    status.refine(&status_refinement);

    let mut players = if is_light {
        PlayerColors::light()
    } else {
        PlayerColors::dark()
    };
    merge_player_colors(&mut players, &theme.style.players);

    let mut colors = if is_light {
        ThemeColors::light()
    } else {
        ThemeColors::dark()
    };
    let mut colors_refinement =
        theme_colors_refinement(&theme.style.colors, &status_refinement, is_light);
    theme::fallback_themes::apply_theme_color_defaults(&mut colors_refinement, &players);
    colors.refine(&colors_refinement);

    let mut accents = if is_light {
        AccentColors::light()
    } else {
        AccentColors::dark()
    };
    merge_accent_colors(&mut accents, &theme.style.accents);

    // 语法高亮：JSON 里有多少个 capture 就铺多少个（Catppuccin 是 102 个）。
    let syntax = Arc::new(SyntaxTheme::new(syntax_overrides(&theme.style)));

    Theme {
        id: theme.name.clone(),
        name: theme.name.clone().into(),
        appearance,
        styles: ThemeStyles {
            system: SystemColors::default(),
            window_background_appearance: theme
                .style
                .window_background_appearance
                .map(|w| w.into_gpui())
                .unwrap_or_default(),
            accents,
            colors,
            status,
            players,
            syntax,
        },
    }
}

/// 用 JSON 的协作者配色覆盖基色（对齐 zed `merge_player_colors`）。
pub fn merge_player_colors(
    player_colors: &mut PlayerColors,
    user_player_colors: &[PlayerColorContent],
) {
    if user_player_colors.is_empty() {
        return;
    }

    for (idx, player) in user_player_colors.iter().enumerate() {
        let cursor = player.cursor.as_ref().and_then(|c| try_parse_color(c).ok());
        let background = player
            .background
            .as_ref()
            .and_then(|c| try_parse_color(c).ok());
        let selection = player
            .selection
            .as_ref()
            .and_then(|c| try_parse_color(c).ok());

        if let Some(player_color) = player_colors.0.get_mut(idx) {
            *player_color = PlayerColor {
                cursor: cursor.unwrap_or(player_color.cursor),
                background: background.unwrap_or(player_color.background),
                selection: selection.unwrap_or(player_color.selection),
            };
        } else {
            player_colors.0.push(PlayerColor {
                cursor: cursor.unwrap_or_default(),
                background: background.unwrap_or_default(),
                selection: selection.unwrap_or_default(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_with_bundled() -> ThemeRegistry {
        let registry = ThemeRegistry::new(Box::new(aa_gpui_kit_assets::Assets));
        load_bundled_themes(&registry);
        registry
    }

    #[test]
    fn bundled_catppuccin_themes_carry_the_full_syntax_table() {
        let registry = registry_with_bundled();
        for name in [
            "Catppuccin Latte",
            "Catppuccin Frappé",
            "Catppuccin Macchiato",
            "Catppuccin Mocha",
        ] {
            let theme = registry
                .get(name)
                .unwrap_or_else(|_| panic!("内置主题 {name} 应被注册"));
            let syntax = theme.syntax();
            assert_eq!(
                syntax.names().count(),
                102,
                "{name} 的语法 capture 数应与 JSON 一致"
            );
            for capture in ["keyword", "function.method", "variable.member", "type.builtin"] {
                assert!(
                    syntax.style_for_name(capture).is_some(),
                    "{name} 应能取到 {capture} 的样式"
                );
            }
        }
    }

    /// 容差比较：JSON 走 `try_parse_color`（palette 的 sRGB→HSL），与 gpui
    /// 自带的转换在浮点末位差 1e-7 量级。
    fn assert_color_close(actual: gpui::Hsla, expected: &str) {
        let expected = gpui::Hsla::from(gpui::Rgba::try_from(expected).unwrap());
        let close = |a: f32, b: f32| (a - b).abs() < 1e-6;
        assert!(
            close(actual.h, expected.h)
                && close(actual.s, expected.s)
                && close(actual.l, expected.l)
                && close(actual.a, expected.a),
            "应为 {expected:?}：\n  actual = {actual:?}"
        );
    }

    /// 锁住「之前手写构造漂移过」的那几个字段（文本 / 断点 / git），
    /// 防止以后又回到手写值上。
    #[test]
    fn bundled_themes_carry_catppuccin_semantic_colors() {
        let registry = registry_with_bundled();

        let macchiato = registry.get("Catppuccin Macchiato").expect("macchiato");
        let colors = macchiato.colors();
        assert_color_close(colors.text, "#cad3f5");
        assert_color_close(colors.debugger_accent, "#ed8796");
        assert_color_close(colors.version_control_added, "#a6da95");
        assert_color_close(colors.version_control_modified, "#eed49f");

        let latte = registry.get("Catppuccin Latte").expect("latte");
        assert_eq!(latte.appearance, Appearance::Light);
        assert_color_close(latte.colors().debugger_accent, "#d20f39");
        assert_color_close(latte.colors().version_control_added, "#40a02b");
    }
}

/// 用 JSON 的强调色整体替换基色（对齐 zed `merge_accent_colors`）。
pub fn merge_accent_colors(accent_colors: &mut AccentColors, user_accent_colors: &[AccentContent]) {
    if user_accent_colors.is_empty() {
        return;
    }

    let colors = user_accent_colors
        .iter()
        .filter_map(|accent| accent.0.as_ref().and_then(|c| try_parse_color(c).ok()))
        .collect::<Vec<_>>();

    if !colors.is_empty() {
        accent_colors.0 = Arc::from(colors);
    }
}
