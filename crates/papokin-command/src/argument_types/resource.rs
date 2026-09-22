use crate::argument_types::FromStringReader;
use crate::argument_types::argument_type::{ArgumentType, JavaClientArgumentType};
use crate::context::command_context::CommandContext;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::{CommandErrorType, DISPATCHER_PARSE_EXCEPTION};
use crate::node::attached::AttachedNode;
use crate::string_reader::StringReader;
use crate::suggestion::suggestions::{Suggestions, SuggestionsBuilder};
use papokin_data::entity::EntityType;
use papokin_data::translation;
use papokin_protocol::java::client::play::SuggestionProviders;
use papokin_util::identifier::Identifier;
use papokin_util::text::TextComponent;
use std::any::Any;
use std::iter::Iterator;

pub static ENTITY_TYPE_REGISTRY: &Identifier = &Identifier::vanilla_static("entity_type");
pub static ENCHANTMENT_REGISTRY: &Identifier = &Identifier::vanilla_static("enchantment");
pub static MOB_EFFECT_REGISTRY: &Identifier = &Identifier::vanilla_static("mob_effect");
pub static DAMAGE_TYPE_REGISTRY: &Identifier = &Identifier::vanilla_static("damage_type");

static DAMAGE_TYPES: [papokin_data::damage::DamageType; 51] = [
    papokin_data::damage::DamageType::ARROW,
    papokin_data::damage::DamageType::BAD_RESPAWN_POINT,
    papokin_data::damage::DamageType::CACTUS,
    papokin_data::damage::DamageType::CAMPFIRE,
    papokin_data::damage::DamageType::CRAMMING,
    papokin_data::damage::DamageType::DRAGON_BREATH,
    papokin_data::damage::DamageType::DROWN,
    papokin_data::damage::DamageType::DRY_OUT,
    papokin_data::damage::DamageType::ENDER_PEARL,
    papokin_data::damage::DamageType::EXPLOSION,
    papokin_data::damage::DamageType::FALL,
    papokin_data::damage::DamageType::FALLING_ANVIL,
    papokin_data::damage::DamageType::FALLING_BLOCK,
    papokin_data::damage::DamageType::FALLING_STALACTITE,
    papokin_data::damage::DamageType::FIREBALL,
    papokin_data::damage::DamageType::FIREWORKS,
    papokin_data::damage::DamageType::FLY_INTO_WALL,
    papokin_data::damage::DamageType::FREEZE,
    papokin_data::damage::DamageType::GENERIC,
    papokin_data::damage::DamageType::GENERIC_KILL,
    papokin_data::damage::DamageType::HOT_FLOOR,
    papokin_data::damage::DamageType::IN_FIRE,
    papokin_data::damage::DamageType::IN_WALL,
    papokin_data::damage::DamageType::INDIRECT_MAGIC,
    papokin_data::damage::DamageType::LAVA,
    papokin_data::damage::DamageType::LIGHTNING_BOLT,
    papokin_data::damage::DamageType::MACE_SMASH,
    papokin_data::damage::DamageType::MAGIC,
    papokin_data::damage::DamageType::MOB_ATTACK,
    papokin_data::damage::DamageType::MOB_ATTACK_NO_AGGRO,
    papokin_data::damage::DamageType::MOB_PROJECTILE,
    papokin_data::damage::DamageType::ON_FIRE,
    papokin_data::damage::DamageType::OUT_OF_WORLD,
    papokin_data::damage::DamageType::OUTSIDE_BORDER,
    papokin_data::damage::DamageType::PLAYER_ATTACK,
    papokin_data::damage::DamageType::PLAYER_EXPLOSION,
    papokin_data::damage::DamageType::SONIC_BOOM,
    papokin_data::damage::DamageType::SPEAR,
    papokin_data::damage::DamageType::SPIT,
    papokin_data::damage::DamageType::STALAGMITE,
    papokin_data::damage::DamageType::STARVE,
    papokin_data::damage::DamageType::STING,
    papokin_data::damage::DamageType::SULFUR_CUBE_HOT,
    papokin_data::damage::DamageType::SWEET_BERRY_BUSH,
    papokin_data::damage::DamageType::THORNS,
    papokin_data::damage::DamageType::THROWN,
    papokin_data::damage::DamageType::TRIDENT,
    papokin_data::damage::DamageType::UNATTRIBUTED_FIREBALL,
    papokin_data::damage::DamageType::WIND_CHARGE,
    papokin_data::damage::DamageType::WITHER,
    papokin_data::damage::DamageType::WITHER_SKULL,
];

static ERROR_UNKNOWN_RESOURCE: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_RESOURCE_NOT_FOUND);

static ERROR_INVALID_RESOURCE_TYPE: CommandErrorType<3> =
    CommandErrorType::new(translation::java::ARGUMENT_RESOURCE_INVALID_TYPE);

