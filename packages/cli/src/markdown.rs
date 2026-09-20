use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Tag, TagEnd};
use ratatui::prelude::*;

/// 将 markdown 文本渲染为 Ratatui 行（带样式）。
pub fn render(text: &str) -> Vec<Line<'static>> {
    let parser = pulldown_cmark::Parser::new(text);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_line = StyledLine::new();

    let mut code_block_lang = String::new();
    let mut code_block_lines: Vec<String> = Vec::new();
    let mut in_code_block = false;
    let mut list_indent: u16 = 0;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let style = heading_style(level);
                    current_line.push_str("", style);
                    // Add a prefix for headings
                    let prefix = "#".repeat(level as usize);
                    current_line.push_str(&format!("{} ", prefix), style.bold());
                }
                Tag::CodeBlock(kind) => {
                    code_block_lang = match kind {
                        CodeBlockKind::Fenced(lang) => lang.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    code_block_lines.clear();
                    in_code_block = true;
                }
                Tag::List(_) => {
                    list_indent += 2;
                }
                Tag::Item => {
                    if current_line.has_content() {
                        lines.extend(current_line.take());
                    }
                    let indent = "  ".repeat(list_indent as usize);
                    current_line.push_str(&format!("{}• ", indent), Style::new().dim());
                }
                Tag::BlockQuote(_) => {
                    current_line.push_str("│ ", Style::new().dark_gray());
                }
                Tag::Paragraph => {}
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    lines.extend(current_line.take());
                    lines.push(Line::from(Span::raw("")));
                }
                TagEnd::CodeBlock => {
                    let lang_display = if code_block_lang.is_empty() {
                        String::new()
                    } else {
                        format!("{} ", code_block_lang)
                    };
                    if !lang_display.is_empty() {
                        lines.push(Line::from(Span::styled(
                            format!("  {}┌─ {}", " ".repeat(2), lang_display.trim()),
                            Style::new().dim(),
                        )));
                    }
                    for code_line in &code_block_lines {
                        lines.push(Line::from(Span::styled(
                            format!("  {}│ {}", " ".repeat(2), code_line),
                            Style::new().fg(Color::DarkGray),
                        )));
                    }
                    lines.push(Line::from(Span::styled(
                        format!("  {}└─", " ".repeat(2)),
                        Style::new().dim(),
                    )));
                    in_code_block = false;
                }
                TagEnd::List(_) => {
                    list_indent = list_indent.saturating_sub(2);
                }
                TagEnd::Item => {}
                TagEnd::Paragraph => {
                    if current_line.has_content() {
                        lines.extend(current_line.take());
                    }
                    lines.push(Line::from(Span::raw("")));
                }
                _ => {}
            },
            Event::Text(t) => {
                if in_code_block {
                    code_block_lines.push(t.to_string());
                } else {
                    current_line.push_str(&t, Style::new());
                }
            }
            Event::Code(t) => {
                current_line.push_str(&t, Style::new().bg(Color::DarkGray).dim());
            }
            Event::SoftBreak | Event::HardBreak => {
                lines.extend(current_line.take());
            }
            Event::Rule => {
                lines.push(Line::from(Span::styled("─".repeat(40), Style::new().dim())));
            }
            _ => {}
        }
    }

    if current_line.has_content() {
        lines.extend(current_line.take());
    }

    lines
}

fn heading_style(level: HeadingLevel) -> Style {
    match level {
        HeadingLevel::H1 => Style::new().fg(Color::Cyan).bold(),
        HeadingLevel::H2 => Style::new().fg(Color::Green).bold(),
        HeadingLevel::H3 => Style::new().fg(Color::Yellow).bold(),
        _ => Style::new().fg(Color::White).bold(),
    }
}

/// Helper to build a line incrementally with styled segments.
struct StyledLine {
    spans: Vec<Span<'static>>,
    buffer: String,
    style: Style,
}

impl StyledLine {
    fn new() -> Self {
        Self {
            spans: Vec::new(),
            buffer: String::new(),
            style: Style::new(),
        }
    }

    fn push_str(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        // Instead of merging, just flush and add a new span
        self.flush();
        self.spans.push(Span::styled(text.to_string(), style));
    }

    fn has_content(&self) -> bool { !self.spans.is_empty() || !self.buffer.is_empty() }

    fn flush(&mut self) {
        if !self.buffer.is_empty() {
            let text = std::mem::take(&mut self.buffer);
            let style = self.style;
            self.spans.push(Span::styled(text, style));
        }
    }

    fn take(&mut self) -> Vec<Line<'static>> {
        self.flush();
        let spans = std::mem::take(&mut self.spans);
        if spans.is_empty() {
            vec![Line::from(Span::raw(""))]
        } else {
            vec![Line::from(spans)]
        }
    }
}
