use std::sync::{Arc, Mutex};

use crate::block::entities::hanging_sign::HangingSignBlockEntity;
use crate::block::entities::sign::{SignBlockEntity, SignEntityRef, Text};
use crate::command::CommandSender;
use crate::command::context::command_source::CommandSource;
use papokin_data::Block;
use papokin_data::BlockDirection;
use papokin_data::BlockId;
use papokin_data::BlockStateId;
use papokin_data::HorizontalFacingExt;
use papokin_data::block_properties::EnumVariants;
use papokin_data::fluid::Fluid;
use papokin_data::tag::Taggable;
use papokin_inventory::screen_handler::InventoryPlayer;
use papokin_macros::pumpkin_block_from_tag;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector2::Vector2;
use papokin_util::math::vector3::Vector3;
use papokin_util::text::TextComponent;
use papokin_util::text::click::ClickEvent;
use papokin_world::tick::TickPriority;
use uuid::Uuid;

use crate::block::BlockBehaviour;
use crate::block::CanPlaceAtArgs;
use crate::block::GetStateForNeighborUpdateArgs;
use crate::block::NormalUseArgs;
use crate::block::OnPlaceArgs;
use crate::block::OnStateReplacedArgs;
use crate::block::PlacedArgs;
use crate::block::PlayerPlacedArgs;
use crate::block::UseWithItemArgs;
use crate::block::registry::BlockActionResult;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::items::dye::DyeItem;
use crate::item::items::glowing_ink_sac::GlowingInkSacItem;
use crate::item::items::honeycomb::HoneyCombItem;
use crate::item::items::ink_sac::InkSacItem;
use crate::world::World;
use papokin_protocol::java::client::play::COpenSignEditor;

#[pumpkin_block_from_tag("minecraft:all_signs")]
pub struct SignBlock;

/// 用于保存支撑检测结果的辅助结构体
struct SupportInfo {
    above_is_valid: bool,
    side_direction: Option<BlockDirection>,
}

/// 用于告示牌放置配置的辅助结构体
struct SignPlacement {
    block_id: BlockId,
    facing: Option<String>,
    rotation: Option<u8>,
    attached: bool,
    waterlogged: bool,
}

impl SignBlock {
    /// 检查方块能否为告示牌提供支撑。
    fn is_valid_support(
        world: &World,
        pos: &BlockPos,
        direction: BlockDirection,
        is_hanging: bool,
    ) -> bool {
        let (block, state) = world.get_block_and_state(pos);

        // 普通木牌测试旧版固体标志，与 `isSolid` 保持一致
        // `StandingSignBlock::canSurvive` / `WallSignBlock::canSurvive`.
        if !is_hanging {
            return state.is_solid();
        }

        let is_permissive = block.has_tag(&papokin_data::tag::Block::MINECRAFT_LEAVES)
            || block.has_tag(&papokin_data::tag::Block::MINECRAFT_SIGNS);

        match direction {
            BlockDirection::Up => state.is_side_solid(BlockDirection::Down) || is_permissive,
            BlockDirection::Down => state.is_center_solid(BlockDirection::Up) || is_permissive,
            _ => state.is_side_solid(direction.opposite()) || is_permissive,
        }
    }

    /// 检测某个位置周围可用的支撑点。
    fn detect_support(world: &World, position: &BlockPos, is_hanging: bool) -> SupportInfo {
        let (block_above, state_above) = world.get_block_and_state(&position.up());
        let above_is_valid = state_above.is_side_solid(BlockDirection::Down)
            || block_above.has_tag(&papokin_data::tag::Block::MINECRAFT_SIGNS)
            || block_above.has_tag(&papokin_data::tag::Block::MINECRAFT_LEAVES);

        let mut side_direction = None;
        for direction in BlockDirection::horizontal() {
            let pos = position.offset(direction.to_offset());
            if Self::is_valid_support(
                world,
                &pos,
                direction.opposite().to_block_direction(),
                is_hanging,
            ) {
                side_direction = Some(direction);
                break;
            }
        }

        SupportInfo {
            above_is_valid,
            side_direction: side_direction.map(|d| d.to_block_direction()),
        }
    }

