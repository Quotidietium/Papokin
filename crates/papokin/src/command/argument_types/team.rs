use crate::command::{
    CommandSource,
    argument_types::argument_type::{ArgumentType, JavaClientArgumentType},
    context::command_context::CommandContext,
    errors::command_syntax_error::CommandSyntaxError,
    string_reader::StringReader,
    suggestion::suggestions::{Suggestions, SuggestionsBuilder},
};

/// 表示解析队伍名称的参数类型。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TeamArgumentType;

impl ArgumentType<CommandSource> for TeamArgumentType {
    type Item = String;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let name = reader.read_unquoted_string();
        Ok(name)
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::Team
    }

    fn list_suggestions(
        &self,
        context: &CommandContext,
        mut builder: SuggestionsBuilder,
    ) -> Suggestions {
        let scoreboard = context
            .world()
            .scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for team_name in scoreboard.get_teams().keys() {
            builder = builder.filter_and_suggest_one(team_name.as_str());
        }
        builder.build()
    }

    fn examples(&self) -> Vec<String> {
        vec!["foo".to_string(), "foo_bar".to_string()]
    }
}

impl TeamArgumentType {
    ///以字符串切片的形式返回 [`CommandContext`] 中已解析的 `String` 参数。
    pub fn get<'a>(context: &'a CommandContext, name: &str) -> Result<&'a str, CommandSyntaxError> {
        Ok(context.get_argument::<String>(name)?.as_str())
    }
}
