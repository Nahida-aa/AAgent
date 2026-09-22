use anyhow::{Context as _, Result};
use client::{AnyProtoClient, Client, TypedEnvelope, UserStore};
use gpui::{AsyncApp, Entity};

use super::state::ProjectClientState;
use super::Project;
use crate::constants::CURRENT_PROJECT_FEATURES;
use crate::types::*;
use crate::{Event, ProjectPath};

impl Project {
    pub async fn in_room(
        remote_id: u64,
        client: Arc<Client>,
        user_store: Entity<UserStore>,
        languages: Arc<language::LanguageRegistry>,
        fs: Arc<dyn Fs>,
        cx: AsyncApp,
    ) -> Result<Entity<Self>> {
        // 原样搬入
    }

    async fn from_join_project_response(
        response: TypedEnvelope<proto::JoinProjectResponse>,
        subscriptions: [super::EntitySubscription; 8],
        client: Arc<Client>,
        run_tasks: bool,
        user_store: Entity<UserStore>,
        languages: Arc<language::LanguageRegistry>,
        fs: Arc<dyn Fs>,
        mut cx: AsyncApp,
    ) -> Result<Entity<Self>> {
        // 原样搬入
    }

    pub(crate) fn set_collaborators_from_proto(
           &mut self,
           messages: Vec<proto::Collaborator>,
           cx: &mut Context<Self>,
       ) -> anyhow::Result<()> {
           // 原样搬入
       }
}
