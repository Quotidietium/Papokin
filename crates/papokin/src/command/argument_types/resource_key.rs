use crate::command::CommandSource;
use crate::command::argument_types::FromStringReader;
use crate::command::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::command::context::command_context::CommandContext;
use crate::command::errors::command_syntax_error::CommandSyntaxError;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::string_reader::StringReader;
use crate::command::suggestion::suggestions::{Suggestions, SuggestionsBuilder};
use papokin_data::{Advancement, translation};
use papokin_util::identifier::Identifier;
use papokin_util::resource::ResourceKey;
use papokin_util::text::TextComponent;
use std::string::ToString;

pub static ADVANCEMENT_REGISTRY: &Identifier = &Identifier::vanilla_static("advancement");
pub static BIOME_REGISTRY: &Identifier = &Identifier::vanilla_static("worldgen/biome");

pub const ERROR_INVALID_ADVANCEMENT: CommandErrorType<1> =
    CommandErrorType::new(translation::java::ADVANCEMENT_ADVANCEMENTNOTFOUND);

pub const ERROR_INVALID_BIOME: CommandErrorType<1> =
    CommandErrorType::new("commands.fillbiome.invalid");

pub const ERROR_NOT_SUMMONABLE_ENTITY: CommandErrorType<1> =
    CommandErrorType::new(translation::java::ENTITY_NOT_SUMMONABLE);

/// 表示用于从标识符获取资源键的参数类型。
///
/// 如果你想要 [`Advancement`]，请放入 [`ADVANCEMENT_REGISTRY`]
/// 并在解析完成后用 [`ResourceKeyArgument::get_advancement`] 获取它
///
/// TODO 配方
///
/// 如果你只想要 [`ResourceKey`]，请使用 [`ResourceKeyArgument::get_registry_key`] 函数
pub struct ResourceKeyArgument(pub &'static Identifier);

pub static ERROR_INVALID: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_ID_INVALID);

impl ArgumentType<CommandSource> for ResourceKeyArgument {
    type Item = ResourceKey;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let identifier = Identifier::from_reader(reader)?;
        Ok(ResourceKey::new(self.0.clone(), identifier))
    }

    fn list_suggestions(
        &self,
        context: &CommandContext,
        suggestions_builder: SuggestionsBuilder,
    ) -> Suggestions {
        if self.0 == ADVANCEMENT_REGISTRY {
            let advancements = context.server().advancement_manager.get_advancements();
            suggestions_builder
                .filter_and_suggest_iter(advancements.iter().map(ToString::to_string))
                .build()
        } else if self.0 == BIOME_REGISTRY {
            let biomes = papokin_data::biome::Biome::ALL
                .iter()
                .map(|biome| format!("minecraft:{}", biome.registry_id));
            suggestions_builder.filter_and_suggest_iter(biomes).build()
        } else {
            Suggestions::empty()
        }
    }

    fn client_side_parser(&self) -> JavaClientArgumentType {
        JavaClientArgumentType::ResourceKey {
            identifier: self.0.clone(),
        }
    }
}

impl ResourceKeyArgument {
    ///将 [`CommandContext`] 中已解析的资源键参数作为 [`Advancement`] 返回。
    ///
    /// # Arguments
    /// * `context` - 包含以给定参数名解析出的 [`ResourceKey`] 的 [`CommandContext`]。
    /// * `name` - 已解析参数的名称。
    ///
    /// # Returns
    /// 包含从资源键参数中取得的进度的 `Advancement`，包装在 `Ok` 中，
    /// 或在无法解析或其他情况下返回带有相应 [`CommandSyntaxError`] 的 `Err`
    /// 该键对应未知的进度。
    pub fn get_advancement(
        context: &CommandContext,
        name: &str,
    ) -> Result<&'static Advancement, CommandSyntaxError> {
        let resource_key: &ResourceKey = Self::get_registry_key(
            context,
            name,
            ADVANCEMENT_REGISTRY,
            &ERROR_INVALID_ADVANCEMENT,
        )?;
        Advancement::from_name(resource_key.identifier.path()).ok_or_else(|| {
            ERROR_INVALID_ADVANCEMENT.create_without_context(TextComponent::text(
                resource_key.identifier.path().to_string(),
            ))
        })
    }

    ///将 [`CommandContext`] 中已解析的资源键参数作为 [`Biome`](papokin_data::biome::Biome) 返回。
    pub fn get_biome(
        context: &CommandContext,
        name: &str,
    ) -> Result<&'static papokin_data::biome::Biome, CommandSyntaxError> {
        let resource_key: &ResourceKey =
            Self::get_registry_key(context, name, BIOME_REGISTRY, &ERROR_INVALID_BIOME)?;
        let path = resource_key.identifier.path();
        papokin_data::biome::Biome::from_name(path).ok_or_else(|| {
            ERROR_INVALID_BIOME
                .create_without_context(TextComponent::text(resource_key.identifier.to_string()))
        })
    }

    ///以 [`ResourceKey`] 的形式返回 [`CommandContext`] 中已解析的资源键参数。
    ///
    /// # Arguments
    /// * `context` - 包含以给定参数名解析出的 [`ResourceKey`] 的 [`CommandContext`]。
    /// * `name` - 已解析参数的名称。
    ///
    /// # Returns
    /// 包含从参数中取得的键的 `ResourceKey`，包装在 `Ok` 中，
    /// 或在无法解析时返回带有相应 [`CommandSyntaxError`] 的 `Err`
    pub fn get_registry_key<'a>(
        context: &'a CommandContext,
        name: &str,
        registry: &Identifier,
        error: &'static CommandErrorType<1>,
    ) -> Result<&'a ResourceKey, CommandSyntaxError> {
        let argument = context.get_argument::<ResourceKey>(name)?;
        argument.cast(registry).ok_or_else(|| {
            error
                .create_without_context(TextComponent::text(argument.identifier.path().to_string()))
        })
    }
}
