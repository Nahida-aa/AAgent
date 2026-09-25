//! 对 Workspace 这个实体的操作。
//! 这里的 `dock` 是“Workspace 对 crate::dock::Dock 的操作”，
//! 不是 Dock 组件本身。Dock 组件在 crate::dock。

// pane 与 item 的关系
// Pane 是容器，拥有 Vec<Box<dyn ItemHandle>>、active_item_index、preview_item_id、pinned_count、nav_history。

// Item 是被容纳的东西，本身不知道自己在哪个 pane。

// Workspace 是全局索引，用 panes: Vec<Entity<Pane>> 和 panes_by_item: HashMap<EntityId, WeakEntity<Pane>> 维护“某个 item 在哪个 pane”。

// 关系是 Pane 1 : N Item，一个 item 同一时刻只在一个 pane 里（clone/split 会生成新 item 实体）。

// 所以“查找 item 属于哪个 pane”这个查询，本质是 Workspace 层面的索引查询，不是 Pane 自己的方法

// ├── ops/                       # 实体操作层：对 Workspace 的操作
// │   ├── mod.rs
// │   ├── dock.rs                # 对 crate::dock::Dock 的操作
// │   ├── pane.rs                # 对 crate::pane::Pane 的操作 pane 结构本身的操作
// │   ├── item.rs                # 对 crate::item::Item 的操作 打开/关闭/激活
// │   ├── window.rs              # 窗口标题、bounds、chrome
// │   ├── serialization.rs       # Workspace 序列化
// │   ├── collaboration.rs       # 协作、follow、channel
// │   ├── following.rs           # follower/leader 状态
// │   ├── worktree.rs            # worktree 切换、信任
// │   └── render.rs              # impl Render for Workspace