    /// 确定墙面悬挂式告示牌的合适朝向。
    fn calculate_wall_hanging_facing(wall_dir: BlockDirection, player_yaw: f32) -> &'static str {
        match wall_dir {
            BlockDirection::North | BlockDirection::South => {
                // 墙沿南北方向延伸，告示牌朝东或朝西
                if (player_yaw + 360.0) % 360.0 < 180.0 {
                    "east"
                } else {
                    "west"
                }
            }
            BlockDirection::East | BlockDirection::West => {
                // 墙沿东西方向延伸，告示牌朝北或朝南
                if (-270.0..=270.0).contains(&player_yaw) {
                    "south"
                } else {
                    "north"
                }
            }
            _ => wall_dir.opposite().to_cardinal_direction().to_value(),
        }
    }

    /// 计算挂在墙面上的告示牌的旋转角度。
    fn calculate_wall_hanging_rotation(
        wall_dir: BlockDirection,
        player_rot: u8,
        is_sneaking: bool,
    ) -> u8 {
        if is_sneaking {
            return player_rot;
        }

        match wall_dir {
            BlockDirection::North | BlockDirection::South => {
                // 对齐到南北轴（0 或 8）
                if (4..12).contains(&player_rot) { 8 } else { 0 }
            }
            BlockDirection::East | BlockDirection::West => {
                // 对齐到东西轴（4 或 12）
                if (2..10).contains(&player_rot) { 4 } else { 12 }
            }
            _ => player_rot,
        }
    }

    /// 确定告示牌的方块变体和放置属性。
    fn determine_placement(args: &OnPlaceArgs, support: &SupportInfo) -> Option<SignPlacement> {
        let is_hanging = args.block.name.contains("hanging");
        let is_sneaking = args.player.get_entity().is_sneaking();

        // 选择方块变体
        let block_id = if is_hanging {
            Self::select_hanging_variant(args, support)?
        } else {
            Self::select_standing_variant(args, support)
        };

        let actual_block = Block::from_id(block_id);
        let is_wall_hanging = is_hanging && actual_block.name.contains("wall_hanging");

        // 计算朝向
        let (facing, rotation, attached) = if is_wall_hanging {
            Self::calculate_wall_hanging_orientation(args, support, is_sneaking)
        } else if is_hanging {
            Self::calculate_ceiling_orientation(args, is_sneaking)
        } else if actual_block.name.contains("wall") {
            Self::calculate_wall_orientation(args)
        } else {
            Self::calculate_standing_orientation(args)
        };

        Some(SignPlacement {
            block_id,
            facing,
            rotation,
            attached,
            waterlogged: args.replacing.water_source(),
        })
    }

    /// 选择合适的悬挂式告示牌变体。
    fn select_hanging_variant(args: &OnPlaceArgs, support: &SupportInfo) -> Option<BlockId> {
        if args.direction == BlockDirection::Down && support.above_is_valid {
            Some(args.block.id) // 悬挂于天花板
        } else if (args.direction.is_horizontal() || args.direction == BlockDirection::Up)
            && support.side_direction.is_some()
        {
            Some(get_sign_variant(args.block, true)) // 带支柱的壁挂式
        } else if support.above_is_valid {
            Some(args.block.id)
        } else {
            None // 无有效放置
        }
    }

    /// 选择合适的立式告示牌变体。
    fn select_standing_variant(args: &OnPlaceArgs, support: &SupportInfo) -> BlockId {
        if args.direction.is_horizontal() && support.side_direction.is_some() {
            get_sign_variant(args.block, false) // 墙面告示牌
        } else {
            args.block.id // 立式告示牌
        }
    }

    /// 计算挂在墙面上的告示牌的朝向。
    fn calculate_wall_hanging_orientation(
        args: &OnPlaceArgs,
        support: &SupportInfo,
        is_sneaking: bool,
    ) -> (Option<String>, Option<u8>, bool) {
        let wall_dir = if args.direction.is_horizontal() {
            args.direction
        } else {
            support.side_direction.unwrap_or(args.direction)
        };

        let player_yaw = args.player.get_entity().yaw.load();
        let facing = Self::calculate_wall_hanging_facing(wall_dir, player_yaw);

        let player_rot = args.player.get_entity().get_flipped_rotation_16();
        let rotation = Self::calculate_wall_hanging_rotation(wall_dir, player_rot, is_sneaking);

        let is_angled = rotation % 4 != 0;
        let attached = is_angled || is_sneaking;

        (Some(facing.to_string()), Some(rotation), attached)
    }

    /// 计算悬挂在天花板上的告示牌的朝向。
    fn calculate_ceiling_orientation(
        args: &OnPlaceArgs,
        is_sneaking: bool,
    ) -> (Option<String>, Option<u8>, bool) {
        let rotation = if is_sneaking {
            args.player.get_entity().get_flipped_rotation_16()
        } else {
            // 对齐到最近的方位
            let index = args.player.get_entity().get_flipped_rotation_16();
            ((index + 2) / 4 * 4) % 16
        };

        let is_angled = rotation % 4 != 0;
        let attached = is_angled || is_sneaking;

        (None, Some(rotation), attached)
    }

    /// 计算墙面告示牌的朝向。
    fn calculate_wall_orientation(args: &OnPlaceArgs) -> (Option<String>, Option<u8>, bool) {
        let facing = args.direction.opposite().to_cardinal_direction().to_value();
        (Some(facing.to_string()), None, false)
    }

    /// 计算立式告示牌的朝向。
    fn calculate_standing_orientation(args: &OnPlaceArgs) -> (Option<String>, Option<u8>, bool) {
        let rotation = args.player.get_entity().get_flipped_rotation_16();
        (None, Some(rotation), false)
    }

    /// 将放置属性应用到方块上。
    fn apply_placement_properties(block: &Block, placement: &SignPlacement) -> BlockStateId {
        let mut props = block
            .properties(block.default_state.id)
            .map(|p| p.to_props())
            .unwrap_or_default();

        if let Some(facing) = &placement.facing
            && let Some(prop) = props.iter_mut().find(|(k, _)| *k == "facing")
        {
            prop.1 = facing;
        }

        if let Some(rotation) = placement.rotation
            && let Some(prop) = props.iter_mut().find(|(k, _)| *k == "rotation")
        {
            prop.1 = match rotation {
                1 => "1",
                2 => "2",
                3 => "3",
                4 => "4",
                5 => "5",
                6 => "6",
                7 => "7",
                8 => "8",
                9 => "9",
                10 => "10",
                11 => "11",
                12 => "12",
                13 => "13",
                14 => "14",
                15 => "15",
                _ => "0",
            };
        }

        if let Some(prop) = props.iter_mut().find(|(k, _)| *k == "attached") {
            prop.1 = if placement.attached { "true" } else { "false" };
        }

        if let Some(prop) = props.iter_mut().find(|(k, _)| *k == "waterlogged") {
            prop.1 = if placement.waterlogged {
                "true"
            } else {
                "false"
            };
        }

        block.from_properties(&props).to_state_id(block)
    }
}

