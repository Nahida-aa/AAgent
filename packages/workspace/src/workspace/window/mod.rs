// ├── window/
// │   ├── mod.rs
// │   ├── title/
// │   │   ├── mod.rs
// │   │   ├── needs.rs               # WindowTitleNeeds
// │   │   ├── context.rs             # WindowTitleContext
// │   │   ├── format.rs              # parse/render_window_title_format
// │   │   └── render.rs              # update_window_title, apply_window_title / project_window_title
// │   ├── bounds.rs                  # ZED_WINDOW_SIZE/POSITION / window_bounds_env_override, 尺寸/位置/display/环境变量/恢复, save_window_bounds, restore
// │   ├── chrome.rs                  # owns_window_chrome / edited indicator, edited 状态 /  refresh, is_window_edited / update_window_edited / refresh_window_state
// │   └── decorations.rs             # client_side_decorations / resize_edge, 客户端装饰 / resize edge

// ZED_WINDOW_SIZE / ZED_WINDOW_POSITION / window_bounds_env_override / parse_pixel_*：环境变量覆盖，测试与调试用。

// save_window_bounds：把当前窗口几何写进 DB / KVP。

// restore_native_window_state：从 DB 恢复原生窗口状态。

// bounds / bounds_save_task_queued：当前几何和节流任务
mod bounds;
// “chrome”指窗口外壳的非内容部分——标题和编辑标记。核心是 owns_window_chrome：多工作区共享一个平台窗口时，只有活动工作区能写标题和 edited 标记，否则后台工作区的事件会覆盖前台
// owns_window_chrome / is_window_edited / update_window_edited / refresh_window_state
mod chrome;
// FocusablePart / RegionFocusHandles / move_part_focus / move_titlebar_item_focus
mod regions;
// on_window_activation_changed, activate_next_window, activate_previous_window, close_global / window switching
mod activation;
// observe_window_bounds / observe_window_appearance / observe_window_activation 的组装
mod subscriptions;
