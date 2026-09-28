# GPUI Action Dispatch: Bubble Window Phase vs Bubble Global

> Debug session 2026-09-29, root cause of WelcomePage "Open Project" opening new window instead of reusing.

## The Bug

Clicking WelcomePage "Open Project" → new window created instead of opening in current window.

Tracing showed `multi_workspace.read(cx)` returning `WindowNotFound` during `workspace_windows_for_location()`.

## Root Cause: Window Slot Take + Bubble Phase Mismatch

### GPUI Window Slot Mechanism

`App::update()` and `Window::dispatch_action()` internally call `update_window_erased()`:

```rust
// gpui app.rs
windows.get_mut(id).and_then(Option::take)  // take slot
// ... execute closure ...
windows[id] = Some(window)                  // put back
```

During the take, `AnyWindowHandle.read()` → `windows.get(id).as_deref()` returns `None` → `WindowNotFound`.

### Action Dispatch Paths

1. **Window::dispatch_action** (internal, called by both menu and button paths):
   - Creates a **deferred** closure that calls `update_window_erased(take #2)`
   - The deferred closure executes AFTER `Window::dispatch_action` returns
   - So the entire Bubble phase (including Bubble global) runs DURING take #2

2. **FocusHandle::dispatch_action** (WelcomePage SectionButton):
   - Directly calls `window.dispatch_action_on_node`
   - Same inner defer → same take #2 issue

### Two Action Listener Registration Levels

| Level | API | When Registered | Phase | window slot state |
|---|---|---|---|---|
| **Bubble global** | `cx.on_action(&Open, ...)` | App init | During `Window::dispatch_action` defer → take #2 | **None** ❌ |
| **Bubble window** | `workspace.register_action(...)` | Workspace entity render | On dispatch path entities | **Intact** ✅ |

### Why Zed Works

Zed registers `&workspace::Open` as **Bubble window phase** on the Workspace entity:

```rust
// zed crates/zed/src/zed.rs register_actions()
workspace
    .register_action(|workspace, action: &workspace::Open, window, cx| {
        workspace::prompt_for_open_path_and_open(workspace, ...);
    })
```

`register_action` (zed workspace.rs L8498) pushes a closure to `workspace_actions`, which `Workspace::render()` calls on the root div chain:

```rust
pub fn register_action<A: Action>(
    &mut self,
    callback: impl Fn(&mut Self, &A, &mut Window, &mut Context<Self>) + 'static,
) -> &mut Self {
    self.workspace_actions.push(Box::new(move |div, _, _, cx| {
        div.on_action(cx.listener(move |workspace, event, window, cx| {
            (callback)(workspace, event, window, cx)
        }))
    }));
    self
}
```

This registers on the **entity's dispatch path** → Bubble window phase → executes BEFORE Bubble global → **propagate_event = false** → Bubble global never runs → no slot take issue.

### Why aacode Failed

aacode had only Bubble global:

```rust
// BAD: Bubble global runs during take #2
cx.on_action(|action: &Open, cx: &mut App| {
    prompt_and_open_paths(app_state, ..., cx);  // read() fails → creates new window
});
```

No Workspace entity Bubble window phase Open listener → `propagate_event = true` → continues to Bubble global → runs during take #2 → `read()` fails.

## The Fix

1. Remove Open/OpenFiles from Bubble global (`cx.on_action`)
2. Register them as Bubble window phase on Workspace entity via `cx.observe_new` + `workspace.register_action`

```rust
cx.observe_new(move |workspace: &mut Workspace, window, cx| {
    workspace
        .register_action({
            let app_state = app_state.clone();
            move |workspace, action: &Open, window, cx| {
                prompt_for_open_path_and_open(workspace, app_state.clone(), ...);
            }
        })
        .register_action({
            let app_state = app_state.clone();
            move |workspace, _: &OpenFiles, window, cx| {
                prompt_for_open_path_and_open(workspace, app_state.clone(), ...);
            }
        });
})
.detach();
```

`prompt_for_open_path_and_open` takes `&mut Workspace, &mut Window, &mut Context<Workspace>` — matches the Bubble window phase callback signature, unlike the old Bubble global callback which took `&mut App`.