impl BlockBehaviour for SignBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let is_hanging = args.block.name.contains("hanging");
        let support = Self::detect_support(args.world, args.position, is_hanging);

        let Some(placement) = Self::determine_placement(&args, &support) else {
            return BlockStateId::AIR; // 无效的放置
        };

        let actual_block = Block::from_id(placement.block_id);
        Self::apply_placement_properties(actual_block, &placement)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.block.name.contains("hanging") {
            args.world
                .add_block_entity(Arc::new(HangingSignBlockEntity::empty(*args.position)));
        } else {
            args.world
                .add_block_entity(Arc::new(SignBlockEntity::empty(*args.position)));
        }
    }

    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(sign) = SignEntityRef::from_block_entity(&*block_entity)
        {
            open_text_edit(
                args.player,
                sign.currently_editing_player(),
                args.position,
                true,
            );
            return;
        }
        args.player
            .try_send_client_packet(&COpenSignEditor::new(*args.position, true));
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let is_hanging = args.block.name.contains("hanging");
        let clicked_face = args
            .use_item_on
            .and_then(|u| papokin_data::BlockDirection::try_from(u.face.0).ok())
            .unwrap_or(papokin_data::BlockDirection::Up);

        // 检测从地板到墙面的附着（目前有问题）
        if is_hanging && clicked_face == BlockDirection::Up {
            for d in papokin_data::BlockDirection::horizontal() {
                let wall_pos = args.position.offset(d.to_offset());
                let (block, state) = args.block_accessor.get_block_and_state(&wall_pos);
                if state.is_side_solid(d.opposite().to_block_direction())
                    || block.has_tag(&papokin_data::tag::Block::MINECRAFT_LEAVES)
                    || block.has_tag(&papokin_data::tag::Block::MINECRAFT_SIGNS)
                {
                    return true;
                }
            }
        }

        // 使用宽松标签的标准支撑校验
        let support_pos = match clicked_face {
            BlockDirection::Up => args.position.down(),
            BlockDirection::Down => args.position.up(),
            _ => args.position.offset(clicked_face.opposite().to_offset()),
        };

        let (block, state) = args.block_accessor.get_block_and_state(&support_pos);
        let is_permissive = block.has_tag(&papokin_data::tag::Block::MINECRAFT_LEAVES)
            || block.has_tag(&papokin_data::tag::Block::MINECRAFT_SIGNS);

        match clicked_face {
            // 立式告示牌需要下方有旧式实心方块，
            // 对应 `StandingSignBlock::canSurvive` 中的 `isSolid`。
            BlockDirection::Up => !is_hanging && state.is_solid(),
            BlockDirection::Down => {
                is_hanging && (state.is_side_solid(BlockDirection::Down) || is_permissive)
            }
            _ => {
                if is_hanging {
                    state.is_side_solid(clicked_face.opposite()) || is_permissive
                } else {
                    // 墙面告示牌背后需要有一个旧版实心方块，
                    // 对应 `WallSignBlock::canSurvive` 中的 `isSolid`。
                    state.is_solid()
                }
            }
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world.remove_block_entity(args.position);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let is_hanging = args.block.name.contains("hanging");
        let is_wall_sign = args.block.name.contains("wall");

        // 确定预期的支撑方向
        let support_dir = if is_wall_sign {
            // 查找 'facing' 属性以找到墙牌背后的支撑方块
            get_wall_support_direction(args.block, args.state_id)
        } else if is_hanging {
            // 悬挂于天花板的告示牌始终朝上
            Some(BlockDirection::Up)
        } else {
            // 立式告示牌始终朝下
            Some(BlockDirection::Down)
        };

        if let Some(dir) = support_dir {
            let support_pos = args.position.offset(dir.to_offset());
            let support_state = args.world.get_block_state(&support_pos);

            let is_leaf = args
                .world
                .get_block(&support_pos)
                .has_tag(&papokin_data::tag::Block::MINECRAFT_LEAVES);

            let is_sign = args
                .world
                .get_block(&support_pos)
                .has_tag(&papokin_data::tag::Block::MINECRAFT_ALL_SIGNS);

            let is_valid = match dir {
                BlockDirection::Up => {
                    support_state.is_center_solid(BlockDirection::Down) || is_leaf || is_sign
                }
                // 立式告示牌可依附在旧式实心方块上，
                // 对应 `StandingSignBlock::canSurvive` 中的 `isSolid`。
                BlockDirection::Down => support_state.is_solid(),
                _ => {
                    if is_hanging {
                        support_state.is_side_solid(dir.opposite()) || is_leaf || is_sign
                    } else {
                        // 墙面告示牌依附于旧版实心方块存活，
                        // 对应 `WallSignBlock::canSurvive` 中的 `isSolid`。
                        support_state.is_solid()
                    }
                }
            };

            if !is_valid {
                return BlockStateId::AIR;
            }
        }

        // 幸存的水浸告示牌使其周围的水保持流动，
        // 对应 `SignBlock::updateShape` 中的 `scheduleTick`。
        if args.state_id.is_waterlogged() {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u8,
                TickPriority::Normal,
            );
        }

        args.state_id
    }

    /// 处理对告示牌方块的常规使用（右键点击）。
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let Some(block_entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::Pass;
        };
        let Some(sign_entity) = SignEntityRef::from_block_entity(&*block_entity) else {
            return BlockActionResult::Pass;
        };

        let is_front_text =
            is_facing_front_text(args.world, args.position, args.block, args.player);
        let text = sign_entity.get_text(is_front_text);

        let executed_click_command =
            execute_click_commands_if_present(args.world, args.player, args.position, text);

        if sign_entity.is_waxed() {
            let is_hanging = args.block.name.contains("hanging");
            let sound = if is_hanging {
                papokin_data::sound::Sound::BlockHangingSignWaxedInteractFail
            } else {
                papokin_data::sound::Sound::BlockSignWaxedInteractFail
            };
            args.world.play_block_sound(
                sound,
                papokin_data::sound::SoundCategory::Blocks,
                *args.position,
            );
            BlockActionResult::SuccessServer
        } else if executed_click_command {
            BlockActionResult::SuccessServer
        } else if !other_player_is_editing_sign(
            args.player,
            sign_entity.currently_editing_player(),
            args.world,
            args.position,
        ) && args.player.may_build()
            && has_editable_text(text)
        {
            open_text_edit(
                args.player,
                sign_entity.currently_editing_player(),
                args.position,
                is_front_text,
            );
            BlockActionResult::SuccessServer
        } else {
            BlockActionResult::Pass
        }
    }

    /// 处理持物品对告示牌方块的使用。
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let Some(block_entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::Pass;
        };
        let Some(sign_entity) = SignEntityRef::from_block_entity(&*block_entity) else {
            return BlockActionResult::Pass;
        };

        let Some(pumpkin_item) = args
            .server
            .item_registry
            .get_pumpkin_item(args.item_stack.item.id)
        else {
            return BlockActionResult::PassToDefaultBlockAction;
        };

        let is_applicator = pumpkin_item.as_any().is::<HoneyCombItem>()
            || pumpkin_item.as_any().is::<GlowingInkSacItem>()
            || pumpkin_item.as_any().is::<InkSacItem>()
            || pumpkin_item.as_any().is::<DyeItem>();

        let has_applicator_to_use = is_applicator && args.player.may_build();

        if has_applicator_to_use
            && !sign_entity.is_waxed()
            && !other_player_is_editing_sign(
                args.player,
                sign_entity.currently_editing_player(),
                args.world,
                args.position,
            )
        {
            let is_front_text =
                is_facing_front_text(args.world, args.position, args.block, args.player);
            let text = sign_entity.get_text(is_front_text);

            let result = if let Some(honeycomb_item) =
                pumpkin_item.as_any().downcast_ref::<HoneyCombItem>()
            {
                honeycomb_item.apply_to_sign(&args, &block_entity, &sign_entity)
            } else if let Some(g_ink_sac_item) =
                pumpkin_item.as_any().downcast_ref::<GlowingInkSacItem>()
            {
                g_ink_sac_item.apply_to_sign(&args, &block_entity, text)
            } else if let Some(ink_sac_item) = pumpkin_item.as_any().downcast_ref::<InkSacItem>() {
                ink_sac_item.apply_to_sign(&args, &block_entity, text)
            } else if let Some(dye) = pumpkin_item.as_any().downcast_ref::<DyeItem>() {
                let color_name = args
                    .item_stack
                    .item
                    .registry_key
                    .strip_suffix("_dye")
                    .unwrap_or(args.item_stack.item.registry_key);
                dye.apply_to_sign(&args, &block_entity, text, color_name)
            } else {
                BlockActionResult::PassToDefaultBlockAction
            };

            if result == BlockActionResult::Success {
                execute_click_commands_if_present(args.world, args.player, args.position, text);
                if pumpkin_item.as_any().is::<GlowingInkSacItem>() {
                    args.player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::GlowedSign,
                    );
                }
                if !args.player.has_infinite_materials() {
                    args.item_stack.decrement(1);
                }
                return BlockActionResult::Success;
            }
        }

        BlockActionResult::PassToDefaultBlockAction
    }
}

