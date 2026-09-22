use std::sync::Arc;

use crate::block::entities::jukebox::JukeboxBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BrokenArgs, EmitsRedstonePowerArgs, GetComparatorOutputArgs,
    GetRedstonePowerArgs, NormalUseArgs, OnStateReplacedArgs, PlacedArgs, UseWithItemArgs,
};
use crate::entity::Entity;
use crate::entity::item::ItemEntity;
use crate::world::World;
use papokin_data::data_component_impl::JukeboxPlayableImpl;
use papokin_data::entity::EntityType;
use papokin_data::jukebox_song::JukeboxSong;
use papokin_data::world::WorldEvent;
use papokin_data::{Block, BlockStateId, block_properties::JukeboxLikeProperties};
use papokin_macros::pumpkin_block;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_world::world::BlockFlags;
use rand::{RngExt, rng};

use tracing::error;

#[pumpkin_block("minecraft:jukebox")]
pub struct JukeboxBlock;

impl JukeboxBlock {
    fn has_record_state(_block: &Block, state_id: BlockStateId) -> bool {
        JukeboxLikeProperties::from_state_id(state_id).has_record
    }

    fn set_record_state(has_record: bool, block: &Block, position: &BlockPos, world: &Arc<World>) {
        let new_state = JukeboxLikeProperties { has_record };
        world.set_block_state(
            position,
            new_state.to_state_id(block),
            BlockFlags::NOTIFY_LISTENERS,
        );
    }

    /// 从唱片机中取出唱片——与原版的 `JukeboxBlockEntity.dropRecord()` 一致
    /// 在 (pos + 0.5, pos + 1.01, pos + 0.5) 处生成物品，并带水平随机偏移
    fn drop_record(position: &BlockPos, world: &Arc<World>) {
        if let Some(block_entity) = world.get_block_entity(position)
            && let Some(jukebox_entity) = block_entity.as_any().downcast_ref::<JukeboxBlockEntity>()
        {
            let record = jukebox_entity.clear_record();
            if !record.is_empty() {
                // 原版：Vec3d.add(pos, 0.5, 1.01, 0.5).addHorizontalRandom(random, 0.7F)
                // addHorizontalRandom 在 [-0.35, 0.35] 范围内取随机数并加到 x 和 z 上
                let spawn_pos = Vector3::new(
                    f64::from(position.0.x) + 0.5 + rng().random_range(-0.35..0.35),
                    f64::from(position.0.y) + 1.01,
                    f64::from(position.0.z) + 0.5 + rng().random_range(-0.35..0.35),
                );

                let entity = Entity::new(world.clone(), spawn_pos, &EntityType::ITEM);
                // 原版：setToDefaultPickupDelay() = 10 刻
                let item_entity = Arc::new(ItemEntity::new(entity, record));
                world.spawn_entity(item_entity);
            }
        }
    }

    /// 停止音乐并更新方块状态
    fn stop_playing(block: &Block, position: &BlockPos, world: &Arc<World>) {
        Self::set_record_state(false, block, position, world);
        world.sync_world_event(WorldEvent::SoundStopJukeboxSong, *position, 0);
    }

    /// 开始播放音乐
    fn start_playing(position: &BlockPos, world: &Arc<World>, song_id: u32) {
        world.sync_world_event(WorldEvent::SoundPlayJukeboxSong, *position, song_id as i32);
    }
}

