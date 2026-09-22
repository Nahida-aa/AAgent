use gpui::SharedString;

/// This is a new type representing a 'tag' on a 'runnable symbol', typically a test of main() function, found via treesitter.
#[derive(Clone, Debug)]
pub struct RunnableTag(pub SharedString);
