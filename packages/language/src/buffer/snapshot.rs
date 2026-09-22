use super::*;
use ::util::RangeExt;
use ::util::maybe;
use aa_clock::{self as clock, ReplicaId};
use std::sync::Arc;

/// An immutable, cheaply cloneable representation of a fixed
/// state of a buffer.
pub struct BufferSnapshot {
    pub text: text::BufferSnapshot,
    pub(crate) syntax: SyntaxSnapshot,
    pub(crate) tree_sitter_data: Arc<TreeSitterData>,
    pub(crate) diagnostics: TreeMap<LanguageServerId, DiagnosticSet>,
    pub(crate) remote_selections: TreeMap<ReplicaId, SelectionSet>,
    pub(crate) language: Option<Arc<Language>>,
    pub(crate) file: Option<Arc<dyn File>>,
    pub(crate) non_text_state_update_count: usize,
    pub capability: Capability,
    pub(crate) modeline: Option<Arc<ModelineSettings>>,
    pub(crate) resolved_settings: Option<Arc<LanguageSettings>>,
}

#[derive(Clone, Debug)]
pub(crate) struct SelectionSet {
    pub(crate) line_mode: bool,
    pub(crate) cursor_shape: CursorShape,
    pub(crate) selections: Arc<[Selection<Anchor>]>,
    pub(crate) lamport_timestamp: clock::Lamport,
}
/// The shape of a selection cursor.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CursorShape {
    /// A vertical bar
    #[default]
    Bar,
    /// A block that surrounds the following character
    Block,
    /// An underline that runs along the following character
    Underline,
    /// A box drawn around the following character
    Hollow,
}

impl From<settings::CursorShape> for CursorShape {
    fn from(shape: settings::CursorShape) -> Self {
        match shape {
            settings::CursorShape::Bar => CursorShape::Bar,
            settings::CursorShape::Block => CursorShape::Block,
            settings::CursorShape::Underline => CursorShape::Underline,
            settings::CursorShape::Hollow => CursorShape::Hollow,
        }
    }
}

impl BufferSnapshot {
    /// Returns [`IndentSize`] for a given line that respects user settings and
    /// language preferences.
    pub fn indent_size_for_line(&self, row: u32) -> IndentSize { indent_size_for_line(self, row) }

    /// The indentation that the block comment closed on `position`'s row belongs
    /// at, or `None` if that row is not the closing line of a multi-line block
    /// comment.
    ///
    /// A closing delimiter is conventionally indented one column past its opening
    /// delimiter, so that it lines up with the comment's prefixes:
    ///
    /// ```text
    /// /**
    ///  * doc
    ///  */
    /// ```
    ///
    /// That extra column belongs to the comment rather than to the surrounding
    /// code, which makes the closing row's own indentation a poor basis for
    /// indenting whatever follows it. The opening row's indentation is used
    /// instead.
    ///
    /// Requires the row to hold nothing but whitespace and the closing delimiter,
    /// and `position` to be at or past the end of that delimiter, so that
    /// splitting the delimiter itself is left alone. Also requires the syntax node
    /// containing the delimiter to end there, so that a line which merely looks
    /// like a closing delimiter is not mistaken for one.
    pub fn block_comment_closing_indent(&self, position: Point) -> Option<IndentSize> {
        let row = position.row;
        let indent_len = self.indent_size_for_line(row).len;
        let delimiter_start = Point::new(row, indent_len);
        let language = self.language_scope_at(delimiter_start)?;
        // A few languages describe a string literal in `block_comment` rather
        // than a comment (Python's `"""`), so either scope is accepted. Relying
        // on the override scope keeps this out of the business of guessing at
        // grammar node names.
        if !matches!(language.override_name(), Some("comment" | "string")) {
            return None;
        }

        let delimiter_len = [language.documentation_comment(), language.block_comment()]
            .into_iter()
            .flatten()
            .find_map(|config| {
                let delimiter = config.end.trim_start();
                if delimiter.is_empty() {
                    return None;
                }
                let mut chars = self.chars_at(delimiter_start);
                if !delimiter
                    .chars()
                    .all(|expected| chars.next() == Some(expected))
                {
                    return None;
                }
                if !chars.take_while(|c| *c != '\n').all(char::is_whitespace) {
                    return None;
                }
                Some(delimiter.len() as u32)
            })?;
        if position.column < indent_len + delimiter_len {
            return None;
        }

        let delimiter_end = Point::new(row, indent_len + delimiter_len);
        let node = self.syntax_ancestor(delimiter_start..delimiter_end)?;
        if Point::from_ts_point(node.end_position()) != delimiter_end {
            return None;
        }
        let opening_row = Point::from_ts_point(node.start_position()).row;
        (opening_row < row).then(|| self.indent_size_for_line(opening_row))
    }

    /// Like [`Self::indent_size_for_line`], but reports the indentation a row
    /// logically sits at, which differs from its physical indentation on the
    /// closing line of a block comment. See [`Self::block_comment_closing_indent`].
    pub(crate) fn logical_indent_size_for_line(&self, row: u32) -> IndentSize {
        self.block_comment_closing_indent(Point::new(row, self.line_len(row)))
            .unwrap_or_else(|| self.indent_size_for_line(row))
    }

    /// Returns [`IndentSize`] for a given position that respects user settings
    /// and language preferences.
    pub fn language_indent_size_at<T: ToOffset>(&self, position: T, cx: &App) -> IndentSize {
        let settings = self.settings_at(position, cx);
        if settings.hard_tabs {
            IndentSize::tab()
        } else {
            IndentSize::spaces(settings.tab_size.get())
        }
    }

    /// Retrieve the suggested indent size for all of the given rows. The unit of indentation
    /// is passed in as `single_indent_size`.
    pub fn suggested_indents(
        &self,
        rows: impl Iterator<Item = u32>,
        single_indent_size: IndentSize,
    ) -> BTreeMap<u32, IndentSize> {
        let mut result = BTreeMap::new();

        for row_range in contiguous_ranges(rows, 10) {
            let suggestions = match self.suggest_autoindents(row_range.clone()) {
                Some(suggestions) => suggestions,
                _ => break,
            };

            for (row, suggestion) in row_range.zip(suggestions) {
                let indent_size = if let Some(suggestion) = suggestion {
                    result
                        .get(&suggestion.basis_row)
                        .copied()
                        .unwrap_or_else(|| self.logical_indent_size_for_line(suggestion.basis_row))
                        .with_delta(suggestion.delta, single_indent_size)
                } else {
                    self.indent_size_for_line(row)
                };

                result.insert(row, indent_size);
            }
        }

        result
    }

    pub(crate) fn suggest_autoindents(
        &self,
        row_range: Range<u32>,
    ) -> Option<impl Iterator<Item = Option<IndentSuggestion>> + '_> {
        let config = &self.language.as_ref()?.config;
        let prev_non_blank_row = self.prev_non_blank_row(row_range.start);

        #[derive(Debug, Clone)]
        struct StartPosition {
            start: Point,
            suffix: SharedString,
            language: Arc<Language>,
        }

        // Find the suggested indentation ranges based on the syntax tree.
        let start = Point::new(prev_non_blank_row.unwrap_or(row_range.start), 0);
        let end = Point::new(row_range.end, 0);
        let range = (start..end).to_offset(&self.text);
        let mut matches = self.syntax.matches_with_options(
            range.clone(),
            &self.text,
            TreeSitterOptions {
                max_bytes_to_query: Some(MAX_BYTES_TO_QUERY),
                max_start_depth: None,
            },
            |grammar| Some(&grammar.indents_config.as_ref()?.query),
        );
        let indent_configs = matches
            .grammars()
            .iter()
            .map(|grammar| {
                grammar
                    .indents_config
                    .as_ref()
                    .expect("grammar in indent match set has indents_config")
            })
            .collect::<Vec<_>>();

