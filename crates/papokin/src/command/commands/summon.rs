use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::difficulty::Difficulty;
use papokin_util::math::position::BlockPos;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;
use uuid::Uuid;

use crate::command::argument_builder::{ArgumentBuilder, argument, command};
use crate::command::argument_types::coordinates::vec3::Vec3ArgumentType;
use crate::command::argument_types::nbt::NbtCompoundArgumentType;
use crate::command::argument_types::resource::{ENTITY_TYPE_ARGUMENT, ResourceArgument};
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::entity::r#type::from_type;
use crate::world::World;

const DESCRIPTION: &str = "在指定位置生成一个实体。";
const PERMISSION: &str = "minecraft:command.summon";

const ERROR_FAILED_PEACEFUL: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_SUMMON_FAILED_PEACEFUL);
const ERROR_DUPLICATE_UUID: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_SUMMON_FAILED_UUID);
const INVALID_POSITION: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_SUMMON_INVALIDPOSITION);

struct SummonExecutor {
    has_pos: bool,
    has_nbt: bool,
}

impl CommandExecutor for SummonExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let entity_type = ResourceArgument::get_summonable_entity_type(context, "entity")?;
        let pos = if self.has_pos {
            Vec3ArgumentType::get_coordinates(context, "pos")?.resolve(&context.source)
        } else {
            context.source.position
        };

        let block_pos = BlockPos::floored_v(pos);
        if !World::is_valid(block_pos) {
            return Err(INVALID_POSITION.create_without_context());
        }

        let world = context.source.world();
        let difficulty = world.level_info.load().difficulty;
        if difficulty == Difficulty::Peaceful && !entity_type.category.is_friendly {
            return Err(ERROR_FAILED_PEACEFUL.create_without_context());
        }

        let nbt = if self.has_nbt {
            Some(NbtCompoundArgumentType::get(context, "nbt")?)
        } else {
            None
        };

        let uuid = nbt
            .and_then(|nbt| nbt.get_uuid("UUID"))
            .unwrap_or_else(Uuid::new_v4);

        let entity = from_type(entity_type, pos, world, uuid);
        if let Some(nbt) = nbt {
            entity.read_nbt_non_mut(nbt);
            entity.get_entity().set_pos(pos);
        }

        if world
            .entities
            .load()
            .iter()
            .any(|e| e.get_entity().entity_uuid == entity.get_entity().entity_uuid)
        {
            return Err(ERROR_DUPLICATE_UUID.create_without_context());
        }

        let name = entity.get_display_name();
        world.spawn_entity(entity);

        context.source.send_feedback(
            TextComponent::translate(translation::java::COMMANDS_SUMMON_SUCCESS, [name]),
            true,
        );

        Ok(1)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("summon", DESCRIPTION).requires(PERMISSION).then(
            argument("entity", ENTITY_TYPE_ARGUMENT.clone())
                .executes(SummonExecutor {
                    has_pos: false,
                    has_nbt: false,
                })
                .then(
                    argument("pos", Vec3ArgumentType::Default)
                        .executes(SummonExecutor {
                            has_pos: true,
                            has_nbt: false,
                        })
                        .then(
                            argument("nbt", NbtCompoundArgumentType).executes(SummonExecutor {
                                has_pos: true,
                                has_nbt: true,
                            }),
                        ),
                ),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::commands::fetchprofile;
    use crate::command::context::command_source::CommandSource;
    use std::sync::Arc;

    #[test]
    fn parse_summon_cat() {
        let mut dispatcher = CommandDispatcher::new();
        let registry = PermissionRegistry::default();

        // 先注册 fetchprofile，确保其 "entity" 参数不冲突
        fetchprofile::register(&mut dispatcher, &registry);
        register(&mut dispatcher, &registry);

        let source = Arc::new(CommandSource::dummy());
        let result = dispatcher.parse_input("summon cat", &source);
        assert!(result.errors.is_empty());
        let context = result.context.build("summon cat");
        let entity_type = ResourceArgument::get_summonable_entity_type(&context, "entity")
            .expect("应能解析可召唤的猫实体类型");
        assert_eq!(entity_type.resource_name, "cat");
    }

    #[test]
    fn parse_summon_cat_with_pos_and_nbt() {
        let mut dispatcher = CommandDispatcher::new();
        let registry = PermissionRegistry::default();

        fetchprofile::register(&mut dispatcher, &registry);
        register(&mut dispatcher, &registry);

        let source = Arc::new(CommandSource::dummy());
        let result = dispatcher.parse_input("summon cat ~ ~ ~ {NoAI:1b}", &source);
        assert!(result.errors.is_empty());
        let context = result.context.build("summon cat ~ ~ ~ {NoAI:1b}");
        let entity_type = ResourceArgument::get_summonable_entity_type(&context, "entity")
            .expect("应能解析可召唤的猫实体类型");
        assert_eq!(entity_type.resource_name, "cat");
        let nbt = NbtCompoundArgumentType::get(&context, "nbt").expect("应能解析 NBT 参数");
        assert_eq!(nbt.get_byte("NoAI"), Some(1));
    }
}
