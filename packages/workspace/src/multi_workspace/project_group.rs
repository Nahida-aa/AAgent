pub use project::ProjectGroupKey;
use settings::SidebarSide;

#[derive(Clone)]
pub struct ProjectGroup {
    pub key: ProjectGroupKey,
    pub workspaces: Vec<Entity<Workspace>>,
    pub expanded: bool,
}

pub struct SerializedProjectGroupState {
    pub key: ProjectGroupKey,
    pub expanded: bool,
}

#[derive(Clone)]
pub struct ProjectGroupState {
    pub key: ProjectGroupKey,
    pub expanded: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemovalIntent {
    KeepProject,
    CloseProject,
}