## Debug Techniques Used

### 1. Zed Local Build + File Tracing

Added tracing to Zed local repo (`/home/aa/repos/learn_ls/zed`, same gpui rev `d3ccd5719`):

```rust
// workspace.rs init
std::fs::write("/tmp/zed_trace.txt", "workspace::init CALLED\n").ok();

// welcome.rs SectionButton click
std::fs::OpenOptions::new().create(true).append(true)
    .open("/tmp/zed_trace.txt").ok()
    .and_then(|mut f| { writeln!(f, "SectionButton clicked").ok(); Some(()) });
```

File-based tracing avoids stderr/stdout being swallowed by Wayland subtrees.

### 2. GPUI DispatchTree Node Tracing

In `dispatch_action_on_node_inner`, trace which listener fires:

```rust
for node_id in dispatch_path.iter().rev() {
    let view_id = self.rendered_frame.dispatch_tree.node_view_id(*node_id);
    let node = self.rendered_frame.dispatch_tree.node(*node_id);
    for DispatchActionListener { action_type, listener } in node.action_listeners.clone() {
        if action_type == any_action.type_id() {
            // TRACE: Bubble window listener HIT on node_id, view_id
            cx.propagate_event = false;  // ← THIS is the key! Stops Bubble global!
            listener(any_action, DispatchPhase::Bubble, self, cx);
        }
    }
}
```

Key finding: Zed dispatch_path node 3 (view_id=None, register_action registers on div chain not entity) had Bubble window phase Open listener with matching TypeId. aacode had none.

### 3. TypeId Comparison

Printed `TypeId::of::<workspace::Open>()` in Zed workspace init and compared with GPUI tracing output:

```
workspace::init CALLED, Open type_id=TypeId(0xe859bde91e60acf05209afc96785b675)
[GPUI] Bubble window listener HIT on node DispatchNodeId(3)
[GPUI]   action_type_id=TypeId(0xe859bde91e60acf05209afc96785b675)
```

Exact match confirmed the Bubble window listener was indeed `&workspace::Open`.

## Key Insights

1. **`Window::dispatch_action` has internal defer** — ANY path through it (FocusHandle::dispatch_action or menu) ends up with Bubble phase running during take #2. This is a fundamental gpui behavior, not a bug.

2. **Bubble phase order matters** — Bubble window phase runs before Bubble global. `propagate_event = false` on Bubble window stops propagation → Bubble global never fires.

3. **`register_action` vs `on_action`** — `workspace.register_action` registers on entity's dispatch path (Bubble window). `cx.on_action` registers globally (Bubble global). Use the former for any action that needs window state during execution.

4. **view_id=None on register_action nodes** — `register_action` pushes closures that render div chains with `.on_action(cx.listener(...))`. These are element-level listeners (view_id=None), not entity-level. That's expected.

5. **Always prefer code alignment over behavior assumptions** — User was right: "逻辑相同但代码不同不能让相同，因为你对逻辑的判断可能是错的". We tried defer (hack) for hours until we found Zed has no defer and works fine because it uses a different listener phase.

## Files Changed

| File | Change |
|---|---|
| `packages/workspace/src/workspace/app/initial.rs` | Open/OpenFiles from `cx.on_action` (Bubble global) → `cx.observe_new + workspace.register_action` (Bubble window) |
| `packages/workspace/src/workspace/open/windows.rs` | Removed `backtrace::Backtrace` debug code (crate wasn't added as dep) |

## Related Zed Source

| Location | Purpose |
|---|---|
| `crates/zed/src/zed.rs:register_actions()` | Zed's Bubble window Open/OpenFiles registration |
| `crates/workspace/src/workspace.rs:register_action()` | Implementation — pushes to `workspace_actions`, called during render |
| `crates/gpui/src/window.rs:dispatch_action_on_node_inner()` | Bubble phase dispatch loop, `propagate_event = false` stops Bubble global |
| `crates/gpui/src/app.rs:update_window_erased()` | The `Option::take` + closure execution + put-back slot mechanism |

## Commit & Tag

```
commit a6b4bca
tag v0.1.0-open-window-fix
```
