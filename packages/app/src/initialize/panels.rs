//! Dock Panel 统一注册 — 对齐 Zed `crates/zed/src/zed.rs::initialize_panels`。
//!
//! Zed 的做法:每个 Panel crate 暴露 `load(workspace_handle, cx) -> Task/Result<Entity<P>>`
//! (都是 async fn，AgentPanel 例外返回 Task)，在 `cx.spawn_in(window, async { futures::join!(...) })`
//! 里并行加载后 `add_panel`，最后 `workspace.finish_dock_restoration(cx)`。

use gpui::{AsyncWindowContext, Context, Entity, Result as GpuiResult, Task, WeakEntity, Window};
use gpui_util::ResultExt;
use workspace::{Panel, Workspace};

use project_panel::ProjectPanel;
use outline_panel::OutlinePanel;
use git_ui::git_panel::GitPanel;
use terminal_view::TerminalPanel;

/// 在 Workspace observe_new 回调里被调用，负责把各 Panel entity 注入 Dock。
///
/// 对齐 Zed `initialize_panels`（crates/zed/src/zed.rs L777）：
/// 返回 `Task<anyhow::Result<()>>`，调用方通过 `workspace.set_panels_task(task)` 保存。
pub fn initialize_panels(window: &mut Window, cx: &mut Context<Workspace>) -> Task<anyhow::Result<()>> {
    cx.spawn_in(window, async move |workspace_handle, cx| {
        let project_panel = ProjectPanel::load(workspace_handle.clone(), cx.clone());
        let outline_panel = OutlinePanel::load(workspace_handle.clone(), cx.clone());
        let git_panel = GitPanel::load(workspace_handle.clone(), cx.clone());
        let terminal_panel = TerminalPanel::load(workspace_handle.clone(), cx.clone());
        // AgentPanel 通过单独的 agent_ui::init() observe_new 注入，这里不管。

        async fn add_panel_when_ready(
            panel_task: impl std::future::Future<Output = anyhow::Result<Entity<impl Panel>>> + 'static,
            workspace_handle: WeakEntity<Workspace>,
            mut cx: AsyncWindowContext,
        ) {
            if let Some(panel) = panel_task.await.log_err() {
                let _: GpuiResult<()> = workspace_handle
                    .update_in(&mut cx, |workspace, window, cx| {
                        workspace.add_panel(panel, window, cx);
                    });
            }
        }

        futures::join!(
            add_panel_when_ready(project_panel, workspace_handle.clone(), cx.clone()),
            add_panel_when_ready(outline_panel, workspace_handle.clone(), cx.clone()),
            add_panel_when_ready(git_panel, workspace_handle.clone(), cx.clone()),
            add_panel_when_ready(terminal_panel, workspace_handle.clone(), cx.clone()),
        );

        let _: GpuiResult<()> = workspace_handle.update(cx, |workspace, cx| {
            workspace.finish_dock_restoration(cx);
        });

        anyhow::Ok(())
    })
}
