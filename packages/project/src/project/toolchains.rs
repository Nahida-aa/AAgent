use std::collections::{BTreeMap, IndexSet};
use std::path::PathBuf;

use anyhow::Result;
use gpui::{App, Context, Entity, Task};
use language::{LanguageName, LanguageRegistry, Toolchain, ToolchainMetadata, ToolchainScope};

use super::Project;
use crate::toolchain_store::{ToolchainStore, Toolchains};
use crate::ProjectPath;

impl Project {
    pub fn available_toolchains(&self, path: ProjectPath, language_name: LanguageName, cx: &App) -> Task<Option<Toolchains>> { /* 原样 */ }
    pub async fn toolchain_metadata(languages: Arc<LanguageRegistry>, language_name: LanguageName) -> Option<ToolchainMetadata> { /* 原样 */ }
    pub fn add_toolchain(&self, toolchain: Toolchain, scope: ToolchainScope, cx: &mut Context<Self>) { /* 原样 */ }
    pub fn remove_toolchain(&self, toolchain: Toolchain, scope: ToolchainScope, cx: &mut Context<Self>) { /* 原样 */ }
    pub fn user_toolchains(&self, cx: &App) -> Option<BTreeMap<ToolchainScope, IndexSet<Toolchain>>> { /* 原样 */ }
    pub fn resolve_toolchain(&self, path: PathBuf, language_name: LanguageName, cx: &App) -> Task<Result<Toolchain>> { /* 原样 */ }
    pub fn toolchain_store(&self) -> Option<Entity<ToolchainStore>> { /* 原样 */ }
    pub fn activate_toolchain(&self, path: ProjectPath, toolchain: Toolchain, cx: &mut App) -> Task<Option<()>> { /* 原样 */ }
    pub fn active_toolchain(&self, path: ProjectPath, language_name: LanguageName, cx: &App) -> Task<Option<Toolchain>> { /* 原样 */ }
}
