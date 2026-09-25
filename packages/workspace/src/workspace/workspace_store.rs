use client::Client;
use collections::HashSet;
use gpui::{
    Action, AnyEntity, AnyView, AnyWeakView, App, AppContext, AsyncApp, AsyncWindowContext, Axis,
    Bounds, ClipboardItem, Context, CursorStyle, Decorations, DragMoveEvent, Entity, EntityId,
    EventEmitter, FocusHandle, Focusable, Global, HitboxBehavior, Hsla, KeyContext, Keystroke,
    ManagedView, MouseButton, PathPromptOptions, Point, PromptLevel, Render, ResizeEdge, Size,
    Stateful, Subscription, SystemWindowTabController, Task, TaskExt, Tiling, WeakEntity,
    WindowBounds, WindowHandle, WindowId, WindowOptions, actions, canvas, point, relative, size,
    transparent_black,
};
use proto::PeerId;



#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Follower {
    pub project_id: Option<u64>,
    pub peer_id: PeerId,
}

pub struct WorkspaceStore {
    pub(crate) workspaces: HashSet<(gpui::AnyWindowHandle, WeakEntity<Workspace>)>,
    pub(crate) client: Arc<Client>,
    pub(crate) _subscriptions: Vec<client::Subscription>,
}

impl WorkspaceStore {
    pub fn new(client: Arc<Client>, cx: &mut Context<Self>) -> Self {
        Self {
            workspaces: Default::default(),
            _subscriptions: vec![
                client.add_request_handler(cx.weak_entity(), Self::handle_follow),
                client.add_message_handler(cx.weak_entity(), Self::handle_update_followers),
            ],
            client,
        }
    }

    pub fn update_followers(
        &self,
        project_id: Option<u64>,
        update: proto::update_followers::Variant,
        cx: &App,
    ) -> Option<()> {
        let active_call = GlobalAnyActiveCall::try_global(cx)?;
        let room_id = active_call.0.room_id(cx)?;
        self.client
            .send(proto::UpdateFollowers {
                room_id,
                project_id,
                variant: Some(update),
            })
            .log_err()
    }

    pub async fn handle_follow(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::Follow>,
        mut cx: AsyncApp,
    ) -> Result<proto::FollowResponse> {
        this.update(&mut cx, |this, cx| {
            let follower = Follower {
                project_id: envelope.payload.project_id,
                peer_id: envelope.original_sender_id()?,
            };

            let mut response = proto::FollowResponse::default();

            this.workspaces.retain(|(window_handle, weak_workspace)| {
                let Some(workspace) = weak_workspace.upgrade() else {
                    return false;
                };
                window_handle
                    .update(cx, |_, window, cx| {
                        workspace.update(cx, |workspace, cx| {
                            let handler_response =
                                workspace.handle_follow(follower.project_id, window, cx);
                            if let Some(active_view) = handler_response.active_view
                                && workspace.project.read(cx).remote_id() == follower.project_id
                            {
                                response.active_view = Some(active_view)
                            }
                        });
                    })
                    .is_ok()
            });

            Ok(response)
        })
    }

    async fn handle_update_followers(
        this: Entity<Self>,
        envelope: TypedEnvelope<proto::UpdateFollowers>,
        mut cx: AsyncApp,
    ) -> Result<()> {
        let leader_id = envelope.original_sender_id()?;
        let update = envelope.payload;

        this.update(&mut cx, |this, cx| {
            this.workspaces.retain(|(window_handle, weak_workspace)| {
                let Some(workspace) = weak_workspace.upgrade() else {
                    return false;
                };
                window_handle
                    .update(cx, |_, window, cx| {
                        workspace.update(cx, |workspace, cx| {
                            let project_id = workspace.project.read(cx).remote_id();
                            if update.project_id != project_id && update.project_id.is_some() {
                                return;
                            }
                            workspace.handle_update_followers(
                                leader_id,
                                update.clone(),
                                window,
                                cx,
                            );
                        });
                    })
                    .is_ok()
            });
            Ok(())
        })
    }

    pub fn workspaces(&self) -> impl Iterator<Item = &WeakEntity<Workspace>> {
        self.workspaces.iter().map(|(_, weak)| weak)
    }

    pub fn workspaces_with_windows(
        &self,
    ) -> impl Iterator<Item = (gpui::AnyWindowHandle, &WeakEntity<Workspace>)> {
        self.workspaces.iter().map(|(window, weak)| (*window, weak))
    }
}
