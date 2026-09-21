use std::ops::DerefMut;
use std::sync::{Arc, LazyLock};

use language_core::Grammar;
use parking_lot::Mutex;
use text::Rope;
use tree_sitter::{self, Parser, QueryCursor, Tree, WasmStore, wasmtime};

use crate::syntax_map::QueryCursorHandle;

static QUERY_CURSORS: Mutex<Vec<QueryCursor>> = Mutex::new(vec![]);
static PARSERS: Mutex<Vec<Parser>> = Mutex::new(vec![]);

static WASM_ENGINE: LazyLock<wasmtime::Engine> = LazyLock::new(|| {
    wasmtime::Engine::new(&wasmtime::Config::new()).expect("Failed to create Wasmtime engine")
});

#[a_tracing::instrument(skip_all)]
pub fn with_parser<F, R>(func: F) -> R
where
    F: FnOnce(&mut Parser) -> R,
{
    let mut parser = PARSERS.lock().pop().unwrap_or_else(|| {
        let mut parser = Parser::new();
        parser
            .set_wasm_store(WasmStore::new(&WASM_ENGINE).unwrap())
            .unwrap();
        parser
    });
    // Tree-sitter auto-resets the parser at the end of a successful parse,
    // but the cancellation paths (progress callback returning `Break`,
    // cancelled balancing) leave outstanding state on the parser. The next
    // call to `parse_with_options` would then *resume* that cancelled parse
    // instead of starting fresh.
    parser.reset();
    parser.set_included_ranges(&[]).unwrap();
    let result = func(&mut parser);
    PARSERS.lock().push(parser);
    result
}

pub fn with_query_cursor<F, R>(func: F) -> R
where
    F: FnOnce(&mut QueryCursor) -> R,
{
    let mut cursor = QueryCursorHandle::new();
    func(cursor.deref_mut())
}

pub(crate) fn parse_text(grammar: &Grammar, text: &Rope, old_tree: Option<Tree>) -> Tree {
    with_parser(|parser| {
        parser
            .set_language(&grammar.ts_language)
            .expect("incompatible grammar");
        let mut chunks = text.chunks_in_range(0..text.len());
        parser
            .parse_with_options(
                &mut move |offset, _| {
                    chunks.seek(offset);
                    chunks.next().unwrap_or("").as_bytes()
                },
                old_tree.as_ref(),
                None,
            )
            .unwrap()
    })
}
