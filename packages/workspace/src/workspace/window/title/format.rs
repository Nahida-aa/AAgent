use super::*;
pub(crate) enum WindowTitleTemplatePart<'a> {
    Literal(&'a str),
    Variable(&'a str),
    Separator,
}

pub(crate) fn parse_window_title_format(template: &str) -> Vec<WindowTitleTemplatePart<'_>> {
    let mut parts = Vec::new();
    let mut start = 0;

    // Keep this placeholder scan in sync with the importer in
    // settings/src/vscode_import.rs.
    while let Some(offset) = template[start..].find("${") {
        let variable_start = start + offset;
        if variable_start > start {
            parts.push(WindowTitleTemplatePart::Literal(
                &template[start..variable_start],
            ));
        }

        let content_start = variable_start + 2;
        let Some(content_end_offset) = template[content_start..].find('}') else {
            parts.push(WindowTitleTemplatePart::Literal(
                &template[variable_start..],
            ));
            return parts;
        };

        let content_end = content_start + content_end_offset;
        let variable = &template[content_start..content_end];
        if variable == "separator" {
            parts.push(WindowTitleTemplatePart::Separator);
        } else {
            parts.push(WindowTitleTemplatePart::Variable(variable));
        }

        start = content_end + 1;
    }

    if start < template.len() {
        parts.push(WindowTitleTemplatePart::Literal(&template[start..]));
    }

    parts
}

pub(crate) fn render_window_title_format(
    template: &str,
    separator: &str,
    context: &WindowTitleContext,
) -> String {
    let parts = parse_window_title_format(template);
    let mut segments = Vec::new();
    let mut current_segment = String::new();

    for part in parts {
        match part {
            WindowTitleTemplatePart::Literal(text) => current_segment.push_str(text),
            WindowTitleTemplatePart::Variable(variable) => {
                if let Some(value) = context.value_for(variable) {
                    current_segment.push_str(value);
                }
            }
            WindowTitleTemplatePart::Separator => {
                if !current_segment.is_empty() {
                    segments.push(std::mem::take(&mut current_segment));
                }
            }
        }
    }

    if !current_segment.is_empty() {
        segments.push(current_segment);
    }

    segments.join(separator)
}
