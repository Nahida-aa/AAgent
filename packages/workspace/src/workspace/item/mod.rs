// Item	对应文件吗
// Editor	是，对应一个 Buffer
// Terminal	否，没有文件
// SharedScreen	否，是别人的屏幕
// WelcomePage	否
// InvalidItemView	是，但文件打不开
// Imageviewer	是，但不是文本 Buffer
// ProjectSearch（Multibuffer）	对应多个文件
// Diff view	对应两个文件
// Notebook	对应一个文件，但结构特殊
//
// ProjectItem 和 Item 的区别
// ProjectItem：模型层，描述“项目里有什么可以被打开”。比如 Buffer、图片。

// Item：UI 层，描述“Pane 里能显示什么”。

// Editor 同时实现两者：impl ProjectItem for Editor、impl Item for Editor。

// Terminal 只实现 Item，不实现 ProjectItem。

// ProjectItemRegistry 注册的是“某类模型用什么 UI 打开”，比如 Buffer → Editor
// open_item_abs_paths
use super::*;
mod read;
mod ops;
// serialize_items
mod serialize;
// open_file_permalink, copy_file_permalink, handle_file_permalink
pub(crate) mod permalink;
//
mod save;
pub mod close;
pub use events::{ItemBufferKind};