/// 为玩家打开告示牌文本编辑界面，并将其注册为允许的编辑者。
fn open_text_edit(
    player: &Player,
    currently_editing_player: &Arc<Mutex<Option<Uuid>>>,
    position: &BlockPos,
    is_front_text: bool,
) {
    *currently_editing_player
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(player.gameprofile.id);
    player.try_send_client_packet(&COpenSignEditor::new(*position, is_front_text));
}

/// 检查是否有其他玩家正在可触及范围内编辑此告示牌。
fn other_player_is_editing_sign(
    player: &Player,
    currently_editing_player: &Arc<Mutex<Option<Uuid>>>,
    world: &World,
    position: &BlockPos,
) -> bool {
    let currently_editing = currently_editing_player
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(editing_player_id) = *currently_editing
        && editing_player_id != player.gameprofile.id
        && let Some(editing_player) = world.get_player_by_uuid(editing_player_id)
        && editing_player.can_interact_with_block_at(position, 4.0)
    {
        return true;
    }
    false
}

/// 检查告示牌给定文本面上的所有消息是否为纯文本或为空。
fn has_editable_text(text: &Text) -> bool {
    let messages = text
        .messages
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    messages.iter().all(|msg| is_plain_or_empty_text(msg))
}

fn is_plain_or_empty_text(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    if !text.starts_with('{') {
        return true;
    }
    match serde_json::from_str::<TextComponent>(text) {
        Ok(component) => {
            component.0.style.click_event.is_none()
                && component.0.style.hover_event.is_none()
                && component.0.extra.is_empty()
                && matches!(
                    *component.0.content,
                    papokin_util::text::TextContent::Text { .. }
                )
        }
        Err(_) => true,
    }
}