static ERROR_NOT_SUMMONABLE_ENTITY: CommandErrorType<1> =
    CommandErrorType::new(translation::java::ENTITY_NOT_SUMMONABLE);

pub static ENTITY_TYPE_ARGUMENT: ResourceArgument =
    ResourceArgument(ENTITY_TYPE_REGISTRY, &|id: Identifier| {
        EntityType::from_name(id.path()).map(|value| value as &'static (dyn Any + Send + Sync))
    });

pub static ENCHANTMENT_ARGUMENT: ResourceArgument =
    ResourceArgument(ENCHANTMENT_REGISTRY, &|id: Identifier| {
        papokin_data::Enchantment::from_name(id.path())
            .or_else(|| papokin_data::Enchantment::from_name(&id.to_string()))
            .map(|value| value as &'static (dyn Any + Send + Sync))
    });

pub static MOB_EFFECT_ARGUMENT: ResourceArgument =
    ResourceArgument(MOB_EFFECT_REGISTRY, &|id: Identifier| {
        papokin_data::effect::StatusEffect::from_name(id.path())
            .or_else(|| papokin_data::effect::StatusEffect::from_minecraft_name(&id.to_string()))
            .map(|value| value as &'static (dyn Any + Send + Sync))
    });

pub static DAMAGE_TYPE_ARGUMENT: ResourceArgument =
    ResourceArgument(DAMAGE_TYPE_REGISTRY, &|id: Identifier| {
        let dt = papokin_data::damage::DamageType::from_name(id.path())
            .or_else(|| papokin_data::damage::DamageType::from_name(&id.to_string()))?;
        let idx = dt.id as usize;
        DAMAGE_TYPES
            .get(idx)
            .map(|val| val as &'static (dyn Any + Send + Sync))
    });

#[derive(Clone)]
pub struct ResourceArgument(
    pub &'static Identifier,
    pub &'static (dyn Fn(Identifier) -> Option<&'static (dyn Any + Send + Sync)> + Send + Sync),
);

impl<S: crate::source::CommandSource> ArgumentType<S> for ResourceArgument {
    type Item = &'static (dyn Any + Send + Sync);

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let identifier = Identifier::from_reader(reader)?;
        self.1(identifier.clone()).ok_or_else(|| {
            ERROR_UNKNOWN_RESOURCE.create(
                reader,
                TextComponent::text(identifier.path().to_string()),
                TextComponent::text(self.0.to_string()),
            )
        })
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        suggestions_builder: SuggestionsBuilder,
    ) -> Suggestions {
        if self.0 == ENTITY_TYPE_REGISTRY {
            let entity_types = EntityType::ALL
                .iter()
                .filter(|entity_type| entity_type.summonable)
                .map(|entity_type| format!("minecraft:{}", entity_type.resource_name));
            suggestions_builder
                .filter_and_suggest_iter(entity_types)
                .build()
        } else if self.0 == ENCHANTMENT_REGISTRY {
            let enchantments = papokin_data::Enchantment::ALL
                .iter()
                .map(|e| format!("minecraft:{}", e.name));
            suggestions_builder
                .filter_and_suggest_iter(enchantments)
                .build()
        } else if self.0 == MOB_EFFECT_REGISTRY {
            let effects = [
                &papokin_data::effect::StatusEffect::ABSORPTION,
                &papokin_data::effect::StatusEffect::BAD_OMEN,
                &papokin_data::effect::StatusEffect::BLINDNESS,
                &papokin_data::effect::StatusEffect::BREATH_OF_THE_NAUTILUS,
                &papokin_data::effect::StatusEffect::CONDUIT_POWER,
                &papokin_data::effect::StatusEffect::DARKNESS,
                &papokin_data::effect::StatusEffect::DOLPHINS_GRACE,
                &papokin_data::effect::StatusEffect::FIRE_RESISTANCE,
                &papokin_data::effect::StatusEffect::GLOWING,
                &papokin_data::effect::StatusEffect::HASTE,
                &papokin_data::effect::StatusEffect::HEALTH_BOOST,
                &papokin_data::effect::StatusEffect::HERO_OF_THE_VILLAGE,
                &papokin_data::effect::StatusEffect::HUNGER,
                &papokin_data::effect::StatusEffect::INFESTED,
                &papokin_data::effect::StatusEffect::INSTANT_DAMAGE,
                &papokin_data::effect::StatusEffect::INSTANT_HEALTH,
                &papokin_data::effect::StatusEffect::INVISIBILITY,
                &papokin_data::effect::StatusEffect::JUMP_BOOST,
                &papokin_data::effect::StatusEffect::LEVITATION,
                &papokin_data::effect::StatusEffect::LUCK,
                &papokin_data::effect::StatusEffect::MINING_FATIGUE,
                &papokin_data::effect::StatusEffect::NAUSEA,
                &papokin_data::effect::StatusEffect::NIGHT_VISION,
                &papokin_data::effect::StatusEffect::OOZING,
                &papokin_data::effect::StatusEffect::POISON,
                &papokin_data::effect::StatusEffect::RAID_OMEN,
                &papokin_data::effect::StatusEffect::REGENERATION,
                &papokin_data::effect::StatusEffect::RESISTANCE,
                &papokin_data::effect::StatusEffect::SATURATION,
                &papokin_data::effect::StatusEffect::SLOW_FALLING,
                &papokin_data::effect::StatusEffect::SLOWNESS,
                &papokin_data::effect::StatusEffect::SPEED,
                &papokin_data::effect::StatusEffect::STRENGTH,
                &papokin_data::effect::StatusEffect::TRIAL_OMEN,
                &papokin_data::effect::StatusEffect::UNLUCK,
                &papokin_data::effect::StatusEffect::WATER_BREATHING,
                &papokin_data::effect::StatusEffect::WEAKNESS,
                &papokin_data::effect::StatusEffect::WEAVING,
                &papokin_data::effect::StatusEffect::WIND_CHARGED,
                &papokin_data::effect::StatusEffect::WITHER,
            ]
            .iter()
            .map(|e| e.minecraft_name.to_string());
            suggestions_builder.filter_and_suggest_iter(effects).build()
        } else if self.0 == DAMAGE_TYPE_REGISTRY {
            let types = DAMAGE_TYPES
                .iter()
                .map(|dt| format!("minecraft:{}", dt.message_id));
            suggestions_builder.filter_and_suggest_iter(types).build()
        } else {
            Suggestions::empty()
        }
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::Resource {
            identifier: self.0.clone(),
        }
    }

    fn override_suggestion_providers(&self) -> Option<SuggestionProviders> {
        (self.0 == ENTITY_TYPE_REGISTRY).then_some(SuggestionProviders::SummonableEntities)
    }
}

impl ResourceArgument {
    pub fn get_resource<T: 'static, S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
        registry_key: &Identifier,
    ) -> Result<&'static T, CommandSyntaxError> {
        let missing_argument = DISPATCHER_PARSE_EXCEPTION
            .create_without_context(TextComponent::text(format!("找不到名称为 '{name}' 的参数")));
        let node = context
            .nodes
            .iter()
            .rev()
            .find_map(|parsed| {
                if let AttachedNode::Argument(cur) = &context.tree[parsed.node]
                    && cur.meta.name == name
                {
                    Some(cur)
                } else {
                    None
                }
            })
            .or_else(|| {
                context.tree.iter().find_map(|node| {
                    if let AttachedNode::Argument(cur) = node
                        && cur.meta.name == name
                    {
                        Some(cur)
                    } else {
                        None
                    }
                })
            })
            .ok_or(missing_argument.clone())?;
        let invalid_argument = DISPATCHER_PARSE_EXCEPTION.create_without_context(
            TextComponent::text(format!("名称为 '{name}' 的参数不是 ResourceArgument")),
        );
        let result_argument = node
            .meta
            .argument_type
            .as_any()
            .downcast_ref::<Self>()
            .ok_or(invalid_argument)?;
        let registry_name = result_argument.0;
        let identifier = context
            .arguments
            .get(name)
            .ok_or(missing_argument)?
            .range
            .substring_slice(context.input.as_str())
            .to_string();
        let err = ERROR_INVALID_RESOURCE_TYPE.create_without_context(
            TextComponent::text(identifier),
            TextComponent::text(registry_name.to_string()),
            TextComponent::text(registry_key.to_string()),
        );
        if registry_name == registry_key {
            context
                .get_argument::<&'static (dyn Any + Send + Sync)>(name)?
                .downcast_ref::<T>()
                .ok_or(err)
        } else {
            Err(err)
        }
    }

    pub fn get_entity_type<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<&'static EntityType, CommandSyntaxError> {
        Self::get_resource(context, name, ENTITY_TYPE_REGISTRY)
    }

    pub fn get_enchantment<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<&'static papokin_data::Enchantment, CommandSyntaxError> {
        Self::get_resource(context, name, ENCHANTMENT_REGISTRY)
    }

    pub fn get_mob_effect<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<&'static papokin_data::effect::StatusEffect, CommandSyntaxError> {
        Self::get_resource(context, name, MOB_EFFECT_REGISTRY)
    }

    pub fn get_damage_type<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<&'static papokin_data::damage::DamageType, CommandSyntaxError> {
        Self::get_resource(context, name, DAMAGE_TYPE_REGISTRY)
    }

    pub fn get_summonable_entity_type<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<&'static EntityType, CommandSyntaxError> {
        let val: &'static EntityType = Self::get_resource(context, name, ENTITY_TYPE_REGISTRY)?;
        if val.summonable {
            Ok(val)
        } else {
            Err(ERROR_NOT_SUMMONABLE_ENTITY
                .create_without_context(TextComponent::text(val.resource_name)))
        }
    }
}