impl BlockBehaviour for JukeboxBlock {
    /// 当唱片机被放置时调用 - 创建方块实体
    fn placed(&self, args: PlacedArgs<'_>) {
        let block_entity = JukeboxBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(block_entity));
    }

    /// 当玩家空手或手持非唱片物品右键时调用
    /// 原版：`JukeboxBlock.onUse()` - 如有唱片则掉落
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state_id = args.world.get_block_state(args.position).id;

        // 原版：if (state.get(HAS_RECORD) && world.getBlockEntity(pos) instanceof JukeboxBlockEntity lv)
        if Self::has_record_state(args.block, state_id) {
            // 掉落唱片
            Self::drop_record(args.position, args.world);
            // 停止音乐并更新方块状态
            Self::stop_playing(args.block, args.position, args.world);
            return BlockActionResult::Success;
        }

        BlockActionResult::Pass
    }

    /// 当玩家手持物品右键时调用
    /// 原版：`JukeboxBlock.onUseWithItem()` -> `JukeboxPlayableComponent.tryPlayStack()`
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let world = args.world;
        let state_id = world.get_block_state(args.position).id;

        // 原版：if (state.get(HAS_RECORD)) return PASS_TO_DEFAULT_BLOCK_ACTION
        if Self::has_record_state(args.block, state_id) {
            return BlockActionResult::PassToDefaultBlockAction;
        }

        let item_stack = &mut *args.item_stack;

        // 原版：JukeboxPlayableComponent lv = stack.get(DataComponentTypes.JUKEBOX_PLAYABLE)
        let jukebox_playable = item_stack
            .get_data_component::<JukeboxPlayableImpl>()
            .map(|i| i.song);

        // 原版：if (lv == null) return PASS_TO_DEFAULT_BLOCK_ACTION
        let Some(jukebox_playable) = jukebox_playable else {
            return BlockActionResult::PassToDefaultBlockAction;
        };

        let Some(song_name) = jukebox_playable.split(':').nth(1) else {
            return BlockActionResult::PassToDefaultBlockAction;
        };

        let Some(jukebox_song) = JukeboxSong::from_name(song_name) else {
            error!("唱片机可播放的歌曲未注册：{song_name}");
            return BlockActionResult::PassToDefaultBlockAction;
        };

        // 原版：ItemStack lv3 = stack.splitUnlessCreative(1, player)
        let record = item_stack.split_unless_creative(args.player.gamemode.load(), 1);

        // 原版：lv4.setStack(lv3)
        if let Some(block_entity) = world.get_block_entity(args.position)
            && let Some(jukebox_entity) = block_entity.as_any().downcast_ref::<JukeboxBlockEntity>()
        {
            jukebox_entity.set_record(record);
            // 开始以歌曲时长跟踪播放
            jukebox_entity.start_playing(jukebox_song.length_in_ticks());
        }

        // 将方块状态更新为 has_record = true
        Self::set_record_state(true, args.block, args.position, world);

        // 开始播放音乐（客户端音频）
        Self::start_playing(args.position, world, jukebox_song.get_id());

        args.player.increment_stat(
            papokin_data::statistic::StatisticCategory::Custom,
            papokin_data::statistic::CustomStatistic::PlayRecord as i32,
            1,
        );

        // TODO: world.emitGameEvent(GameEvent.BLOCK_CHANGE, pos, ...)

        BlockActionResult::Success
    }

    /// 当唱片机被破坏时调用
    fn broken(&self, args: BrokenArgs<'_>) {
        // 若有唱片则掉落
        Self::drop_record(args.position, args.world);
        // 停止音乐
        args.world
            .sync_world_event(WorldEvent::SoundStopJukeboxSong, *args.position, 0);
    }

    /// 原版：`JukeboxBlock.onStateReplaced()` -> `ItemScatterer.onStateReplaced()`
    fn on_state_replaced(&self, _args: OnStateReplacedArgs<'_>) {
        // 原版调用 ItemScatterer.onStateReplaced，它会更新比较器
        // TODO: world.updateComparators(pos, block) 实现后再调用
    }

    /// 原版：`JukeboxBlock.emitsRedstonePower()` 返回 true
    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    /// 原版：播放时返回 15，否则返回 0
    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        // 原版：return world.getBlockEntity(pos) instanceof JukeboxBlockEntity lv && lv.getManager().isPlaying() ? 15 : 0
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(jukebox_entity) = block_entity.as_any().downcast_ref::<JukeboxBlockEntity>()
            && jukebox_entity.is_playing()
        {
            15
        } else {
            0
        }
    }

    /// 原版：返回唱片的比较器输出（0-15）
    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        // 原版：return world.getBlockEntity(pos) instanceof JukeboxBlockEntity lv ? lv.getComparatorOutput() : 0
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(jukebox_entity) = block_entity.as_any().downcast_ref::<JukeboxBlockEntity>()
        {
            let record = jukebox_entity.get_record();
            // 从唱片的 jukebox_playable 组件获取歌曲
            if let Some(playable) = record.get_data_component::<JukeboxPlayableImpl>()
                && let Some(song_name) = playable.song.rsplit(':').next()
                && let Some(song) = JukeboxSong::from_name(song_name)
            {
                return Some(song.comparator_output());
            }
        }
        Some(0)
    }
}
