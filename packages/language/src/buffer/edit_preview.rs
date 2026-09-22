use super::*;

#[derive(Clone)]
pub struct EditPreview {
    pub(crate) old_snapshot: text::BufferSnapshot,
    pub(crate) applied_edits_snapshot: text::BufferSnapshot,
    pub(crate) syntax_snapshot: SyntaxSnapshot,
}

impl EditPreview {
    pub fn unchanged(snapshot: &BufferSnapshot) -> Self {
        Self {
            old_snapshot: snapshot.text.clone(),
            applied_edits_snapshot: snapshot.text.clone(),
            syntax_snapshot: snapshot.syntax.clone(),
        }
    }

    pub fn as_unified_diff(
        &self,
        file: Option<&Arc<dyn File>>,
        edits: &[(Range<Anchor>, impl AsRef<str>)],
    ) -> Option<String> {
        let (first, _) = edits.first()?;
        let (last, _) = edits.last()?;

        let start = first.start.to_point(&self.old_snapshot);
        let old_end = last.end.to_point(&self.old_snapshot);
        let new_end = last
            .end
            .bias_right(&self.old_snapshot)
            .to_point(&self.applied_edits_snapshot);

        let start = Point::new(start.row.saturating_sub(3), 0);
        let old_end = Point::new(old_end.row + 4, 0).min(self.old_snapshot.max_point());
        let new_end = Point::new(new_end.row + 4, 0).min(self.applied_edits_snapshot.max_point());

        let diff_body = unified_diff_with_offsets(
            &self
                .old_snapshot
                .text_for_range(start..old_end)
                .collect::<String>(),
            &self
                .applied_edits_snapshot
                .text_for_range(start..new_end)
                .collect::<String>(),
            start.row,
            start.row,
        );

        let path = file.map(|f| f.path().as_unix_str());
        let header = match path {
            Some(p) => format!("--- a/{}\n+++ b/{}\n", p, p),
            None => String::new(),
        };

        Some(format!("{}{}", header, diff_body))
    }

    pub fn highlight_edits(
        &self,
        current_snapshot: &BufferSnapshot,
        edits: &[(Range<Anchor>, impl AsRef<str>)],
        include_deletions: bool,
        cx: &App,
    ) -> HighlightedText {
        let Some(visible_range_in_preview_snapshot) = self.compute_visible_range(edits) else {
            return HighlightedText::default();
        };

        let mut highlighted_text = HighlightedTextBuilder::default();

        let visible_range_in_preview_snapshot =
            visible_range_in_preview_snapshot.to_offset(&self.applied_edits_snapshot);
        let mut offset_in_preview_snapshot = visible_range_in_preview_snapshot.start;

        let insertion_highlight_style = HighlightStyle {
            background_color: Some(cx.theme().status().created_background),
            ..Default::default()
        };
        let deletion_highlight_style = HighlightStyle {
            background_color: Some(cx.theme().status().deleted_background),
            ..Default::default()
        };
        let syntax_theme = cx.theme().syntax();

        for (range, edit_text) in edits {
            let edit_new_end_in_preview_snapshot = range
                .end
                .bias_right(&self.old_snapshot)
                .to_offset(&self.applied_edits_snapshot);
            let edit_start_in_preview_snapshot =
                edit_new_end_in_preview_snapshot - edit_text.as_ref().len();

            let unchanged_range_in_preview_snapshot =
                offset_in_preview_snapshot..edit_start_in_preview_snapshot;
            if !unchanged_range_in_preview_snapshot.is_empty() {
                highlighted_text.add_text_from_buffer_range(
                    unchanged_range_in_preview_snapshot,
                    &self.applied_edits_snapshot,
                    &self.syntax_snapshot,
                    None,
                    syntax_theme,
                );
            }

            let range_in_current_snapshot = range.to_offset(current_snapshot);
            if include_deletions && !range_in_current_snapshot.is_empty() {
                highlighted_text.add_text_from_buffer_range(
                    range_in_current_snapshot,
                    &current_snapshot.text,
                    &current_snapshot.syntax,
                    Some(deletion_highlight_style),
                    syntax_theme,
                );
            }

            if !edit_text.as_ref().is_empty() {
                highlighted_text.add_text_from_buffer_range(
                    edit_start_in_preview_snapshot..edit_new_end_in_preview_snapshot,
                    &self.applied_edits_snapshot,
                    &self.syntax_snapshot,
                    Some(insertion_highlight_style),
                    syntax_theme,
                );
            }

            offset_in_preview_snapshot = edit_new_end_in_preview_snapshot;
        }

        highlighted_text.add_text_from_buffer_range(
            offset_in_preview_snapshot..visible_range_in_preview_snapshot.end,
            &self.applied_edits_snapshot,
            &self.syntax_snapshot,
            None,
            syntax_theme,
        );

        highlighted_text.build()
    }

    pub fn build_result_buffer(&self, cx: &mut App) -> Entity<Buffer> {
        cx.new(|cx| {
            let mut buffer = Buffer::local_normalized(
                self.applied_edits_snapshot.as_rope().clone(),
                self.applied_edits_snapshot.line_ending(),
                cx,
            );
            buffer.set_language_async(self.syntax_snapshot.root_language(), cx);
            buffer
        })
    }

    pub fn result_text_snapshot(&self) -> &text::BufferSnapshot { &self.applied_edits_snapshot }

    pub fn result_syntax_snapshot(&self) -> &SyntaxSnapshot { &self.syntax_snapshot }

    pub fn anchor_to_offset_in_result(&self, anchor: Anchor) -> usize {
        anchor
            .bias_right(&self.old_snapshot)
            .to_offset(&self.applied_edits_snapshot)
    }

    pub fn compute_visible_range<T>(&self, edits: &[(Range<Anchor>, T)]) -> Option<Range<Point>> {
        let (first, _) = edits.first()?;
        let (last, _) = edits.last()?;

        let start = first
            .start
            .bias_left(&self.old_snapshot)
            .to_point(&self.applied_edits_snapshot);
        let end = last
            .end
            .bias_right(&self.old_snapshot)
            .to_point(&self.applied_edits_snapshot);

        // Ensure that the first line of the first edit and the last line of the last edit are always fully visible
        let range = Point::new(start.row, 0)
            ..Point::new(end.row, self.applied_edits_snapshot.line_len(end.row));

        Some(range)
    }
}

pub struct EditedBufferSnapshot {
    text: text::EditedBufferSnapshot,
    snapshot: BufferSnapshot,
}

impl EditedBufferSnapshot {
    pub fn snapshot(&self) -> &BufferSnapshot { &self.snapshot }

    pub fn base_version(&self) -> &clock::Global { &self.text.base_version }
}