/// 执行告示牌文本消息中定义的所有 `run_command` 点击事件。
fn execute_click_commands_if_present(
    world: &Arc<World>,
    player: &Arc<Player>,
    position: &BlockPos,
    text: &Text,
) -> bool {
    let Some(server) = world.server.upgrade() else {
        return false;
    };

    let messages = text
        .messages
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut has_run_command = false;
    for msg in messages.iter() {
        if msg.is_empty() || !msg.starts_with('{') {
            continue;
        }
        if let Ok(component) = serde_json::from_str::<TextComponent>(msg)
            && let Some(ClickEvent::RunCommand { command }) = &component.0.style.click_event
        {
            // 让插件预处理（并可选择取消/重写）该
            // 告示牌的点击命令，然后才运行。
            let mut event = crate::plugin::api::events::player::player_sign_command_preprocess::PlayerSignCommandPreprocessEvent::new(
                player.clone(),
                *position,
                command.clone(),
            );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                continue;
            }

            let source = CommandSource::new(
                CommandSender::Dummy,
                world.clone(),
                Some(player.clone()),
                position.to_centered_f64(),
                Vector2::new(0.0, 0.0),
                player.gameprofile.name.clone(),
                player.get_display_name(),
                server.clone(),
            );
            let command_str = event.command.strip_prefix('/').unwrap_or(&event.command);
            let dispatcher = server.command_dispatcher.load();
            dispatcher.handle_command(&source, command_str);
            has_run_command = true;
        }
    }
    has_run_command
}

