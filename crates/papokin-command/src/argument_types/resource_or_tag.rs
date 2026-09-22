use papokin_data::structures::StructureSet;
use papokin_data::tag::{self, RegistryKey};
use papokin_data::translation;
use papokin_util::identifier::Identifier;
use papokin_util::text::TextComponent;

use crate::argument_types::FromStringReader;
use crate::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::context::command_context::CommandContext;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::CommandErrorType;
use crate::string_reader::StringReader;
use crate::suggestion::suggestions::{Suggestions, SuggestionsBuilder};

pub static BIOME_REGISTRY: Identifier = Identifier::vanilla_static("worldgen/biome");
pub static STRUCTURE_REGISTRY: Identifier = Identifier::vanilla_static("worldgen/structure");
pub static POI_REGISTRY: Identifier = Identifier::vanilla_static("point_of_interest_type");

pub static ERROR_UNKNOWN_RESOURCE: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_RESOURCE_NOT_FOUND);

pub static ERROR_UNKNOWN_TAG: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_RESOURCE_TAG_NOT_FOUND);

/// 一个已解析的引用，指向单个注册表条目或以 `#` 为前缀的标签
/// 条目列表，由 [`ResourceOrTagKeyArgument`] 和
/// [`ResourceOrTagArgument`]。
#[derive(Debug, Clone)]
pub enum ResourceOrTag {
    Resource(Identifier),
    Tag(Identifier),
}

impl ResourceOrTag {
    /// 此引用面向用户的显示形式（原版的 `asPrintable`）：
    /// 条目使用 `namespace:path`，标签使用 `#namespace:path`。
    #[must_use]
    pub fn printable(&self) -> String {
        match self {
            Self::Resource(id) => id.to_string(),
            Self::Tag(id) => format!("#{id}"),
        }
    }

    fn from_string_reader(reader: &mut StringReader) -> Result<Self, CommandSyntaxError> {
        if reader.peek() == Some('#') {
            reader.skip();
            Ok(Self::Tag(Identifier::from_reader(reader)?))
        } else {
            Ok(Self::Resource(Identifier::from_reader(reader)?))
        }
    }
}

/// 由两种参数类型共享的注册表感知建议：已知条目
/// id，以及（当该注册表存在标签数据时）带 `#` 前缀的标签。
fn suggest_for_registry(registry: &Identifier, builder: SuggestionsBuilder) -> Suggestions {
    let tag_names = |key: RegistryKey| {
        tag::get_latest_map(key)
            .into_iter()
            .flat_map(|map| map.keys().map(|tag| format!("#{tag}")))
    };

    if *registry == STRUCTURE_REGISTRY {
        // 生成器模拟的是原版的结构集，因此这些就是
        // 可定位的名称。没有可提供的结构标签数据。
        builder
            .filter_and_suggest_iter(
                StructureSet::NAMES
                    .iter()
                    .map(|name| format!("minecraft:{name}")),
            )
            .build()
    } else if *registry == BIOME_REGISTRY {
        let biomes = papokin_data::biome::Biome::ALL
            .iter()
            .map(|biome| format!("minecraft:{}", biome.registry_id));
        builder
            .filter_and_suggest_iter(biomes.chain(tag_names(RegistryKey::WorldgenBiome)))
            .build()
    } else if *registry == POI_REGISTRY {
        // 目前（还）没有生成的 POI 类型注册表，因此提供这些类型
        // 标签数据中已知的那些，再加上服务器实际创建的那些。
        let poi_types = [
            "minecraft:armorer",
            "minecraft:butcher",
            "minecraft:cartographer",
            "minecraft:cleric",
            "minecraft:farmer",
            "minecraft:fisherman",
            "minecraft:fletcher",
            "minecraft:leatherworker",
            "minecraft:librarian",
            "minecraft:mason",
            "minecraft:shepherd",
            "minecraft:toolsmith",
            "minecraft:weaponsmith",
            "minecraft:home",
            "minecraft:meeting",
            "minecraft:beehive",
            "minecraft:bee_nest",
            "minecraft:nether_portal",
            "minecraft:lodestone",
            "minecraft:lightning_rod",
        ];
        builder
            .filter_and_suggest_iter(
                poi_types
                    .iter()
                    .map(|&s| s.to_string())
                    .chain(tag_names(RegistryKey::PointOfInterestType)),
            )
            .build()
    } else {
        builder.build()
    }
}

/// 一种参数类型，解析注册表的条目 ID 或 `#` 标签。
///
/// 该值不会对照注册表进行校验，类似于原版的
/// `ResourceOrTagKeyArgument`；解析（及相应的错误）交由
/// 该命令。
pub struct ResourceOrTagKeyArgument(pub Identifier);

impl<S: crate::source::CommandSource> ArgumentType<S> for ResourceOrTagKeyArgument {
    type Item = ResourceOrTag;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        ResourceOrTag::from_string_reader(reader)
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        suggest_for_registry(&self.0, builder)
    }

    fn client_side_parser(&self) -> JavaClientArgumentType {
        JavaClientArgumentType::ResourceOrTagKey {
            identifier: self.0.clone(),
        }
    }

    fn examples(&self) -> Vec<String> {
        examples!("foo", "foo:bar", "#foo")
    }
}

/// 一种参数类型，解析并验证条目 ID 或 `#` 标签，该条目属于某个
/// 注册表。
///
/// 验证在解析时进行，此时注册表数据可用，例如
/// 原版的 `ResourceOrTagArgument`：目前涵盖生物群系 ID 以及
/// 有生成标签数据的每个注册表的标签；没有生成数据的注册表的 id
/// 生成的条目数据（如 POI 类型）按原样接受。
pub struct ResourceOrTagArgument(pub Identifier);

impl<S: crate::source::CommandSource> ArgumentType<S> for ResourceOrTagArgument {
    type Item = ResourceOrTag;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let value = ResourceOrTag::from_string_reader(reader)?;

        match &value {
            ResourceOrTag::Resource(id) => {
                if self.0 == BIOME_REGISTRY
                    && !(id.is_vanilla()
                        && papokin_data::biome::Biome::from_name(id.path()).is_some())
                {
                    return Err(ERROR_UNKNOWN_RESOURCE.create_without_context(
                        TextComponent::text(id.to_string()),
                        TextComponent::text(self.0.to_string()),
                    ));
                }
            }
            ResourceOrTag::Tag(id) => {
                let registry_key = if self.0 == BIOME_REGISTRY {
                    Some(RegistryKey::WorldgenBiome)
                } else if self.0 == POI_REGISTRY {
                    Some(RegistryKey::PointOfInterestType)
                } else {
                    None
                };

                if let Some(key) = registry_key
                    && tag::get_tag_values(key, &id.to_string()).is_none()
                {
                    return Err(ERROR_UNKNOWN_TAG.create_without_context(
                        TextComponent::text(id.to_string()),
                        TextComponent::text(self.0.to_string()),
                    ));
                }
            }
        }

        Ok(value)
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        suggest_for_registry(&self.0, builder)
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::ResourceOrTag {
            identifier: self.0.clone(),
        }
    }

    fn examples(&self) -> Vec<String> {
        examples!("foo", "foo:bar", "#foo")
    }
}
