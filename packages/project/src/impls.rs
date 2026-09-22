use gpui::{App, AppContext as _, Entity};
use language::{Buffer, Capability, DiskState, File};

use crate::path::ProjectPath;
use crate::types::{Completion, CompletionIntent, CompletionSource};
use crate::Project;
use crate::item::ProjectItem;

impl ProjectItem for Buffer {
    // 原样搬入
}

impl Completion {
    // 原样搬入（kind / label / filter_text / sort_key / is_snippet_kind / is_snippet / color）
}
