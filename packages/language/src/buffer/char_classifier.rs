use super::*;

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Debug)]
pub enum CharKind {
    Whitespace,
    Punctuation,
    Word,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum CharScopeContext {
    Completion,
    LinkedEdit,
}

#[derive(Clone, Default, Debug)]
pub struct CharClassifier {
    scope: Option<LanguageScope>,
    scope_context: Option<CharScopeContext>,
    ignore_punctuation: bool,
}

impl CharClassifier {
    pub fn new(scope: Option<LanguageScope>) -> Self {
        Self {
            scope,
            scope_context: None,
            ignore_punctuation: false,
        }
    }

    pub fn scope_context(self, scope_context: Option<CharScopeContext>) -> Self {
        Self {
            scope_context,
            ..self
        }
    }

    pub fn ignore_punctuation(self, ignore_punctuation: bool) -> Self {
        Self {
            ignore_punctuation,
            ..self
        }
    }

    pub fn is_whitespace(&self, c: char) -> bool { self.kind(c) == CharKind::Whitespace }

    pub fn is_word(&self, c: char) -> bool { self.kind(c) == CharKind::Word }

    pub fn is_punctuation(&self, c: char) -> bool { self.kind(c) == CharKind::Punctuation }

    pub fn kind_with(&self, c: char, ignore_punctuation: bool) -> CharKind {
        if c.is_alphanumeric() || c == '_' {
            return CharKind::Word;
        }

        if let Some(scope) = &self.scope {
            let characters = match self.scope_context {
                Some(CharScopeContext::Completion) => scope.completion_query_characters(),
                Some(CharScopeContext::LinkedEdit) => scope.linked_edit_characters(),
                None => scope.word_characters(),
            };
            if let Some(characters) = characters
                && characters.contains(&c)
            {
                return CharKind::Word;
            }
        }

        if c.is_whitespace() {
            return CharKind::Whitespace;
        }

        if ignore_punctuation {
            CharKind::Word
        } else {
            CharKind::Punctuation
        }
    }

    pub fn kind(&self, c: char) -> CharKind { self.kind_with(c, self.ignore_punctuation) }
}