///返回支撑墙上告示牌的方块的方向。
fn get_wall_support_direction(block: &Block, state_id: BlockStateId) -> Option<BlockDirection> {
    block.properties(state_id).and_then(|props| {
        let prop_map = props.to_props();
        prop_map
            .into_iter()
            .find(|(k, _)| k == &"facing")
            .map(|(_, v)| match v {
                "north" => BlockDirection::South,
                "south" => BlockDirection::North,
                "east" => BlockDirection::West,
                _ => BlockDirection::East, // "west" 及默认情况
            })
    })
}

/// 辅助函数：将普通告示牌转换为墙壁变体。
///返回墙变体的方块 ID；若未找到，则返回基础方块的 ID。
fn get_sign_variant(base: &Block, is_hanging: bool) -> BlockId {
    let base_name = base.name;
    let wood_type = base_name
        .strip_suffix("_hanging_sign")
        .or_else(|| base_name.strip_suffix("_sign"))
        .unwrap_or("oak");

    let target_name = if is_hanging {
        // 这是提供“水平木柱”的变体
        format!("{wood_type}_wall_hanging_sign")
    } else {
        format!("{wood_type}_wall_sign")
    };

    papokin_data::Block::from_name(&target_name).map_or(base.id, |b| b.id)
}

fn is_facing_front_text(
    world: &World,
    location: &BlockPos,
    block: &Block,
    player: &Player,
) -> bool {
    let state_id = world.get_block_state_id(location);
    // 动态读取属性：某些告示牌类型使用 `rotation` 属性（0..15），
    // 其他（墙上告示牌）使用 `facing` 属性（north/south/west/east），
    // 悬挂式告示牌可能带有 `rotation` + `attached`。
    let mut rotation: f32 = 0.0;
    if let Some(props) = block.properties(state_id) {
        let prop_map = props.to_props();
        if let Some((_, val)) = prop_map.iter().find(|(k, _)| k == &"rotation") {
            let r = val.parse().unwrap_or(0);
            rotation = get_yaw_from_rotation_16(r);
        } else if let Some((_, val)) = prop_map.iter().find(|(k, _)| k == &"facing") {
            rotation = match &val[..] {
                "north" => 180.0,
                "west" => 90.0,
                "east" => -90.0,
                _ => 0.0,
            };
        }
    }
    let bounding_box = Vector3::new(0.5, 0.5, 0.5);

    let d = player.eye_position().x - (f64::from(location.0.x) + bounding_box.x);
    let d1 = player.eye_position().z - (f64::from(location.0.z) + bounding_box.z);

    let f = (d1.atan2(d).to_degrees() as f32) - 90.0;

    let diff = (f - rotation + 180.0).rem_euclid(360.0) - 180.0;
    diff.abs() <= 90.0
}

fn get_yaw_from_rotation_16(rotation: u8) -> f32 {
    f32::from(rotation) * 22.5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(block: &Block, waterlogged: bool) -> SignPlacement {
        SignPlacement {
            block_id: block.id,
            facing: None,
            rotation: None,
            attached: false,
            waterlogged,
        }
    }

    /// `apply_placement_properties` 按字符串匹配属性名，因此
    /// 重命名或缺失 `waterlogged` 会导致告示牌悄无声息地停止含水。
    #[test]
    fn placement_carries_waterlogging_into_the_state() {
        for block in [
            &Block::OAK_SIGN,
            &Block::OAK_WALL_SIGN,
            &Block::OAK_HANGING_SIGN,
            &Block::OAK_WALL_HANGING_SIGN,
        ] {
            for waterlogged in [false, true] {
                let state_id =
                    SignBlock::apply_placement_properties(block, &placement(block, waterlogged));
                assert_eq!(
                    state_id.is_waterlogged(),
                    waterlogged,
                    "{} placed with waterlogged={waterlogged}",
                    block.name
                );
            }
        }
    }
}