        let mut indent_ranges = Vec::<Range<Point>>::new();
        let mut start_positions = Vec::<StartPosition>::new();
        let mut outdent_positions = Vec::<Point>::new();
        while let Some(mat) = matches.peek() {
            let mut start: Option<Point> = None;
            let mut end: Option<Point> = None;

            let config = indent_configs[mat.grammar_index];
            for capture in mat.captures {
                if capture.index == config.indent_capture_ix {
                    start.get_or_insert(Point::from_ts_point(capture.node.start_position()));
                    end.get_or_insert(Point::from_ts_point(capture.node.end_position()));
                } else if Some(capture.index) == config.start_capture_ix {
                    start = Some(Point::from_ts_point(capture.node.end_position()));
                } else if Some(capture.index) == config.end_capture_ix {
                    end = Some(Point::from_ts_point(capture.node.start_position()));
                } else if Some(capture.index) == config.outdent_capture_ix {
                    outdent_positions.push(Point::from_ts_point(capture.node.start_position()));
                } else if let Some(suffix) = config.suffixed_start_captures.get(&capture.index) {
                    start_positions.push(StartPosition {
                        start: Point::from_ts_point(capture.node.start_position()),
                        suffix: suffix.clone(),
                        language: mat.language.clone(),
                    });
                }
            }

            matches.advance();
            if let Some((start, end)) = start.zip(end) {
                if start.row == end.row {
                    continue;
                }
                let range = start..end;
                match indent_ranges.binary_search_by_key(&range.start, |r| r.start) {
                    Err(ix) => indent_ranges.insert(ix, range),
                    Ok(ix) => {
                        let prev_range = &mut indent_ranges[ix];
                        prev_range.end = prev_range.end.max(range.end);
                    }
                }
            }
        }

        let mut error_ranges = Vec::<Range<Point>>::new();
        let mut matches = self
            .syntax
            .matches(range, &self.text, |grammar| grammar.error_query.as_ref());
        while let Some(mat) = matches.peek() {
            let node = mat.captures[0].node;
            let start = Point::from_ts_point(node.start_position());
            let end = Point::from_ts_point(node.end_position());
            let range = start..end;
            let ix = match error_ranges.binary_search_by_key(&range.start, |r| r.start) {
                Ok(ix) | Err(ix) => ix,
            };
            let mut end_ix = ix;
            while let Some(existing_range) = error_ranges.get(end_ix) {
                if existing_range.end < end {
                    end_ix += 1;
                } else {
                    break;
                }
            }
            error_ranges.splice(ix..end_ix, [range]);
            matches.advance();
        }

        outdent_positions.sort();
        for outdent_position in outdent_positions {
            // find the innermost indent range containing this outdent_position
            // set its end to the outdent position
            if let Some(range_to_truncate) = indent_ranges
                .iter_mut()
                .rfind(|indent_range| indent_range.contains(&outdent_position))
            {
                range_to_truncate.end = outdent_position;
            }
        }

        start_positions.sort_by_key(|b| b.start);

        // Find the suggested indentation increases and decreased based on regexes.
        let mut regex_outdent_map = HashMap::default();
        let mut last_seen_suffix: HashMap<String, Vec<StartPosition>> = HashMap::default();
        let mut start_positions_iter = start_positions.iter().peekable();

        let mut indent_change_rows = Vec::<(u32, Ordering)>::new();
        self.for_each_line(
            Point::new(prev_non_blank_row.unwrap_or(row_range.start), 0)
                ..Point::new(row_range.end, 0),
            |row, line| {
                let indent_len = self.indent_size_for_line(row).len;
                let row_language = self.language_at(Point::new(row, indent_len)).cloned();
                let row_language_config = row_language
                    .as_ref()
                    .map(|lang| lang.config())
                    .unwrap_or(config);

                if row_language_config
                    .decrease_indent_pattern
                    .as_ref()
                    .is_some_and(|regex| regex.is_match(line))
                {
                    indent_change_rows.push((row, Ordering::Less));
                }
                if row_language_config
                    .increase_indent_pattern
                    .as_ref()
                    .is_some_and(|regex| regex.is_match(line))
                {
                    indent_change_rows.push((row + 1, Ordering::Greater));
                }
                while let Some(pos) = start_positions_iter.peek() {
                    if pos.start.row < row {
                        let pos = start_positions_iter.next().unwrap().clone();
                        last_seen_suffix
                            .entry(pos.suffix.to_string())
                            .or_default()
                            .push(pos);
                    } else {
                        break;
                    }
                }
                for rule in &row_language_config.decrease_indent_patterns {
                    if rule.pattern.as_ref().is_some_and(|r| r.is_match(line)) {
                        let row_start_column = self.indent_size_for_line(row).len;
                        let basis_row = rule
                            .valid_after
                            .iter()
                            .filter_map(|valid_suffix| last_seen_suffix.get(valid_suffix))
                            .flatten()
                            .filter(|pos| {
                                row_language
                                    .as_ref()
                                    .or(self.language.as_ref())
                                    .is_some_and(|lang| Arc::ptr_eq(lang, &pos.language))
                            })
                            .filter(|pos| pos.start.column <= row_start_column)
                            .max_by_key(|pos| pos.start.row);
                        if let Some(outdent_to) = basis_row {
                            regex_outdent_map.insert(row, outdent_to.start.row);
                        }
                        break;
                    }
                }
            },
        );

        let mut indent_changes = indent_change_rows.into_iter().peekable();
        let mut prev_row = if config.auto_indent_using_last_non_empty_line {
            prev_non_blank_row.unwrap_or(0)
        } else {
            row_range.start.saturating_sub(1)
        };

