//! Tab 渲染相关的类型。
//!
//! 对齐 Zed `TabContentParams` + `TabTooltipContent` + `ItemBufferKind`。

use gpui::SharedString;

/// Tab 渲染的上下文参数 — 对齐 Zed `TabContentParams`。
///
/// Pane 渲染 tab bar 时，每个 tab 的内容（label、icon、tooltip）
/// 都要根据当前状态（选中/预览/未聚焦）做调整。TabContentParams
/// 把这些状态打包一起传，避免 tab_label/tab_icon 各自拿一堆参数。
#[derive(Debug, Clone, Copy, Default)]
pub struct TabContentParams {
    /// Zed 里有的 detail 字段 — 比如 terminal 显示 shell 名、
    /// editor 显示 git status。AAgent 暂时不用，保留结构。
    pub detail: Option<usize>,
    /// 当前 tab 是否被选中。
    pub selected: bool,
    /// 是否 preview tab（点一下临时打开的那种）。
    pub preview: bool,
    /// 不聚焦的 pane 里 tab 应该弱化显示。
    pub deemphasized: bool,
    /// 标题最大长度（None = item 自己决定）。
    pub max_title_len: Option<usize>,
    /// 是否中间截断长标题。
    pub truncate_title_middle: bool,
}

impl TabContentParams {
    /// 根据 selected + deemphasized 算 tab 文字颜色。
    /// 对齐 Zed TabContentParams::text_color()。
    /// AAgent 没 Color enum（用 hsla），先返回 bool 给 caller 做判断。
    pub fn use_dimmed_text(&self) -> bool {
        if self.deemphasized {
            true
        } else if self.selected {
            false
        } else {
            // 未选中非弱化 — 也要 dim 一下
            true
        }
    }
}

/// Tab tooltip 内容 — 对齐 Zed `TabTooltipContent`。
///
/// Zed 有两种：纯文本 / 自定义 view。AAgent 先只做文本版。
pub enum TabTooltipContent {
    Text(SharedString),
    // 预留 Custom — 以后 Editor 可以显示完整路径 + git status 等
}

/// Item buffer 类型 — 对齐 Zed `ItemBufferKind`。
/// Zed 用来判断 tab 是否显示 split marker、buffer 数量等。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemBufferKind {
    /// 多个 buffer（比如 Search Results multibuffer）。
    Multibuffer,
    /// 单实例（比如 Editor 一个文件一个实例）。
    Singleton,
    /// 不适用（Terminal 没有 buffer 概念）。
    None,
}

impl Default for ItemBufferKind {
    fn default() -> Self { Self::None }
}
