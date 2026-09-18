# Sidebar 解耦设计：循环依赖与 dyn object

## 问题：循环依赖

Sidebar entity 未来独立到 `packages/sidebar/` crate（对齐 zed `crates/sidebar`）后：

```
sidebar crate  ──依赖──▶  workspace crate  （Sidebar 实现 Sidebar trait）
workspace crate ──依赖──▶  sidebar crate  （MultiWorkspace 存 Sidebar entity）
===========================================
                 循环 ❌ Rust Cargo 不允许
```

必须在 `workspace` crate 里定义好 Sidebar 的接口契约（trait），让 `sidebar` crate 只实现契约、不反向依赖 `workspace` crate 的具体类型。

---

## Zed 解法：两层 trait + dyn object

Zed 选择了 **两层 trait** 解耦（`crates/workspace/src/multi_workspace.rs`）：

### 1. 强类型 trait `Sidebar`（L121）

```rust
// workspace crate 定义，sidebar crate 实现（单向依赖 OK）
pub trait Sidebar: Focusable + Render + Sized {
    fn width(&self, cx: &App) -> Pixels;
    fn set_width(&mut self, width: Option<Pixels>, cx: &mut Context<Self>);
    fn has_notifications(&self, cx: &App) -> bool;
    fn side(&self, cx: &App) -> SidebarSide;
    // ...
}
```

### 2. dyn object trait `SidebarHandle`（L162）

```rust
// MultiWorkspace 存 Box<dyn SidebarHandle>
// 因为 MultiWorkspace 不能直接存 Entity<Sidebar>（循环）
pub trait SidebarHandle: Send + Sync {
    fn width(&self, cx: &App) -> Pixels;
    fn focus_handle(&self, cx: &App) -> FocusHandle;
    fn to_any(&self) -> AnyView;
    fn entity_id(&self) -> EntityId;
    // ...
}
```

### 3. 桥接 impl（L191）

```rust
// 任何实现了 Sidebar 的 Entity<T> 自动是 SidebarHandle
impl<T: Sidebar> SidebarHandle for Entity<T> {
    fn width(&self, cx: &App) -> Pixels { self.read(cx).width(cx) }
    // ...
}
```

### MultiWorkspace 字段（L316）

```rust
pub struct MultiWorkspace {
    sidebar: Option<Box<dyn SidebarHandle>>,  // dyn，不依赖 sidebar crate
}
```

**为什么不循环了？** workspace crate 只依赖 **trait 定义**（自己内部的），sidebar crate 依赖 workspace crate 的 trait 定义（单向）。

---

## 其他可选解法

| 方案 | 描述 | 优点 | 缺点 |
|---|---|---|---|
| **A. dyn object**（Zed 选） | `Box<dyn SidebarHandle>` | 解耦彻底；sidebar crate 可完全独立演进 | 动态分发开销；失去 Entity 强类型 |
| **B. 泛型 MultiWorkspace<S: Sidebar>** | MultiWorkspace 模板化 | 强类型保留；无 dyn 开销 | 污染所有上层类型（AppShell 也要泛型）；GPUI Entity 不支持泛型穿透 |
| **C. Sidebar 不独立 crate** | 留在 `workspace/src/sidebar/` | 零开销；强类型完整 | 违反单一职责；阻碍独立演进；workspace crate 膨胀 |
| **D. 第三 trait crate** | `sidebar-trait` crate 只放 trait | 单向依赖清晰 | 过度工程；crate 数量爆炸 |

Zed 选 A 因为 sidebar crate 足够大（~8000 行）、独立演进需求强，dyn 开销在 UI 层可忽略。

---

## AAgent 当前状态

**暂用方案 C** — Sidebar entity 还在 `workspace/src/sidebar/` 子模块。循环依赖不存在，MultiWorkspace 强类型存 `Entity<Sidebar>`。

### 迁移路径（当 Sidebar 独立到 packages/sidebar/ 时）

```
步骤 1: sidebar.rs        pub trait Sidebar         (强类型 trait — 已定义)
步骤 2: sidebar_handle.rs pub trait SidebarHandle   (dyn object trait — 需补桥接 impl)
步骤 3: sidebar crate     impl Sidebar for SidebarEntity
步骤 4: MultiWorkspace   sidebar: Option<Box<dyn SidebarHandle>>
```

迁移路径已铺平 — 只需补 SidebarHandle dyn trait + 桥接 impl + 改字段类型。

### 关键文件

| 文件 | 内容 |
|---|---|
| `workspace/src/multi_workspace/sidebar.rs` | `pub trait Sidebar`（强类型） |
| `workspace/src/multi_workspace/sidebar_handle.rs` | `pub trait SidebarHandle`（dyn object）+ `impl<T> SidebarHandle for Entity<T>` 桥接 |
| `workspace/src/sidebar/mod.rs` | Sidebar entity，`impl Sidebar for Sidebar` |
| `workspace/src/multi_workspace/sidebar_render_state.rs` | `SidebarRenderState`（只读投影） |

---

## Zed 源码参考

| 位置 | 内容 |
|---|---|
| `crates/workspace/src/multi_workspace.rs:64` | `SidebarRenderState` |
| `crates/workspace/src/multi_workspace.rs:121` | `pub trait Sidebar` |
| `crates/workspace/src/multi_workspace.rs:162` | `pub trait SidebarHandle` |
| `crates/workspace/src/multi_workspace.rs:191` | `impl<T: Sidebar> SidebarHandle for Entity<T>` |
| `crates/workspace/src/multi_workspace.rs:316` | `sidebar: Option<Box<dyn SidebarHandle>>` |
| `crates/sidebar/src/sidebar.rs` | Zed 的 Sidebar entity 实现 |