        let mut prev_row_start = Point::new(prev_row, self.indent_size_for_line(prev_row).len);
        Some(row_range.map(move |row| {
            let row_start = Point::new(row, self.indent_size_for_line(row).len);

            let mut indent_from_prev_row = false;
            let mut outdent_from_prev_row = false;
            let mut outdent_to_row = u32::MAX;
            let mut from_regex = false;

            while let Some((indent_row, delta)) = indent_changes.peek() {
                match indent_row.cmp(&row) {
                    Ordering::Equal => match delta {
                        Ordering::Less => {
                            from_regex = true;
                            outdent_from_prev_row = true
                        }
                        Ordering::Greater => {
                            indent_from_prev_row = true;
                            from_regex = true
                        }
                        _ => {}
                    },

                    Ordering::Greater => break,
                    Ordering::Less => {}
                }

                indent_changes.next();
            }

            for range in &indent_ranges {
                if range.start.row >= row {
                    break;
                }
                if range.start.row == prev_row && range.end > row_start {
                    indent_from_prev_row = true;
                }
                if range.end > prev_row_start && range.end <= row_start {
                    outdent_to_row = outdent_to_row.min(range.start.row);
                }
            }

            if let Some(basis_row) = regex_outdent_map.get(&row) {
                indent_from_prev_row = false;
                outdent_to_row = *basis_row;
                from_regex = true;
            }

            let within_error = error_ranges
                .iter()
                .any(|e| e.start.row < row && e.end > row_start);

            let suggestion = if outdent_to_row == prev_row
                || (outdent_from_prev_row && indent_from_prev_row)
            {
                Some(IndentSuggestion {
                    basis_row: prev_row,
                    delta: Ordering::Equal,
                    within_error: within_error && !from_regex,
                })
            } else if indent_from_prev_row {
                Some(IndentSuggestion {
                    basis_row: prev_row,
                    delta: Ordering::Greater,
                    within_error: within_error && !from_regex,
                })
            } else if outdent_to_row < prev_row {
                Some(IndentSuggestion {
                    basis_row: outdent_to_row,
                    delta: Ordering::Equal,
                    within_error: within_error && !from_regex,
                })
            } else if outdent_from_prev_row {
                Some(IndentSuggestion {
                    basis_row: prev_row,
                    delta: Ordering::Less,
                    within_error: within_error && !from_regex,
                })
            } else if config.auto_indent_using_last_non_empty_line || !self.is_line_blank(prev_row)
            {
                Some(IndentSuggestion {
                    basis_row: prev_row,
                    delta: Ordering::Equal,
                    within_error: within_error && !from_regex,
                })
            } else {
                None
            };

            prev_row = row;
            prev_row_start = row_start;
            suggestion
        }))
    }

    fn prev_non_blank_row(&self, mut row: u32) -> Option<u32> {
        while row > 0 {
            row -= 1;
            if !self.is_line_blank(row) {
                return Some(row);
            }
        }
        None
    }

    pub fn captures(
        &self,
        range: Range<usize>,
        query: fn(&Grammar) -> Option<&tree_sitter::Query>,
    ) -> SyntaxMapCaptures<'_> {
        self.syntax.captures(range, &self.text, query)
    }

    #[a_tracing::instrument(skip_all)]
    pub(crate) fn get_highlights(
        &self,
        range: Range<usize>,
    ) -> (SyntaxMapCaptures<'_>, Vec<HighlightMap>) {
        let captures = self.syntax.captures(range, &self.text, |grammar| {
            grammar
                .highlights_config
                .as_ref()
                .map(|config| &config.query)
        });
        let highlight_maps = captures
            .grammars()
            .iter()
            .map(|grammar| grammar.highlight_map())
            .collect();
        (captures, highlight_maps)
    }

    /// Iterates over chunks of text in the given range of the buffer. Text is chunked
    /// in an arbitrary way due to being stored in a [`Rope`](text::Rope). The text is also
    /// returned in chunks where each chunk has a single syntax highlighting style and
    /// diagnostic status.
    #[a_tracing::instrument(skip_all)]
    pub fn chunks<T: ToOffset>(
        &self,
        range: Range<T>,
        language_aware: LanguageAwareStyling,
    ) -> BufferChunks<'_> {
        let range = range.start.to_offset(self)..range.end.to_offset(self);

        let mut syntax = None;
        if language_aware.tree_sitter {
            match self.cached_highlight_runs(range.clone()) {
                Some(runs) => {
                    return BufferChunks::with_cached_highlights(
                        self.text.as_rope(),
                        range,
                        runs,
                        language_aware.diagnostics,
                        self,
                    );
                }
                None => syntax = Some(self.get_highlights(range.clone())),
            }
        }
        BufferChunks::new(
            self.text.as_rope(),
            range,
            syntax,
            language_aware.diagnostics,
            Some(self),
        )
    }

    pub(crate) fn cached_highlight_runs(&self, range: Range<usize>) -> Option<Vec<HighlightRun>> {
        #[cfg(any(test, feature = "test-support"))]
        {
            static DISABLE_HIGHLIGHT_CACHE: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| {
                    std::env::var_os("ZED_DISABLE_HIGHLIGHT_CACHE").is_some()
                });
            if *DISABLE_HIGHLIGHT_CACHE {
                return None;
            }
        }
        self.language.as_ref()?.grammar()?;
        if range.is_empty() {
            return Some(Vec::new());
        }
        let mut runs = Vec::<HighlightRun>::new();
        for chunk in self
            .tree_sitter_data
            .chunks
            .applicable_chunks(&[range.to_point(self)])
        {
            let chunk_range = chunk.anchor_range().to_offset(self);
            if chunk_range.end <= range.start || chunk_range.start >= range.end {
                continue;
            }
            if chunk_range.len() > MAX_BYTES_TO_HIGHLIGHT_IN_A_CHUNK {
                return None;
            }
            let chunk_highlights = match self.tree_sitter_data.highlights_by_chunks.get(chunk.id) {
                Some(chunk_highlights) => chunk_highlights,
                None => {
                    let chunk_highlights = self.compute_chunk_highlights(chunk_range);
                    self.tree_sitter_data
                        .highlights_by_chunks
                        .insert(chunk.id, chunk_highlights.clone());
                    chunk_highlights
                }
            };
            for (run_range, highlight_id) in chunk_highlights.runs.iter() {
                if run_range.end <= range.start {
                    continue;
                }
                if run_range.start >= range.end {
                    break;
                }
                match runs.last_mut() {
                    Some((last_range, last_highlight_id))
                        if last_highlight_id == highlight_id
                            && last_range.end == run_range.start =>
                    {
                        last_range.end = run_range.end;
                    }
                    _ => runs.push((run_range.clone(), *highlight_id)),
                }
            }
        }
        Some(runs)
    }

    fn compute_chunk_highlights(&self, range: Range<usize>) -> ResolvedHighlights {
        let captures = self.syntax.captures(range.clone(), &self.text, |grammar| {
            grammar
                .highlights_config
                .as_ref()
                .map(|config| &config.query)
        });
        let sources = captures
            .grammars()
            .iter()
            .map(|&grammar| (Arc::clone(grammar), grammar.highlight_map()))
            .collect::<SmallVec<[(Arc<Grammar>, HighlightMap); 2]>>();
        let mut runs = Vec::<(Range<usize>, HighlightId)>::new();
        for region in flattened_highlight_regions(captures, range) {
            let highlight_id = region.stack.iter().rev().find_map(|capture| {
                let (_, highlight_map) = sources.get(capture.grammar_index)?;
                highlight_map.get(capture.capture_id)
            });
            let Some(highlight_id) = highlight_id else {
                continue;
            };
            match runs.last_mut() {
                Some((last_range, last_highlight_id))
                    if *last_highlight_id == highlight_id
                        && last_range.end == region.range.start =>
                {
                    last_range.end = region.range.end;
                }
                _ => runs.push((region.range, highlight_id)),
            }
        }
        ResolvedHighlights {
            sources,
            runs: runs.into(),
        }
    }

    pub fn highlighted_text_for_range<T: ToOffset>(
        &self,
        range: Range<T>,
        override_style: Option<HighlightStyle>,
        syntax_theme: &SyntaxTheme,
    ) -> HighlightedText {
        HighlightedText::from_buffer_range(
            range,
            &self.text,
            &self.syntax,
            override_style,
            syntax_theme,
        )
    }

    /// Invokes the given callback for each line of text in the given range of the buffer.
    /// Uses callback to avoid allocating a string for each line.
    fn for_each_line(&self, range: Range<Point>, mut callback: impl FnMut(u32, &str)) {
        let mut line = String::new();
        let mut row = range.start.row;
        for chunk in self
            .as_rope()
            .chunks_in_range(range.to_offset(self))
            .chain(["\n"])
        {
            for (newline_ix, text) in chunk.split('\n').enumerate() {
                if newline_ix > 0 {
                    callback(row, &line);
                    row += 1;
                    line.clear();
                }
                line.push_str(text);
            }
        }
    }

    /// Iterates over every [`SyntaxLayer`] in the buffer.
    pub fn syntax_layers(&self) -> impl Iterator<Item = SyntaxLayer<'_>> + '_ {
        self.syntax_layers_for_range(0..self.len(), true)
    }

    pub fn syntax_layer_at<D: ToOffset>(&self, position: D) -> Option<SyntaxLayer<'_>> {
        let offset = position.to_offset(self);
        self.syntax_layers_for_range(offset..offset, false)
            .filter(|l| {
                if let Some(ranges) = l.included_sub_ranges {
                    ranges.iter().any(|range| {
                        let start = range.start.to_offset(self);
                        start <= offset && {
                            let end = range.end.to_offset(self);
                            offset < end
                        }
                    })
                } else {
                    l.node().start_byte() <= offset && l.node().end_byte() > offset
                }
            })
            .last()
    }

    pub fn syntax_layers_for_range<D: ToOffset>(
        &self,
        range: Range<D>,
        include_hidden: bool,
    ) -> impl Iterator<Item = SyntaxLayer<'_>> + '_ {
        self.syntax
            .layers_for_range(range, &self.text, include_hidden)
    }

    pub fn syntax_layers_languages(&self) -> impl Iterator<Item = &Arc<Language>> {
        self.syntax.languages(&self, true)
    }

    pub fn smallest_syntax_layer_containing<D: ToOffset>(
        &self,
        range: Range<D>,
    ) -> Option<SyntaxLayer<'_>> {
        let range = range.to_offset(self);
        self.syntax
            .layers_for_range(range, &self.text, false)
            .max_by(|a, b| {
                if a.depth != b.depth {
                    a.depth.cmp(&b.depth)
                } else if a.offset.0 != b.offset.0 {
                    a.offset.0.cmp(&b.offset.0)
                } else {
                    a.node().end_byte().cmp(&b.node().end_byte()).reverse()
                }
            })
    }

    /// Returns the [`ModelineSettings`].
    pub fn modeline(&self) -> Option<&Arc<ModelineSettings>> { self.modeline.as_ref() }

    pub(crate) fn resolved_settings(&self) -> Option<&Arc<LanguageSettings>> {
        self.resolved_settings.as_ref()
    }

    /// Returns the main [`Language`].
    pub fn language(&self) -> Option<&Arc<Language>> { self.language.as_ref() }

    /// Returns the [`Language`] at the given location.
    pub fn language_at<D: ToOffset>(&self, position: D) -> Option<&Arc<Language>> {
        self.syntax_layer_at(position)
            .map(|info| info.language)
            .or(self.language.as_ref())
    }

    /// Returns the settings for the language at the given location.
    pub fn settings_at<'a, D: ToOffset>(
        &'a self,
        position: D,
        cx: &'a App,
    ) -> Arc<LanguageSettings> {
        LanguageSettings::for_buffer_snapshot(self, Some(position.to_offset(self)), cx)
    }

    pub fn char_classifier_at<T: ToOffset>(&self, point: T) -> CharClassifier {
        CharClassifier::new(self.language_scope_at(point))
    }

    /// Returns the [`LanguageScope`] at the given location.
    pub fn language_scope_at<D: ToOffset>(&self, position: D) -> Option<LanguageScope> {
        let offset = position.to_offset(self);
        let mut scope = None;
        let mut smallest_range_and_depth: Option<(Range<usize>, usize)> = None;
        let text: &TextBufferSnapshot = self;

        // Use the layer that has the smallest node intersecting the given point.
        for layer in self
            .syntax
            .layers_for_range(offset..offset, &self.text, false)
        {
            if let Some(ranges) = layer.included_sub_ranges
                && !offset_in_sub_ranges(ranges, offset, text)
            {
                continue;
            }

            let mut cursor = layer.node().walk();

            let mut range = None;
            loop {
                let child_range = cursor.node().byte_range();
                if !child_range.contains(&offset) {
                    break;
                }

                range = Some(child_range);
                if cursor.goto_first_child_for_byte(offset).is_none() {
                    break;
                }
            }

            if let Some(range) = range
                && smallest_range_and_depth.as_ref().is_none_or(
                    |(smallest_range, smallest_range_depth)| {
                        if layer.depth > *smallest_range_depth {
                            true
                        } else if layer.depth == *smallest_range_depth {
                            range.len() < smallest_range.len()
                        } else {
                            false
                        }
                    },
                )
            {
                smallest_range_and_depth = Some((range, layer.depth));
                scope = Some(LanguageScope {
                    language: layer.language.clone(),
                    override_id: layer.override_id(offset, &self.text),
                });
            }
        }

        scope.or_else(|| {
            self.language.clone().map(|language| LanguageScope {
                language,
                override_id: None,
            })
        })
    }

    /// Returns a tuple of the range and character kind of the word
    /// surrounding the given position.
    pub fn surrounding_word<T: ToOffset>(
        &self,
        start: T,
        scope_context: Option<CharScopeContext>,
    ) -> (Range<usize>, Option<CharKind>) {
        let mut start = start.to_offset(self);
        let mut end = start;
        let mut next_chars = self.chars_at(start).take(128).peekable();
        let mut prev_chars = self.reversed_chars_at(start).take(128).peekable();

        let classifier = self.char_classifier_at(start).scope_context(scope_context);
        let word_kind = cmp::max(
            prev_chars.peek().copied().map(|c| classifier.kind(c)),
            next_chars.peek().copied().map(|c| classifier.kind(c)),
        );

        for ch in prev_chars {
            if Some(classifier.kind(ch)) == word_kind && ch != '\n' {
                start -= ch.len_utf8();
            } else {
                break;
            }
        }

        for ch in next_chars {
            if Some(classifier.kind(ch)) == word_kind && ch != '\n' {
                end += ch.len_utf8();
            } else {
                break;
            }
        }

        (start..end, word_kind)
    }

    /// Moves the TreeCursor to the smallest descendant or ancestor syntax node enclosing the given
    /// range. When `require_larger` is true, the node found must be larger than the query range.
    ///
    /// Returns true if a node was found, and false otherwise. In the `false` case the cursor will
    /// be moved to the root of the tree.
    fn goto_node_enclosing_range(
        cursor: &mut tree_sitter::TreeCursor,
        query_range: &Range<usize>,
        require_larger: bool,
    ) -> bool {
        let mut ascending = false;
        loop {
            let mut range = cursor.node().byte_range();
            if query_range.is_empty() {
                // When the query range is empty and the current node starts after it, move to the
                // previous sibling to find the node the containing node.
                if range.start > query_range.start {
                    cursor.goto_previous_sibling();
                    range = cursor.node().byte_range();
                }
            } else {
                // When the query range is non-empty and the current node ends exactly at the start,
                // move to the next sibling to find a node that extends beyond the start.
                if range.end == query_range.start {
                    cursor.goto_next_sibling();
                    range = cursor.node().byte_range();
                }
            }

            let encloses = range.contains_inclusive(query_range)
                && (!require_larger || range.len() > query_range.len());
            if !encloses {
                ascending = true;
                if !cursor.goto_parent() {
                    return false;
                }
                continue;
            } else if ascending {
                return true;
            }

            // Descend into the current node.
            if cursor
                .goto_first_child_for_byte(query_range.start)
                .is_none()
            {
                return true;
            }
        }
    }

    pub fn syntax_ancestor<'a, T: ToOffset>(
        &'a self,
        range: Range<T>,
    ) -> Option<tree_sitter::Node<'a>> {
        let range = range.start.to_offset(self)..range.end.to_offset(self);
        let mut result: Option<tree_sitter::Node<'a>> = None;
        for layer in self
            .syntax
            .layers_for_range(range.clone(), &self.text, true)
        {
            let mut cursor = layer.node().walk();

            // Find the node that both contains the range and is larger than it.
            if !Self::goto_node_enclosing_range(&mut cursor, &range, true) {
                continue;
            }

            let left_node = cursor.node();
            let mut layer_result = left_node;

            // For an empty range, try to find another node immediately to the right of the range.
            if left_node.end_byte() == range.start {
                let mut right_node = None;
                while !cursor.goto_next_sibling() {
                    if !cursor.goto_parent() {
                        break;
                    }
                }

                while cursor.node().start_byte() == range.start {
                    right_node = Some(cursor.node());
                    if !cursor.goto_first_child() {
                        break;
                    }
                }

                // If there is a candidate node on both sides of the (empty) range, then
                // decide between the two by favoring a named node over an anonymous token.
                // If both nodes are the same in that regard, favor the right one.
                if let Some(right_node) = right_node
                    && (right_node.is_named() || !left_node.is_named())
                {
                    layer_result = right_node;
                }
            }

            if let Some(previous_result) = &result
                && previous_result.byte_range().len() < layer_result.byte_range().len()
            {
                continue;
            }
            result = Some(layer_result);
        }

        result
    }

    /// Find the previous sibling syntax node at the given range.
    ///
    /// This function locates the syntax node that precedes the node containing
    /// the given range. It searches hierarchically by:
    /// 1. Finding the node that contains the given range
    /// 2. Looking for the previous sibling at the same tree level
    /// 3. If no sibling is found, moving up to parent levels and searching for siblings
    ///
    /// Returns `None` if there is no previous sibling at any ancestor level.
    pub fn syntax_prev_sibling<'a, T: ToOffset>(
        &'a self,
        range: Range<T>,
    ) -> Option<tree_sitter::Node<'a>> {
        let range = range.start.to_offset(self)..range.end.to_offset(self);
        let mut result: Option<tree_sitter::Node<'a>> = None;

        for layer in self
            .syntax
            .layers_for_range(range.clone(), &self.text, true)
        {
            let mut cursor = layer.node().walk();

            // Find the node that contains the range
            if !Self::goto_node_enclosing_range(&mut cursor, &range, false) {
                continue;
            }

            // Look for the previous sibling, moving up ancestor levels if needed
            loop {
                if cursor.goto_previous_sibling() {
                    let layer_result = cursor.node();

                    if let Some(previous_result) = &result {
                        if previous_result.byte_range().end < layer_result.byte_range().end {
                            continue;
                        }
                    }
                    result = Some(layer_result);
                    break;
                }

                // No sibling found at this level, try moving up to parent
                if !cursor.goto_parent() {
                    break;
                }
            }
        }

        result
    }

    /// Find the next sibling syntax node at the given range.
    ///
    /// This function locates the syntax node that follows the node containing
    /// the given range. It searches hierarchically by:
    /// 1. Finding the node that contains the given range
    /// 2. Looking for the next sibling at the same tree level
    /// 3. If no sibling is found, moving up to parent levels and searching for siblings
    ///
    /// Returns `None` if there is no next sibling at any ancestor level.
    pub fn syntax_next_sibling<'a, T: ToOffset>(
        &'a self,
        range: Range<T>,
    ) -> Option<tree_sitter::Node<'a>> {
        let range = range.start.to_offset(self)..range.end.to_offset(self);
        let mut result: Option<tree_sitter::Node<'a>> = None;

        for layer in self
            .syntax
            .layers_for_range(range.clone(), &self.text, true)
        {
            let mut cursor = layer.node().walk();

            // Find the node that contains the range
            if !Self::goto_node_enclosing_range(&mut cursor, &range, false) {
                continue;
            }

            // Look for the next sibling, moving up ancestor levels if needed
            loop {
                if cursor.goto_next_sibling() {
                    let layer_result = cursor.node();

                    if let Some(previous_result) = &result {
                        if previous_result.byte_range().start > layer_result.byte_range().start {
                            continue;
                        }
                    }
                    result = Some(layer_result);
                    break;
                }

                // No sibling found at this level, try moving up to parent
                if !cursor.goto_parent() {
                    break;
                }
            }
        }

        result
    }

    /// Returns the root syntax node within the given row
    pub fn syntax_root_ancestor(&self, position: Anchor) -> Option<tree_sitter::Node<'_>> {
        let start_offset = position.to_offset(self);

        let row = self.summary_for_anchor::<text::PointUtf16>(&position).row as usize;

        let layer = self
            .syntax
            .layers_for_range(start_offset..start_offset, &self.text, true)
            .next()?;

        let mut cursor = layer.node().walk();

        // Descend to the first leaf that touches the start of the range.
        while cursor.goto_first_child_for_byte(start_offset).is_some() {
            if cursor.node().end_byte() == start_offset {
                cursor.goto_next_sibling();
            }
        }

        // Ascend to the root node within the same row.
        while cursor.goto_parent() {
            if cursor.node().start_position().row != row {
                break;
            }
        }

        Some(cursor.node())
    }

    /// Returns the outline for the buffer.
    ///
    /// This method allows passing an optional [`SyntaxTheme`] to
    /// syntax-highlight the returned symbols.
    pub fn outline(&self, theme: Option<&SyntaxTheme>) -> Outline<Anchor> {
        Outline::new(self.outline_items_containing(0..self.len(), true, theme))
    }

    /// Returns all the symbols that contain the given position.
    ///
    /// This method allows passing an optional [`SyntaxTheme`] to
    /// syntax-highlight the returned symbols.
    pub fn symbols_containing<T: ToOffset>(
        &self,
        position: T,
        theme: Option<&SyntaxTheme>,
    ) -> Vec<OutlineItem<Anchor>> {
        let position = position.to_offset(self);
        let start = self.clip_offset(position.saturating_sub(1), Bias::Left);
        let end = self.clip_offset(position + 1, Bias::Right);
        let mut items = self.outline_items_containing(start..end, false, theme);
        let mut prev_depth = None;
        items.retain(|item| {
            let result = prev_depth.is_none_or(|prev_depth| item.depth > prev_depth);
            prev_depth = Some(item.depth);
            result
        });
        items
    }

    pub fn outline_ranges_containing<T: ToOffset>(
        &self,
        range: Range<T>,
    ) -> impl Iterator<Item = Range<Point>> + '_ {
        let range = range.to_offset(self);
        let mut matches = self.syntax.matches(range.clone(), &self.text, |grammar| {
            grammar.outline_config.as_ref().map(|c| &c.query)
        });
        let configs = matches
            .grammars()
            .iter()
            .map(|g| g.outline_config.as_ref().unwrap())
            .collect::<Vec<_>>();

        std::iter::from_fn(move || {
            while let Some(mat) = matches.peek() {
                let config = &configs[mat.grammar_index];
                let containing_item_node = {
                    let item_node = mat.captures.iter().find_map(|cap| {
                        if cap.index == config.item_capture_ix {
                            Some(cap.node)
                        } else {
                            None
                        }
                    });
                    match item_node {
                        Some(node) => {
                            let item_byte_range = node.byte_range();
                            if item_byte_range.end < range.start
                                || item_byte_range.start > range.end
                            {
                                None
                            } else {
                                Some(node)
                            }
                        }
                        None => None,
                    }
                };

                let range = containing_item_node.map(|item_node| {
                    Point::from_ts_point(item_node.start_position())
                        ..Point::from_ts_point(item_node.end_position())
                });
                matches.advance();
                if range.is_some() {
                    return range;
                }
            }
            None
        })
    }

    pub fn outline_range_containing<T: ToOffset>(&self, range: Range<T>) -> Option<Range<Point>> {
        self.outline_ranges_containing(range).next()
    }

    pub fn outline_items_containing<T: ToOffset>(
        &self,
        range: Range<T>,
        include_extra_context: bool,
        theme: Option<&SyntaxTheme>,
    ) -> Vec<OutlineItem<Anchor>> {
        self.outline_items_containing_internal(
            range,
            include_extra_context,
            theme,
            |this, range| this.anchor_after(range.start)..this.anchor_before(range.end),
        )
    }

    pub fn outline_items_as_points_containing<T: ToOffset>(
        &self,
        range: Range<T>,
        include_extra_context: bool,
        theme: Option<&SyntaxTheme>,
    ) -> Vec<OutlineItem<Point>> {
        self.outline_items_containing_internal(range, include_extra_context, theme, |_, range| {
            range
        })
    }

    pub fn outline_items_as_offsets_containing<T: ToOffset>(
        &self,
        range: Range<T>,
        include_extra_context: bool,
        theme: Option<&SyntaxTheme>,
    ) -> Vec<OutlineItem<usize>> {
        self.outline_items_containing_internal(
            range,
            include_extra_context,
            theme,
            |buffer, range| range.to_offset(buffer),
        )
    }

    fn outline_items_containing_internal<T: ToOffset, U>(
        &self,
        range: Range<T>,
        include_extra_context: bool,
        theme: Option<&SyntaxTheme>,
        range_callback: fn(&Self, Range<Point>) -> Range<U>,
    ) -> Vec<OutlineItem<U>> {
        let range = range.to_offset(self);
        let mut matches = self.syntax.matches(range.clone(), &self.text, |grammar| {
            grammar.outline_config.as_ref().map(|c| &c.query)
        });

        let mut items = Vec::new();
        let mut annotation_row_ranges: Vec<Range<u32>> = Vec::new();
        while let Some(mat) = matches.peek() {
            let config = matches.grammars()[mat.grammar_index]
                .outline_config
                .as_ref()
                .unwrap();
            if let Some(item) =
                self.next_outline_item(config, &mat, &range, include_extra_context, theme)
            {
                items.push(item);
            } else if let Some(capture) = mat
                .captures
                .iter()
                .find(|capture| Some(capture.index) == config.annotation_capture_ix)
            {
                let capture_range = capture.node.start_position()..capture.node.end_position();
                let mut capture_row_range =
                    capture_range.start.row as u32..capture_range.end.row as u32;
                if capture_range.end.row > capture_range.start.row && capture_range.end.column == 0
                {
                    capture_row_range.end -= 1;
                }
                if let Some(last_row_range) = annotation_row_ranges.last_mut() {
                    if last_row_range.end >= capture_row_range.start.saturating_sub(1) {
                        last_row_range.end = capture_row_range.end;
                    } else {
                        annotation_row_ranges.push(capture_row_range);
                    }
                } else {
                    annotation_row_ranges.push(capture_row_range);
                }
            }
            matches.advance();
        }

        items.sort_by_key(|item| (item.range.start, Reverse(item.range.end)));

        // Assign depths based on containment relationships and convert to anchors.
        let mut item_ends_stack = Vec::<Point>::new();
        let mut anchor_items = Vec::new();
        let mut annotation_row_ranges = annotation_row_ranges.into_iter().peekable();
        for item in items {
            while let Some(last_end) = item_ends_stack.last().copied() {
                if last_end < item.range.end {
                    item_ends_stack.pop();
                } else {
                    break;
                }
            }

            let mut annotation_row_range = None;
            while let Some(next_annotation_row_range) = annotation_row_ranges.peek() {
                let row_preceding_item = item.range.start.row.saturating_sub(1);
                if next_annotation_row_range.end < row_preceding_item {
                    annotation_row_ranges.next();
                } else {
                    if next_annotation_row_range.end == row_preceding_item {
                        annotation_row_range = Some(next_annotation_row_range.clone());
                        annotation_row_ranges.next();
                    }
                    break;
                }
            }

            anchor_items.push(OutlineItem {
                depth: item_ends_stack.len(),
                range: range_callback(self, item.range.clone()),
                selection_range: range_callback(self, item.selection_range.clone()),
                source_range_for_text: range_callback(self, item.source_range_for_text.clone()),
                text: item.text,
                highlight_ranges: item.highlight_ranges,
                name_ranges: item.name_ranges,
                body_range: item.body_range.map(|r| range_callback(self, r)),
                annotation_range: annotation_row_range.map(|annotation_range| {
                    let point_range = Point::new(annotation_range.start, 0)
                        ..Point::new(annotation_range.end, self.line_len(annotation_range.end));
                    range_callback(self, point_range)
                }),
            });
            item_ends_stack.push(item.range.end);
        }

        anchor_items
    }

    fn next_outline_item(
        &self,
        config: &OutlineConfig,
        mat: &SyntaxMapMatch,
        range: &Range<usize>,
        include_extra_context: bool,
        theme: Option<&SyntaxTheme>,
    ) -> Option<OutlineItem<Point>> {
        let item_node = mat.captures.iter().find_map(|cap| {
            if cap.index == config.item_capture_ix {
                Some(cap.node)
            } else {
                None
            }
        })?;

        let item_byte_range = item_node.byte_range();
        if item_byte_range.end < range.start || item_byte_range.start > range.end {
            return None;
        }
        let item_point_range = Point::from_ts_point(item_node.start_position())
            ..Point::from_ts_point(item_node.end_position());

        let mut open_point = None;
        let mut close_point = None;

        let mut buffer_ranges = Vec::new();
        let mut add_to_buffer_ranges = |node: tree_sitter::Node, node_is_name| {
            let mut range = node.start_byte()..node.end_byte();
            let start = node.start_position();
            if node.end_position().row > start.row {
                range.end = range.start + self.line_len(start.row as u32) as usize - start.column;
            }

            if !range.is_empty() {
                buffer_ranges.push((range, node_is_name));
            }
        };

        for capture in mat.captures {
            if capture.index == config.name_capture_ix {
                add_to_buffer_ranges(capture.node, true);
            } else if Some(capture.index) == config.context_capture_ix
                || (Some(capture.index) == config.extra_context_capture_ix && include_extra_context)
            {
                add_to_buffer_ranges(capture.node, false);
            } else {
                if Some(capture.index) == config.open_capture_ix {
                    open_point = Some(Point::from_ts_point(capture.node.end_position()));
                } else if Some(capture.index) == config.close_capture_ix {
                    close_point = Some(Point::from_ts_point(capture.node.start_position()));
                }
            }
        }

        if buffer_ranges.is_empty() {
            return None;
        }
        let source_range_for_text =
            buffer_ranges.first().unwrap().0.start..buffer_ranges.last().unwrap().0.end;
        let selection_range = buffer_ranges
            .iter()
            .filter(|(_, node_is_name)| *node_is_name)
            .map(|(buffer_range, _)| buffer_range.clone())
            .reduce(|mut combined_range, next_range| {
                combined_range.end = next_range.end;
                combined_range
            })?;

        let mut text = String::new();
        let mut highlight_ranges = Vec::new();
        let mut name_ranges = Vec::new();
        let mut chunks = self.chunks(
            source_range_for_text.clone(),
            LanguageAwareStyling {
                tree_sitter: true,
                diagnostics: true,
            },
        );
        let mut last_buffer_range_end = 0;
        for (buffer_range, is_name) in buffer_ranges {
            let space_added = !text.is_empty() && buffer_range.start > last_buffer_range_end;
            if space_added {
                text.push(' ');
            }
            let before_append_len = text.len();
            let mut offset = buffer_range.start;
            chunks.seek(buffer_range.clone());
            for mut chunk in chunks.by_ref() {
                if chunk.text.len() > buffer_range.end - offset {
                    chunk.text = &chunk.text[0..(buffer_range.end - offset)];
                    offset = buffer_range.end;
                } else {
                    offset += chunk.text.len();
                }
                let style = chunk
                    .syntax_highlight_id
                    .zip(theme)
                    .and_then(|(highlight, theme)| {
                        theme.highlight(usize::from(highlight)).cloned()
                    });

                if let Some(style) = style {
                    let start = text.len();
                    let end = start + chunk.text.len();
                    highlight_ranges.push((start..end, style));
                }
                text.push_str(chunk.text);
                if offset >= buffer_range.end {
                    break;
                }
            }
            if is_name {
                let after_append_len = text.len();
                let start = if space_added && !name_ranges.is_empty() {
                    before_append_len - 1
                } else {
                    before_append_len
                };
                name_ranges.push(start..after_append_len);
            }
            last_buffer_range_end = buffer_range.end;
        }

        Some(OutlineItem {
            depth: 0, // We'll calculate the depth later
            range: item_point_range,
            selection_range: selection_range.to_point(self),
            source_range_for_text: source_range_for_text.to_point(self),
            text: text.into(),
            highlight_ranges,
            name_ranges,
            body_range: open_point.zip(close_point).map(|(start, end)| start..end),
            annotation_range: None,
        })
    }

    pub fn function_body_fold_ranges<T: ToOffset>(
        &self,
        within: Range<T>,
    ) -> impl Iterator<Item = Range<usize>> + '_ {
        self.text_object_ranges(within, TreeSitterOptions::default())
            .filter_map(|(range, obj)| (obj == TextObject::InsideFunction).then_some(range))
    }

    /// For each grammar in the language, runs the provided
    /// [`tree_sitter::Query`] against the given range.
    pub fn matches(
        &self,
        range: Range<usize>,
        query: fn(&Grammar) -> Option<&tree_sitter::Query>,
    ) -> SyntaxMapMatches<'_> {
        self.syntax.matches(range, self, query)
    }

    pub fn debug_variables_query<T: ToOffset>(
        &self,
        range: Range<T>,
    ) -> impl Iterator<Item = (Range<usize>, DebuggerTextObject)> + '_ {
        let range = range.start.to_previous_offset(self)..range.end.to_next_offset(self);

        let mut matches = self.syntax.matches_with_options(
            range.clone(),
            &self.text,
            TreeSitterOptions::default(),
            |grammar| grammar.debug_variables_config.as_ref().map(|c| &c.query),
        );

        let configs = matches
            .grammars()
            .iter()
            .map(|grammar| grammar.debug_variables_config.as_ref())
            .collect::<Vec<_>>();

        let mut captures = Vec::<(Range<usize>, DebuggerTextObject)>::new();

        iter::from_fn(move || {
            loop {
                while let Some(capture) = captures.pop() {
                    if capture.0.overlaps(&range) {
                        return Some(capture);
                    }
                }

                let mat = matches.peek()?;

                let Some(config) = configs[mat.grammar_index].as_ref() else {
                    matches.advance();
                    continue;
                };

                for capture in mat.captures {
                    let Some(ix) = config
                        .objects_by_capture_ix
                        .binary_search_by_key(&capture.index, |e| e.0)
                        .ok()
                    else {
                        continue;
                    };
                    let text_object = config.objects_by_capture_ix[ix].1;
                    let byte_range = capture.node.byte_range();

                    let mut found = false;
                    for (range, existing) in captures.iter_mut() {
                        if existing == &text_object {
                            range.start = range.start.min(byte_range.start);
                            range.end = range.end.max(byte_range.end);
                            found = true;
                            break;
                        }
                    }

                    if !found {
                        captures.push((byte_range, text_object));
                    }
                }

                matches.advance();
            }
        })
    }

    pub fn text_object_ranges<T: ToOffset>(
        &self,
        range: Range<T>,
        options: TreeSitterOptions,
    ) -> impl Iterator<Item = (Range<usize>, TextObject)> + '_ {
        let range =
            range.start.to_previous_offset(self)..self.len().min(range.end.to_next_offset(self));

        let mut matches =
            self.syntax
                .matches_with_options(range.clone(), &self.text, options, |grammar| {
                    grammar.text_object_config.as_ref().map(|c| &c.query)
                });

        let configs = matches
            .grammars()
            .iter()
            .map(|grammar| grammar.text_object_config.as_ref())
            .collect::<Vec<_>>();

        let mut captures = Vec::<(Range<usize>, TextObject)>::new();

        iter::from_fn(move || {
            loop {
                while let Some(capture) = captures.pop() {
                    if capture.0.overlaps(&range) {
                        return Some(capture);
                    }
                }

                let mat = matches.peek()?;

                let Some(config) = configs[mat.grammar_index].as_ref() else {
                    matches.advance();
                    continue;
                };

                for capture in mat.captures {
                    let Some(ix) = config
                        .text_objects_by_capture_ix
                        .binary_search_by_key(&capture.index, |e| e.0)
                        .ok()
                    else {
                        continue;
                    };
                    let text_object = config.text_objects_by_capture_ix[ix].1;
                    let byte_range = capture.node.byte_range();

                    let mut found = false;
                    for (range, existing) in captures.iter_mut() {
                        if existing == &text_object {
                            range.start = range.start.min(byte_range.start);
                            range.end = range.end.max(byte_range.end);
                            found = true;
                            break;
                        }
                    }

                    if !found {
                        captures.push((byte_range, text_object));
                    }
                }

                matches.advance();
            }
        })
    }

    /// Returns anchor ranges for any matches of the redaction query.
    /// The buffer can be associated with multiple languages, and the redaction query associated with each
    /// will be run on the relevant section of the buffer.
    pub fn redacted_ranges<T: ToOffset>(
        &self,
        range: Range<T>,
    ) -> impl Iterator<Item = Range<usize>> + '_ {
        let offset_range = range.start.to_offset(self)..range.end.to_offset(self);
        let mut syntax_matches = self.syntax.matches(offset_range, self, |grammar| {
            grammar
                .redactions_config
                .as_ref()
                .map(|config| &config.query)
        });

        let configs = syntax_matches
            .grammars()
            .iter()
            .map(|grammar| grammar.redactions_config.as_ref())
            .collect::<Vec<_>>();

        iter::from_fn(move || {
            let redacted_range = syntax_matches
                .peek()
                .and_then(|mat| {
                    configs[mat.grammar_index].and_then(|config| {
                        mat.captures
                            .iter()
                            .find(|capture| capture.index == config.redaction_capture_ix)
                    })
                })
                .map(|mat| mat.node.byte_range());
            syntax_matches.advance();
            redacted_range
        })
    }

    pub fn injections_intersecting_range<T: ToOffset>(
        &self,
        range: Range<T>,
    ) -> impl Iterator<Item = (Range<usize>, &Arc<Language>)> + '_ {
        let offset_range = range.start.to_offset(self)..range.end.to_offset(self);

        let mut syntax_matches = self.syntax.matches(offset_range, self, |grammar| {
            grammar
                .injection_config
                .as_ref()
                .map(|config| &config.query)
        });

        let configs = syntax_matches
            .grammars()
            .iter()
            .map(|grammar| grammar.injection_config.as_ref())
            .collect::<Vec<_>>();

        iter::from_fn(move || {
            let ranges = syntax_matches.peek().and_then(|mat| {
                let config = &configs[mat.grammar_index]?;
                let content_capture_range = mat.captures.iter().find_map(|capture| {
                    if capture.index == config.content_capture_ix {
                        Some(capture.node.byte_range())
                    } else {
                        None
                    }
                })?;
                let language = self.language_at(content_capture_range.start)?;
                Some((content_capture_range, language))
            });
            syntax_matches.advance();
            ranges
        })
    }

    pub fn runnable_ranges(
        &self,
        offset_range: Range<usize>,
    ) -> impl Iterator<Item = RunnableRange> + '_ {
        runnable::runnable_ranges(self, offset_range)
    }

    /// Returns selections for remote peers intersecting the given range.
    #[allow(clippy::type_complexity)]
    pub fn selections_in_range(
        &self,
        range: Range<Anchor>,
        include_local: bool,
    ) -> impl Iterator<
        Item = (
            ReplicaId,
            bool,
            CursorShape,
            impl Iterator<Item = &Selection<Anchor>> + '_,
        ),
    > + '_ {
        self.remote_selections
            .iter()
            .filter(move |(replica_id, set)| {
                (include_local || **replica_id != self.text.replica_id())
                    && !set.selections.is_empty()
            })
            .map(move |(replica_id, set)| {
                let start_ix = match set.selections.binary_search_by(|probe| {
                    probe.end.cmp(&range.start, self).then(Ordering::Greater)
                }) {
                    Ok(ix) | Err(ix) => ix,
                };
                let end_ix = match set.selections.binary_search_by(|probe| {
                    probe.start.cmp(&range.end, self).then(Ordering::Less)
                }) {
                    Ok(ix) | Err(ix) => ix,
                };

                (
                    *replica_id,
                    set.line_mode,
                    set.cursor_shape,
                    set.selections[start_ix..end_ix].iter(),
                )
            })
    }

    /// Returns if the buffer contains any diagnostics.
    pub fn has_diagnostics(&self) -> bool { !self.diagnostics.is_empty() }

    /// Returns all the diagnostics intersecting the given range.
    pub fn diagnostics_in_range<'a, T, O>(
        &'a self,
        search_range: Range<T>,
        reversed: bool,
    ) -> impl 'a + Iterator<Item = DiagnosticEntryRef<'a, O>>
    where
        T: 'a + Clone + ToOffset,
        O: 'a + FromAnchor,
    {
        self.diagnostic_entries_in_range(search_range, reversed)
            .map(|entry| entry.resolve(self))
    }

    /// Returns the stored entries that intersect the given range, with their ranges
    /// and related information left in the buffer's own coordinates.
    pub fn diagnostic_entries_in_range<'a, T>(
        &'a self,
        search_range: Range<T>,
        reversed: bool,
    ) -> impl 'a + Iterator<Item = &'a DiagnosticEntry<Anchor>>
    where
        T: 'a + Clone + ToOffset,
    {
        self.diagnostic_entries_in_range_with_server_id(search_range, reversed)
            .map(|(_, entry)| entry)
    }

    /// Returns the stored entries that intersect the given range along with the
    /// language server that produced each diagnostic.
    pub fn diagnostic_entries_in_range_with_server_id<'a, T>(
        &'a self,
        search_range: Range<T>,
        reversed: bool,
    ) -> impl 'a + Iterator<Item = (LanguageServerId, &'a DiagnosticEntry<Anchor>)>
    where
        T: 'a + Clone + ToOffset,
    {
        let mut iterators: Vec<_> = self
            .diagnostics
            .iter()
            .map(|(server_id, collection)| {
                (
                    *server_id,
                    collection
                        .entries_in_range::<T>(search_range.clone(), self, true, reversed)
                        .peekable(),
                )
            })
            .collect();

        std::iter::from_fn(move || {
            let (next_ix, _) = iterators
                .iter_mut()
                .enumerate()
                .flat_map(|(ix, (_, iter))| Some((ix, iter.peek()?)))
                .min_by(|(_, a), (_, b)| {
                    let cmp = a
                        .range
                        .start
                        .cmp(&b.range.start, self)
                        // when range is equal, sort by diagnostic severity
                        .then(a.diagnostic.severity.cmp(&b.diagnostic.severity))
                        // and stabilize order with group_id
                        .then(a.diagnostic.group_id.cmp(&b.diagnostic.group_id));
                    if reversed { cmp.reverse() } else { cmp }
                })?;
            let (server_id, iterator) = iterators.get_mut(next_ix)?;
            let server_id = *server_id;
            iterator.next().map(|entry| (server_id, entry))
        })
    }

    /// Returns all the diagnostic groups associated with the given
    /// language server ID. If no language server ID is provided,
    /// all diagnostics groups are returned.
    pub fn diagnostic_groups(
        &self,
        language_server_id: Option<LanguageServerId>,
    ) -> Vec<(LanguageServerId, DiagnosticGroup<'_, Anchor>)> {
        let mut groups = Vec::new();

        if let Some(language_server_id) = language_server_id {
            if let Some(set) = self.diagnostics.get(&language_server_id) {
                set.groups(language_server_id, &mut groups, self);
            }
        } else {
            for (language_server_id, diagnostics) in self.diagnostics.iter() {
                diagnostics.groups(*language_server_id, &mut groups, self);
            }
        }

        groups.sort_by(|(id_a, group_a), (id_b, group_b)| {
            let a_start = &group_a.entries[group_a.primary_ix].range.start;
            let b_start = &group_b.entries[group_b.primary_ix].range.start;
            a_start.cmp(b_start, self).then_with(|| id_a.cmp(id_b))
        });

        groups
    }

    /// Returns an iterator over the diagnostics for the given group.
    pub fn diagnostic_group<O>(
        &self,
        group_id: usize,
    ) -> impl Iterator<Item = DiagnosticEntryRef<'_, O>> + use<'_, O>
    where
        O: FromAnchor + 'static,
    {
        self.diagnostics
            .iter()
            .flat_map(move |(_, set)| set.group(group_id, self))
    }

    /// An integer version number that accounts for all updates besides
    /// the buffer's text itself (which is versioned via a version vector).
    pub fn non_text_state_update_count(&self) -> usize { self.non_text_state_update_count }

    /// An integer version that changes when the buffer's syntax changes.
    pub fn syntax_update_count(&self) -> usize { self.syntax.update_count() }

    /// Returns a snapshot of underlying file.
    pub fn file(&self) -> Option<&Arc<dyn File>> { self.file.as_ref() }

    pub fn resolve_file_path(&self, include_root: bool, cx: &App) -> Option<String> {
        if let Some(file) = self.file() {
            if file.path().file_name().is_none() || include_root {
                Some(file.full_path(cx).to_string_lossy().into_owned())
            } else {
                Some(file.path().display(file.path_style(cx)).to_string())
            }
        } else {
            None
        }
    }

    pub fn words_in_range(&self, query: WordsQuery) -> BTreeMap<String, Range<Anchor>> {
        let query_str = query.fuzzy_contents;
        if query_str.is_some_and(|query| query.is_empty()) {
            return BTreeMap::default();
        }

        let classifier = CharClassifier::new(self.language.clone().map(|language| LanguageScope {
            language,
            override_id: None,
        }));

        let mut query_ix = 0;
        let query_chars = query_str.map(|query| query.chars().collect::<Vec<_>>());
        let query_len = query_chars.as_ref().map_or(0, |query| query.len());

        let mut words = BTreeMap::default();
        let mut current_word_start_ix = None;
        let mut chunk_ix = query.range.start;
        for chunk in self.chunks(
            query.range,
            LanguageAwareStyling {
                tree_sitter: false,
                diagnostics: false,
            },
        ) {
            for (i, c) in chunk.text.char_indices() {
                let ix = chunk_ix + i;
                if classifier.is_word(c) {
                    if current_word_start_ix.is_none() {
                        current_word_start_ix = Some(ix);
                    }

                    if let Some(query_chars) = &query_chars
                        && query_ix < query_len
                        && c.to_lowercase().eq(query_chars[query_ix].to_lowercase())
                    {
                        query_ix += 1;
                    }
                    continue;
                } else if let Some(word_start) = current_word_start_ix.take()
                    && query_ix == query_len
                {
                    let word_range = self.anchor_before(word_start)..self.anchor_after(ix);
                    let mut word_text = self.text_for_range(word_start..ix).peekable();
                    let first_char = word_text
                        .peek()
                        .and_then(|first_chunk| first_chunk.chars().next());
                    // Skip empty and "words" starting with digits as a heuristic to reduce useless completions
                    if !query.skip_digits
                        || first_char.is_none_or(|first_char| !first_char.is_digit(10))
                    {
                        words.insert(word_text.collect(), word_range);
                    }
                }
                query_ix = 0;
            }
            chunk_ix += chunk.text.len();
        }

        words
    }
}

impl Clone for BufferSnapshot {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            syntax: self.syntax.clone(),
            file: self.file.clone(),
            remote_selections: self.remote_selections.clone(),
            diagnostics: self.diagnostics.clone(),
            language: self.language.clone(),
            tree_sitter_data: self.tree_sitter_data.clone(),
            non_text_state_update_count: self.non_text_state_update_count,
            capability: self.capability,
            modeline: self.modeline.clone(),
            resolved_settings: self.resolved_settings.clone(),
        }
    }
}

impl Deref for BufferSnapshot {
    type Target = text::BufferSnapshot;

    fn deref(&self) -> &Self::Target { &self.text }
}
