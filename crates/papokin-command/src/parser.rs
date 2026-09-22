use std::borrow::Cow;

use crate::{
    errors::error_types::{AnyCommandErrorType, CommandErrorType},
    string_reader::StringReader,
};

/// `CommandSyntaxError` 的延迟版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedCommandSyntaxError {
    pub error_type: &'static dyn AnyCommandErrorType,
    pub java_translation_key: &'static str,
    pub arguments: Vec<Cow<'static, str>>,
}

#[derive(Debug, Default)]
pub struct ParserErrors {
    pub cursor: usize,
    pub command_error: Option<DelayedCommandSyntaxError>,
    pub suggestions: Vec<Cow<'static, str>>,
}

/// 一个 trait，使得专注于以下内容的解析器能够
/// 用于追踪错误的情况无需追踪
/// 建议列表，反之亦然。
impl ParserErrors {
    pub fn simple_static(
        &mut self,
        reader: &StringReader,
        error_type: &'static CommandErrorType<0>,
        suggestions: &[&'static str],
    ) {
        self.store(
            reader,
            || DelayedCommandSyntaxError {
                error_type,
                java_translation_key: error_type.java_translation_key,
                arguments: vec![],
            },
            |entries| {
                for suggestion in suggestions {
                    entries.push(Cow::Borrowed(*suggestion));
                }
            },
        );
    }

    pub fn dynamic_static<A: Into<Cow<'static, str>>>(
        &mut self,
        reader: &StringReader,
        error_type: &'static CommandErrorType<1>,
        arg1: A,
        suggestions: &[&'static str],
    ) {
        self.store(
            reader,
            || DelayedCommandSyntaxError {
                error_type,
                java_translation_key: error_type.java_translation_key,
                arguments: vec![arg1.into()],
            },
            |entries| {
                for suggestion in suggestions {
                    entries.push(Cow::Borrowed(*suggestion));
                }
            },
        );
    }

    pub fn simple(
        &mut self,
        reader: &StringReader,
        error_type: &'static CommandErrorType<0>,
        suggestions: Vec<String>,
    ) {
        self.store(
            reader,
            || DelayedCommandSyntaxError {
                error_type,
                java_translation_key: error_type.java_translation_key,
                arguments: vec![],
            },
            |entries| {
                for suggestion in suggestions {
                    entries.push(Cow::Owned(suggestion));
                }
            },
        );
    }

    pub fn dynamic<A: Into<Cow<'static, str>>>(
        &mut self,
        reader: &StringReader,
        error_type: &'static CommandErrorType<1>,
        arg1: A,
        suggestions: Vec<String>,
    ) {
        self.store(
            reader,
            || DelayedCommandSyntaxError {
                error_type,
                java_translation_key: error_type.java_translation_key,
                arguments: vec![arg1.into()],
            },
            |entries| {
                for suggestion in suggestions {
                    entries.push(Cow::Owned(suggestion));
                }
            },
        );
    }

    #[inline]
    fn store(
        &mut self,
        reader: &StringReader,
        error: impl FnOnce() -> DelayedCommandSyntaxError,
        suggestions: impl FnOnce(&mut Vec<Cow<'static, str>>),
    ) {
        let current = self.cursor;
        let new = reader.cursor();

        if self.command_error.is_none() || new > current {
            self.command_error = Some(error());
            self.cursor = new;
            self.suggestions.clear();
            suggestions(&mut self.suggestions);
        } else if new == current {
            suggestions(&mut self.suggestions);
        }
    }
}

/// 一个由 `SnbtParser` 等类似规则的解析器实现的 trait。
pub trait Parser<'r, 's> {
    fn state_mut(&mut self) -> (&mut StringReader<'s>, &mut ParserErrors);

    /// 记录解析期间发生的简单错误，并添加建议予以纠正。
    fn store_simple_error_and_suggest(
        &mut self,
        error_type: &'static CommandErrorType<0>,
        suggestions: &[&'static str],
    ) {
        let (reader, errors) = self.state_mut();
        errors.simple_static(reader, error_type, suggestions);
    }

    /// 记录解析期间发生的动态错误，并添加建议予以纠正。
    fn store_dynamic_error_and_suggest(
        &mut self,
        error_type: &'static CommandErrorType<1>,
        arg1: impl Into<Cow<'static, str>>,
        suggestions: &[&'static str],
    ) {
        let (reader, errors) = self.state_mut();
        errors.dynamic_static(reader, error_type, arg1, suggestions);
    }

    /// 记录解析期间发生的简单错误。
    fn store_simple_error(&mut self, error_type: &'static CommandErrorType<0>) {
        let (reader, errors) = self.state_mut();
        errors.simple(reader, error_type, Vec::new());
    }

    /// 记录解析期间发生的动态错误。
    fn store_dynamic_error(
        &mut self,
        error_type: &'static CommandErrorType<1>,
        arg1: impl Into<Cow<'static, str>>,
    ) {
        let (reader, errors) = self.state_mut();
        errors.dynamic(reader, error_type, arg1, Vec::new());
    }
}
