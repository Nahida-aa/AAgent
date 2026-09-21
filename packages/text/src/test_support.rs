#![cfg(any(test, feature = "test-support"))]

use super::*;

impl Buffer {
    #[track_caller]
    pub fn edit_via_marked_text(&mut self, marked_string: &str) { /* 原样 */
    }

    #[track_caller]
    pub fn edits_for_marked_text(&self, marked_string: &str) -> Vec<(Range<usize>, String)> { /* 原样 */
    }

    pub fn check_invariants(&self) { /* 原样 */
    }

    pub fn random_byte_range(&self, start_offset: usize, rng: &mut impl rand::Rng) -> Range<usize> { /* 原样 */
    }

    pub fn get_random_edits<T>(/* ... */) -> Vec<(Range<usize>, Arc<str>)>
    where
        T: rand::Rng,
    { /* 原样 */
    }

    pub fn randomly_edit<T>(/* ... */) -> (Vec<(Range<usize>, Arc<str>)>, Operation)
    where
        T: rand::Rng,
    { /* 原样 */
    }

    pub fn randomly_undo_redo(&mut self, rng: &mut impl rand::Rng) -> Vec<Operation> { /* 原样 */
    }
}
