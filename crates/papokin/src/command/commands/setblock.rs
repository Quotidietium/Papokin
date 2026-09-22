use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;
use papokin_world::world::BlockFlags;

use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::block::BlockArgumentType;
use crate::command::argument_types::coordinates::block_pos::BlockPosArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};

const DESCRIPTION: &str = "放置一个方块。";
const PERMISSION: &str = "minecraft:command.setblock";

const ERROR_FAILED: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMANDS_SETBLOCK_FAILED);

#[derive(Clone, Copy)]
enum Mode {
    /// 伴随粒子效果 + 物品掉落
    Destroy,

    /// 只替换空气
    Keep,

    /// 默认；不带粒子时
    Replace,

    /// 放置方块而不触发其周围的更新
    Strict,
}

struct SetBlockExecutor(Mode);

impl CommandExecutor for SetBlockExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let block = BlockArgumentType::get(context, "block")?;
        let block_state_id = block.default_state.id;
        let mode = self.0;
        let world = context.source.world();
        let pos = BlockPosArgumentType::get_loaded_block_pos(context, "pos")?;

        let success = match mode {
            Mode::Destroy => {
                world.break_block(
                    &pos,
                    None,
                    BlockFlags::SKIP_DROPS | BlockFlags::NOTIFY_ALL | BlockFlags::FORCE_STATE,
                );
                world.set_block_state(
                    &pos,
                    block_state_id,
                    BlockFlags::NOTIFY_ALL | BlockFlags::FORCE_STATE,
                );
                true
            }
            Mode::Replace => {
                world.set_block_state(
                    &pos,
                    block_state_id,
                    BlockFlags::NOTIFY_ALL | BlockFlags::FORCE_STATE,
                );
                true
            }
            Mode::Keep => {
                let old_state = world.get_block_state(&pos);
                if old_state.is_air() {
                    world.set_block_state(
                        &pos,
                        block_state_id,
                        BlockFlags::NOTIFY_ALL | BlockFlags::FORCE_STATE,
                    );
                    true
                } else {
                    false
                }
            }
            Mode::Strict => {
                world.set_block_state(
                    &pos,
                    block_state_id,
                    BlockFlags::NOTIFY_LISTENERS
                        | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK
                        | BlockFlags::FORCE_STATE,
                );
                true
            }
        };

        if success {
            world.flush_block_updates();
            context.source.send_feedback(
                TextComponent::translate(
                    papokin_data::translation::java::COMMANDS_SETBLOCK_SUCCESS,
                    [
                        TextComponent::text(pos.0.x.to_string()),
                        TextComponent::text(pos.0.y.to_string()),
                        TextComponent::text(pos.0.z.to_string()),
                    ],
                ),
                true,
            );
            Ok(1)
        } else {
            Err(ERROR_FAILED.create_without_context())
        }
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("setblock", DESCRIPTION).requires(PERMISSION).then(
            argument("pos", BlockPosArgumentType).then(
                argument("block", BlockArgumentType)
                    .executes(SetBlockExecutor(Mode::Replace))
                    .then(literal("destroy").executes(SetBlockExecutor(Mode::Destroy)))
                    .then(literal("keep").executes(SetBlockExecutor(Mode::Keep)))
                    .then(literal("replace").executes(SetBlockExecutor(Mode::Replace)))
                    .then(literal("strict").executes(SetBlockExecutor(Mode::Strict))),
            ),
        ),
    );
}
