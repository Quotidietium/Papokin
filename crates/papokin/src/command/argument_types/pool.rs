use papokin_command::argument_types::FromStringReader;
use papokin_command::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use papokin_command::context::command_context::CommandContext;
use papokin_command::errors::command_syntax_error::CommandSyntaxError;
use papokin_command::source::CommandSource;
use papokin_command::string_reader::StringReader;
use papokin_command::suggestion::suggestions::{Suggestions, SuggestionsBuilder};
use papokin_protocol::java::client::play::SuggestionProviders;
use papokin_util::identifier::Identifier;

pub struct PoolNameArgumentType;

impl<S: CommandSource> ArgumentType<S> for PoolNameArgumentType {
    type Item = Identifier;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        Identifier::from_reader(reader)
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        let names = papokin_world::generation::structure::template::all_pool_names();
        builder
            .filter_and_suggest_iter(names.iter().map(|n| format!("minecraft:{n}")))
            .build()
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::ResourceLocation
    }

    fn override_suggestion_providers(&self) -> Option<SuggestionProviders> {
        Some(SuggestionProviders::AskServer)
    }

    fn examples(&self) -> Vec<String> {
        vec!["minecraft:village/plains/houses".to_string()]
    }
}
