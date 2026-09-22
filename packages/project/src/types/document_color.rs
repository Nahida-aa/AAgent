use super::*;

pub struct DocumentColor {
    pub lsp_range: lsp::Range,
    pub color: lsp::Color,
    pub resolved: bool,
    pub color_presentations: Vec<ColorPresentation>,
}

impl Eq for DocumentColor {}

impl std::hash::Hash for DocumentColor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.lsp_range.hash(state);
        self.color.red.to_bits().hash(state);
        self.color.green.to_bits().hash(state);
        self.color.blue.to_bits().hash(state);
        self.color.alpha.to_bits().hash(state);
        self.resolved.hash(state);
        self.color_presentations.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorPresentation {
    pub label: SharedString,
    pub text_edit: Option<lsp::TextEdit>,
    pub additional_text_edits: Vec<lsp::TextEdit>,
}

impl std::hash::Hash for ColorPresentation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.label.hash(state);
        if let Some(ref edit) = self.text_edit {
            edit.range.hash(state);
            edit.new_text.hash(state);
        }
        self.additional_text_edits.len().hash(state);
        for edit in &self.additional_text_edits {
            edit.range.hash(state);
            edit.new_text.hash(state);
        }
    }
}
