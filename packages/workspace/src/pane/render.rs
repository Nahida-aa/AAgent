use super::actions::{
    ActivateItem, ActivateLastItem, ActivateNextItem, ActivatePreviousItem, AlternateFile,
    CloseActiveItem, CloseAllItems, CloseCleanItems, CloseItemsToTheLeft, CloseItemsToTheRight,
    CloseMultibufferItems, CloseOtherItems, DeploySearch, GoBack, GoForward, GoToNewerTag,
    GoToOlderTag, JoinAll, JoinIntoNext, RevealInProjectPanel, SplitAndMoveDown, SplitAndMoveLeft,
    SplitAndMoveRight, SplitAndMoveUp, SplitDown, SplitHorizontal, SplitLeft, SplitRight, SplitUp,
    SplitVertical, SwapItemLeft, SwapItemRight, TogglePinTab, TogglePreviewTab, UnpinAllTabs,
};

impl Focusable for Pane {
    /* 放 mod.rs 或这里都行，建议 mod.rs */
}

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        /* 原函数体 */
    }
}

impl Render for DraggedTab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        /* 原函数体 */
    }
}
