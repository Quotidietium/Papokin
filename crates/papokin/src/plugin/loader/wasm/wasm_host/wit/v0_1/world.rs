use papokin_data::block_properties::NoteblockInstrument as InternalNoteblockInstrument;
use papokin_data::block_state::PistonBehavior;
use papokin_data::{BlockDirection as InternalBlockDirection, BlockId, BlockStateId};
use papokin_protocol::PositionFlag;
use papokin_util::math::position::BlockPos;
use papokin_world::chunk::ChunkHeightmapType;
use papokin_world::chunk::io::Dirtiable;
use papokin_world::world::BlockFlags;
use std::sync::{Arc, Mutex as StdMutex};
use wasmtime::component::{Access, HasSelf, Resource};

use crate::block::entities::banner::BannerBlockEntity as InternalBannerBlockEntity;
use crate::block::entities::barrel::BarrelBlockEntity as InternalBarrelBlockEntity;
use crate::block::entities::beacon::BeaconBlockEntity as InternalBeaconBlockEntity;
use crate::block::entities::bed::BedBlockEntity as InternalBedBlockEntity;
use crate::block::entities::beehive::BeehiveBlockEntity as InternalBeehiveBlockEntity;
use crate::block::entities::bell::BellBlockEntity as InternalBellBlockEntity;
use crate::block::entities::blasting_furnace::BlastingFurnaceBlockEntity as InternalBlastingFurnaceBlockEntity;
use crate::block::entities::brewing_stand::BrewingStandBlockEntity as InternalBrewingStandBlockEntity;
use crate::block::entities::brushable_block::BrushableBlockBlockEntity as InternalBrushableBlockBlockEntity;
use crate::block::entities::calibrated_sculk_sensor::CalibratedSculkSensorBlockEntity as InternalCalibratedSculkSensorBlockEntity;
use crate::block::entities::campfire::CampfireBlockEntity as InternalCampfireBlockEntity;
use crate::block::entities::chest::ChestBlockEntity as InternalChestBlockEntity;
use crate::block::entities::chiseled_bookshelf::ChiseledBookshelfBlockEntity as InternalChiseledBookshelfBlockEntity;
use crate::block::entities::command_block::CommandBlockEntity as InternalCommandBlockEntity;
use crate::block::entities::comparator::ComparatorBlockEntity as InternalComparatorBlockEntity;
use crate::block::entities::conduit::ConduitBlockEntity as InternalConduitBlockEntity;
use crate::block::entities::copper_golem_statue::CopperGolemStatueBlockEntity as InternalCopperGolemStatueBlockEntity;
use crate::block::entities::crafter::CrafterBlockEntity as InternalCrafterBlockEntity;
use crate::block::entities::creaking_heart::CreakingHeartBlockEntity as InternalCreakingHeartBlockEntity;
use crate::block::entities::daylight_detector::DaylightDetectorBlockEntity as InternalDaylightDetectorBlockEntity;
use crate::block::entities::decorated_pot::DecoratedPotBlockEntity as InternalDecoratedPotBlockEntity;
use crate::block::entities::dispenser::DispenserBlockEntity as InternalDispenserBlockEntity;
use crate::block::entities::dropper::DropperBlockEntity as InternalDropperBlockEntity;
use crate::block::entities::enchanting_table::EnchantingTableBlockEntity as InternalEnchantingTableBlockEntity;
use crate::block::entities::end_gateway::EndGatewayBlockEntity as InternalEndGatewayBlockEntity;
use crate::block::entities::end_portal::EndPortalBlockEntity as InternalEndPortalBlockEntity;
use crate::block::entities::ender_chest::EnderChestBlockEntity as InternalEnderChestBlockEntity;
use crate::block::entities::furnace::FurnaceBlockEntity as InternalFurnaceBlockEntity;
use crate::block::entities::hanging_sign::HangingSignBlockEntity as InternalHangingSignBlockEntity;
use crate::block::entities::hopper::HopperBlockEntity as InternalHopperBlockEntity;
use crate::block::entities::jigsaw_block::JigsawBlockEntity as InternalJigsawBlockEntity;
use crate::block::entities::jukebox::JukeboxBlockEntity as InternalJukeboxBlockEntity;
use crate::block::entities::lectern::LecternBlockEntity as InternalLecternBlockEntity;
use crate::block::entities::map::MapBlockEntity as InternalMapBlockEntity;
use crate::block::entities::mob_spawner::MobSpawnerBlockEntity as InternalMobSpawnerBlockEntity;
use crate::block::entities::piston::PistonBlockEntity as InternalPistonBlockEntity;
use crate::block::entities::potent_sulfur::PotentSulfurBlockEntity as InternalPotentSulfurBlockEntity;
use crate::block::entities::sculk_catalyst::SculkCatalystBlockEntity as InternalSculkCatalystBlockEntity;
use crate::block::entities::sculk_sensor::SculkSensorBlockEntity as InternalSculkSensorBlockEntity;
use crate::block::entities::sculk_shrieker::SculkShriekerBlockEntity as InternalSculkShriekerBlockEntity;
use crate::block::entities::shelf::ShelfBlockEntity as InternalShelfBlockEntity;
use crate::block::entities::shulker_box::ShulkerBoxBlockEntity as InternalShulkerBoxBlockEntity;
use crate::block::entities::sign::SignBlockEntity as InternalSignBlockEntity;
use crate::block::entities::skull::SkullBlockEntity as InternalSkullBlockEntity;
use crate::block::entities::smoker::SmokerBlockEntity as InternalSmokerBlockEntity;
use crate::block::entities::structure_block::StructureBlockBlockEntity as InternalStructureBlockBlockEntity;
use crate::block::entities::test_block::TestBlockBlockEntity as InternalTestBlockBlockEntity;
use crate::block::entities::test_instance_block::TestInstanceBlockBlockEntity as InternalTestInstanceBlockBlockEntity;
use crate::block::entities::trapped_chest::TrappedChestBlockEntity as InternalTrappedChestBlockEntity;
use crate::block::entities::trial_spawner::TrialSpawnerBlockEntity as InternalTrialSpawnerBlockEntity;
use crate::block::entities::vault::VaultBlockEntity as InternalVaultBlockEntity;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::common::Position as WitPosition;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::game_rules::{
    GameRule as WitGameRule, GameRuleValue as WitGameRuleValue,
};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::world::{
    Block as WitBlock, BlockDirection as WitBlockDirection, BlockEntity, BlockEntityType,
    BlockFlags as WitBlockFlags, BlockPos as WitBlockPos, BlockState as WitBlockState,
    BlockStateInfo as WitBlockStateInfo, BoundingBox as WitBoundingBox, Chunk as WitChunk,
    ChunkSnapshot as WitChunkSnapshot, Flammable as WitFlammable,
    NoteblockInstrument as WitNoteblockInstrument, PistonBehavior as WitPistonBehavior,
    RayTraceBlockResult as WitRayTraceBlockResult, RayTraceEntityResult as WitRayTraceEntityResult,
    TeleportFlags as WitTeleportFlags, WorldBorder as WitWorldBorder,
    WorldSpawnLocation as WitWorldSpawnLocation,
};
use crate::plugin::loader::wasm::wasm_host::{
    state::{
        ChunkResource, ChunkSnapshot as HostChunkSnapshotData, ChunkSnapshotResource,
        PluginHostState, TextComponentResource, WorldBorderResource, WorldResource,
    },
    wit::v0_1::papokin::{self, plugin::world::World},
};
use crate::world::explosion::ExplosionInteraction;
use papokin_data::game_rules::{GameRule, GameRuleValue};

pub(crate) fn from_wit_game_rule(rule: WitGameRule) -> GameRule {
    // SAFETY: WIT GameRule 与 papokin_data::game_rules::GameRule 的变体顺序完全一致
    unsafe { std::mem::transmute::<u8, GameRule>(rule as u8) }
}

pub(crate) fn to_wit_game_rule_value(value: &GameRuleValue<i64, bool>) -> WitGameRuleValue {
    match *value {
        GameRuleValue::Int(v) => {
            WitGameRuleValue::Int(v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
        }
        GameRuleValue::Bool(v) => WitGameRuleValue::Bool(v),
    }
}

pub(crate) const fn from_wit_game_rule_value(value: WitGameRuleValue) -> GameRuleValue<i64, bool> {
    match value {
        WitGameRuleValue::Int(v) => GameRuleValue::Int(v as i64),
        WitGameRuleValue::Bool(v) => GameRuleValue::Bool(v),
    }
}

fn from_wit_block_flags(flags: WitBlockFlags) -> BlockFlags {
    let mut internal = BlockFlags::empty();
    if flags.contains(WitBlockFlags::NOTIFY_NEIGHBORS) {
        internal |= BlockFlags::NOTIFY_NEIGHBORS;
    }
    if flags.contains(WitBlockFlags::NOTIFY_LISTENERS) {
        internal |= BlockFlags::NOTIFY_LISTENERS;
    }
    if flags.contains(WitBlockFlags::FORCE_STATE) {
        internal |= BlockFlags::FORCE_STATE;
    }
    if flags.contains(WitBlockFlags::SKIP_DROPS) {
        internal |= BlockFlags::SKIP_DROPS;
    }
    if flags.contains(WitBlockFlags::MOVED) {
        internal |= BlockFlags::MOVED;
    }
    if flags.contains(WitBlockFlags::SKIP_REDSTONE_WIRE_STATE_REPLACEMENT) {
        internal |= BlockFlags::SKIP_REDSTONE_WIRE_STATE_REPLACEMENT;
    }
    if flags.contains(WitBlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK) {
        internal |= BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK;
    }
    if flags.contains(WitBlockFlags::SKIP_BLOCK_ADDED_CALLBACK) {
        internal |= BlockFlags::SKIP_BLOCK_ADDED_CALLBACK;
    }
    internal
}

/// 将 WIT 传送标志转换为协议位置标志。WIT 标志
/// 顺序与 `PositionFlag` 位域顺序一致。
pub(crate) fn from_wit_teleport_flags(flags: WitTeleportFlags) -> Vec<PositionFlag> {
    let mut relatives = Vec::new();
    if flags.contains(WitTeleportFlags::X) {
        relatives.push(PositionFlag::X);
    }
    if flags.contains(WitTeleportFlags::Y) {
        relatives.push(PositionFlag::Y);
    }
    if flags.contains(WitTeleportFlags::Z) {
        relatives.push(PositionFlag::Z);
    }
    if flags.contains(WitTeleportFlags::Y_ROT) {
        relatives.push(PositionFlag::YRot);
    }
    if flags.contains(WitTeleportFlags::X_ROT) {
        relatives.push(PositionFlag::XRot);
    }
    if flags.contains(WitTeleportFlags::DELTA_X) {
        relatives.push(PositionFlag::DeltaX);
    }
    if flags.contains(WitTeleportFlags::DELTA_Y) {
        relatives.push(PositionFlag::DeltaY);
    }
    if flags.contains(WitTeleportFlags::DELTA_Z) {
        relatives.push(PositionFlag::DeltaZ);
    }
    if flags.contains(WitTeleportFlags::ROTATE_DELTA) {
        relatives.push(PositionFlag::RotateDelta);
    }
    relatives
}

fn world_and_plugin(
    state: &PluginHostState,
    world: &Resource<World>,
) -> wasmtime::Result<(
    Arc<crate::world::World>,
    Arc<crate::plugin::loader::wasm::wasm_host::WasmPlugin>,
)> {
    let world = state.get_world_res(world)?.provider.clone();
    let plugin = state
        .plugin
        .as_ref()
        .and_then(std::sync::Weak::upgrade)
        .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
    Ok((world, plugin))
}

async fn set_block_state_with_store(
    mut host: Access<'_, PluginHostState, HasSelf<PluginHostState>>,
    world: Resource<World>,
    pos: WitBlockPos,
    state: u16,
    update_flags: WitBlockFlags,
) -> wasmtime::Result<()> {
    let Some(state_id) = BlockStateId::new(state) else {
        return Err(wasmtime::Error::msg("无效的 BlockStateId"));
    };
    let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
    let internal_flags = from_wit_block_flags(update_flags);
    let (world, plugin) = world_and_plugin(host.get(), &world)?;

    plugin
        .store
        .pump_blocking(&mut host, move || {
            world.set_block_state(&internal_pos, state_id, internal_flags);
        })
        .await
}

pub(crate) const fn to_wasm_block_direction(dir: InternalBlockDirection) -> WitBlockDirection {
    match dir {
        InternalBlockDirection::Down => WitBlockDirection::Down,
        InternalBlockDirection::Up => WitBlockDirection::Up,
        InternalBlockDirection::North => WitBlockDirection::North,
        InternalBlockDirection::South => WitBlockDirection::South,
        InternalBlockDirection::West => WitBlockDirection::West,
        InternalBlockDirection::East => WitBlockDirection::East,
    }
}

pub(crate) const fn to_wit_noteblock_instrument(
    instr: InternalNoteblockInstrument,
) -> WitNoteblockInstrument {
    match instr {
        InternalNoteblockInstrument::Harp => WitNoteblockInstrument::Harp,
        InternalNoteblockInstrument::Basedrum => WitNoteblockInstrument::Basedrum,
        InternalNoteblockInstrument::Snare => WitNoteblockInstrument::Snare,
        InternalNoteblockInstrument::Hat => WitNoteblockInstrument::Hat,
        InternalNoteblockInstrument::Bass => WitNoteblockInstrument::Bass,
        InternalNoteblockInstrument::Flute => WitNoteblockInstrument::Flute,
        InternalNoteblockInstrument::Bell => WitNoteblockInstrument::Bell,
        InternalNoteblockInstrument::Guitar => WitNoteblockInstrument::Guitar,
        InternalNoteblockInstrument::Chime => WitNoteblockInstrument::Chime,
        InternalNoteblockInstrument::Xylophone => WitNoteblockInstrument::Xylophone,
        InternalNoteblockInstrument::IronXylophone => WitNoteblockInstrument::IronXylophone,
        InternalNoteblockInstrument::CowBell => WitNoteblockInstrument::CowBell,
        InternalNoteblockInstrument::Didgeridoo => WitNoteblockInstrument::Didgeridoo,
        InternalNoteblockInstrument::Bit => WitNoteblockInstrument::Bit,
        InternalNoteblockInstrument::Banjo => WitNoteblockInstrument::Banjo,
        InternalNoteblockInstrument::Pling => WitNoteblockInstrument::Pling,
        InternalNoteblockInstrument::Trumpet => WitNoteblockInstrument::Trumpet,
        InternalNoteblockInstrument::TrumpetExposed => WitNoteblockInstrument::TrumpetExposed,
        InternalNoteblockInstrument::TrumpetOxidized => WitNoteblockInstrument::TrumpetOxidized,
        InternalNoteblockInstrument::TrumpetWeathered => WitNoteblockInstrument::TrumpetWeathered,
        InternalNoteblockInstrument::Zombie => WitNoteblockInstrument::Zombie,
        InternalNoteblockInstrument::Skeleton => WitNoteblockInstrument::Skeleton,
        InternalNoteblockInstrument::Creeper => WitNoteblockInstrument::Creeper,
        InternalNoteblockInstrument::Dragon => WitNoteblockInstrument::Dragon,
        InternalNoteblockInstrument::WitherSkeleton => WitNoteblockInstrument::WitherSkeleton,
        InternalNoteblockInstrument::Piglin => WitNoteblockInstrument::Piglin,
        InternalNoteblockInstrument::CustomHead => WitNoteblockInstrument::CustomHead,
    }
}

pub(crate) const fn to_wit_bounding_box(
    bb: papokin_util::math::boundingbox::BoundingBox,
) -> WitBoundingBox {
    WitBoundingBox {
        min: (bb.min.x, bb.min.y, bb.min.z),
        max: (bb.max.x, bb.max.y, bb.max.z),
    }
}

pub(crate) fn to_wit_block(block: &papokin_data::Block) -> WitBlock {
    WitBlock {
        id: block.id.as_u16(),
        name: block.name.to_string(),
        hardness: block.hardness,
        blast_resistance: block.blast_resistance,
        map_color: block.map_color,
        slipperiness: block.slipperiness,
        velocity_multiplier: block.velocity_multiplier,
        jump_velocity_multiplier: block.jump_velocity_multiplier,
        item_id: block.item_id,
        default_state_id: block.default_state.id.as_u16(),
        state_ids: block.states.iter().map(|s| s.id.as_u16()).collect(),
        is_solid: block.is_solid(),
        is_air: block.is_air(),
        is_flammable: block.flammable.is_some(),
        flammable: block.flammable.as_ref().map(|f| WitFlammable {
            spread_chance: f.spread_chance,
            burn_chance: f.burn_chance,
        }),
    }
}

pub(crate) fn to_wit_block_state(
    state: &papokin_data::BlockState,
    pos: Option<&BlockPos>,
) -> WitBlockState {
    let dummy_pos = BlockPos::new(0, 0, 0);
    let internal_pos = pos.unwrap_or(&dummy_pos);
    let block = papokin_data::Block::from_state_id(state.id);
    let properties = block
        .properties(state.id)
        .map(|p| {
            p.to_props()
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .unwrap_or_default();

    WitBlockState {
        id: state.id.as_u16(),
        block_id: block.id.as_u16(),
        block_name: block.name.to_string(),
        luminance: state.luminance,
        opacity: state.opacity,
        hardness: state.hardness,
        is_air: state.is_air(),
        is_liquid: state.is_liquid(),
        is_solid: state.is_solid(),
        is_full_cube: state.is_full_cube(),
        has_random_ticks: state.has_random_ticks(),
        piston_behavior: match state.piston_behavior {
            PistonBehavior::Normal => WitPistonBehavior::Normal,
            PistonBehavior::Destroy => WitPistonBehavior::Destroy,
            PistonBehavior::Block => WitPistonBehavior::Block,
            PistonBehavior::Ignore => WitPistonBehavior::Ignore,
            PistonBehavior::PushOnly => WitPistonBehavior::PushOnly,
        },
        burnable: state.burnable(),
        tool_required: state.tool_required(),
        sided_transparency: state.sided_transparency(),
        replaceable: state.replaceable(),
        is_solid_block: state.is_solid_block(),
        block_entity_type: state.block_entity_type,
        instrument: to_wit_noteblock_instrument(state.instrument),
        collision_shapes: state
            .get_block_collision_shapes_at(internal_pos)
            .map(to_wit_bounding_box)
            .collect(),
        outline_shapes: state
            .get_block_outline_shapes_at(internal_pos)
            .map(to_wit_bounding_box)
            .collect(),
        down_side_solid: state.is_side_solid(InternalBlockDirection::Down),
        up_side_solid: state.is_side_solid(InternalBlockDirection::Up),
        north_side_solid: state.is_side_solid(InternalBlockDirection::North),
        south_side_solid: state.is_side_solid(InternalBlockDirection::South),
        west_side_solid: state.is_side_solid(InternalBlockDirection::West),
        east_side_solid: state.is_side_solid(InternalBlockDirection::East),
        down_center_solid: state.is_center_solid(InternalBlockDirection::Down),
        up_center_solid: state.is_center_solid(InternalBlockDirection::Up),
        map_color: block.map_color,
        properties,
    }
}

// --- 陷阱辅助 ---
impl PluginHostState {
    pub(crate) fn get_world_res(&self, res: &Resource<World>) -> wasmtime::Result<&WorldResource> {
        self.resource_table
            .get::<WorldResource>(&Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)
    }

    fn get_chunk_res(&self, res: &Resource<WitChunk>) -> wasmtime::Result<&ChunkResource> {
        self.resource_table
            .get::<ChunkResource>(&Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)
    }

    fn get_world_border_res(
        &self,
        res: &Resource<WitWorldBorder>,
    ) -> wasmtime::Result<&WorldBorderResource> {
        self.resource_table
            .get::<WorldBorderResource>(&Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)
    }

    pub(crate) fn get_text_provider(
        &self,
        res: &Resource<papokin::plugin::text::TextComponent>,
    ) -> wasmtime::Result<papokin_util::text::TextComponent> {
        Ok(self
            .resource_table
            .get::<TextComponentResource>(&Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)?
            .provider
            .clone())
    }

    fn get_wit_biome(
        biome: &papokin_data::biome::Biome,
    ) -> wasmtime::Result<papokin::plugin::biomes::Biome> {
        to_wit_biome(biome)
    }

    fn get_wit_block_entity(
        &mut self,
        block_entity: Arc<dyn crate::block::entities::BlockEntity>,
    ) -> wasmtime::Result<Option<BlockEntityType>> {
        let be = block_entity;
        macro_rules! match_be {
            ($( $internal_type:ty => $variant:ident ),* $(,)?) => {
                $(
                    if be.as_any().downcast_ref::<$internal_type>().is_some() {
                        let res: Resource<BlockEntity> = self.add_block_entity(be)?;
                        return Ok(Some(BlockEntityType::$variant(Resource::new_own(res.rep()))));
                    }
                )*
            };
        }

        match_be! {
            InternalCommandBlockEntity => CommandBlockEntity,
            InternalSignBlockEntity => SignBlockEntity,
            InternalHangingSignBlockEntity => HangingSignBlockEntity,
            InternalJukeboxBlockEntity => JukeboxBlockEntity,
            InternalChestBlockEntity => ChestBlockEntity,
            InternalTrappedChestBlockEntity => TrappedChestBlockEntity,
            InternalMobSpawnerBlockEntity => MobSpawnerBlockEntity,
            InternalMapBlockEntity => MapBlockEntity,
            InternalBannerBlockEntity => BannerBlockEntity,
            InternalBarrelBlockEntity => BarrelBlockEntity,
            InternalBeaconBlockEntity => BeaconBlockEntity,
            InternalBedBlockEntity => BedBlockEntity,
            InternalBeehiveBlockEntity => BeehiveBlockEntity,
            InternalBellBlockEntity => BellBlockEntity,
            InternalBlastingFurnaceBlockEntity => BlastingFurnaceBlockEntity,
            InternalBrewingStandBlockEntity => BrewingStandBlockEntity,
            InternalBrushableBlockBlockEntity => BrushableBlockBlockEntity,
            InternalCalibratedSculkSensorBlockEntity => CalibratedSculkSensorBlockEntity,
            InternalCampfireBlockEntity => CampfireBlockEntity,
            InternalChiseledBookshelfBlockEntity => ChiseledBookshelfBlockEntity,
            InternalComparatorBlockEntity => ComparatorBlockEntity,
            InternalConduitBlockEntity => ConduitBlockEntity,
            InternalCopperGolemStatueBlockEntity => CopperGolemStatueBlockEntity,
            InternalCrafterBlockEntity => CrafterBlockEntity,
            InternalCreakingHeartBlockEntity => CreakingHeartBlockEntity,
            InternalDaylightDetectorBlockEntity => DaylightDetectorBlockEntity,
            InternalDecoratedPotBlockEntity => DecoratedPotBlockEntity,
            InternalDispenserBlockEntity => DispenserBlockEntity,
            InternalDropperBlockEntity => DropperBlockEntity,
            InternalEnchantingTableBlockEntity => EnchantingTableBlockEntity,
            InternalEndGatewayBlockEntity => EndGatewayBlockEntity,
            InternalEndPortalBlockEntity => EndPortalBlockEntity,
            InternalEnderChestBlockEntity => EnderChestBlockEntity,
            InternalFurnaceBlockEntity => FurnaceBlockEntity,
            InternalHopperBlockEntity => HopperBlockEntity,
            InternalJigsawBlockEntity => JigsawBlockEntity,
            InternalLecternBlockEntity => LecternBlockEntity,
            InternalPistonBlockEntity => PistonBlockEntity,
            InternalPotentSulfurBlockEntity => PotentSulfurBlockEntity,
            InternalSculkCatalystBlockEntity => SculkCatalystBlockEntity,
            InternalSculkSensorBlockEntity => SculkSensorBlockEntity,
            InternalSculkShriekerBlockEntity => SculkShriekerBlockEntity,
            InternalShelfBlockEntity => ShelfBlockEntity,
            InternalShulkerBoxBlockEntity => ShulkerBoxBlockEntity,
            InternalSkullBlockEntity => SkullBlockEntity,
            InternalSmokerBlockEntity => SmokerBlockEntity,
            InternalStructureBlockBlockEntity => StructureBlockBlockEntity,
            InternalTestBlockBlockEntity => TestBlockBlockEntity,
            InternalTestInstanceBlockBlockEntity => TestInstanceBlockBlockEntity,
            InternalTrialSpawnerBlockEntity => TrialSpawnerBlockEntity,
            InternalVaultBlockEntity => VaultBlockEntity,
        }

        Ok(None)
    }
}

impl papokin::plugin::world::Host for PluginHostState {
    async fn resolve_block_state(
        &mut self,
        name: String,
        properties: Vec<(String, String)>,
    ) -> wasmtime::Result<Option<u16>> {
        let Some(block) = papokin_data::Block::from_name(&name) else {
            return Ok(None);
        };

        if properties.is_empty() {
            return Ok(Some(block.default_state.id.as_u16()));
        }

        let props: Vec<(&str, &str)> = properties
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        // from_properties/from_value 遇到未知属性值时会 panic，
        // 因此要捕获 panic，避免加载其他 MC 版本的结构文件时崩溃。
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let block_props = block.from_properties(&props);
            block_props.to_state_id(block).as_u16()
        }));
        Ok(result.ok())
    }

    async fn block_state_to_info(
        &mut self,
        state_id: u16,
    ) -> wasmtime::Result<Option<WitBlockStateInfo>> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let bsid = BlockStateId::new_or_air(state_id);
            let block = papokin_data::Block::from_state_id(bsid);
            let name = format!("minecraft:{}", block.name);
            let properties = block
                .properties(bsid)
                .map(|p| {
                    p.to_props()
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            WitBlockStateInfo { name, properties }
        }));
        Ok(result.ok())
    }

    async fn get_block_by_id(&mut self, id: u16) -> wasmtime::Result<Option<WitBlock>> {
        let block_id = BlockId::new(id);
        Ok(block_id.map(|id| to_wit_block(papokin_data::Block::from_id(id))))
    }

    async fn get_block_by_name(&mut self, name: String) -> wasmtime::Result<Option<WitBlock>> {
        Ok(papokin_data::Block::from_name(&name).map(to_wit_block))
    }

    async fn get_all_blocks(&mut self) -> wasmtime::Result<Vec<WitBlock>> {
        let mut blocks = Vec::with_capacity(BlockId::COUNT as usize);
        for raw_id in 0..BlockId::COUNT {
            if let Some(id) = BlockId::new(raw_id) {
                blocks.push(to_wit_block(papokin_data::Block::from_id(id)));
            }
        }
        Ok(blocks)
    }

    async fn get_all_block_names(&mut self) -> wasmtime::Result<Vec<String>> {
        let mut names = Vec::with_capacity(BlockId::COUNT as usize);
        for raw_id in 0..BlockId::COUNT {
            if let Some(id) = BlockId::new(raw_id) {
                names.push(papokin_data::Block::from_id(id).name.to_string());
            }
        }
        Ok(names)
    }

    async fn get_block_count(&mut self) -> wasmtime::Result<u32> {
        Ok(BlockId::COUNT as u32)
    }

    async fn get_block_state_count(&mut self) -> wasmtime::Result<u32> {
        Ok(BlockStateId::COUNT as u32)
    }

    async fn get_states_for_block(
        &mut self,
        block: WitBlock,
    ) -> wasmtime::Result<Vec<WitBlockState>> {
        let block_id = BlockId::new_or_air(block.id);
        let block_ref = papokin_data::Block::from_id(block_id);
        Ok(block_ref
            .states
            .iter()
            .map(|s| to_wit_block_state(s, None))
            .collect())
    }

    async fn get_states_for_block_id(
        &mut self,
        block_id: u16,
    ) -> wasmtime::Result<Vec<WitBlockState>> {
        let Some(id) = BlockId::new(block_id) else {
            return Ok(Vec::new());
        };
        let block_ref = papokin_data::Block::from_id(id);
        Ok(block_ref
            .states
            .iter()
            .map(|s| to_wit_block_state(s, None))
            .collect())
    }

    async fn get_state_ids_for_block_id(&mut self, block_id: u16) -> wasmtime::Result<Vec<u16>> {
        let Some(id) = BlockId::new(block_id) else {
            return Ok(Vec::new());
        };
        let block_ref = papokin_data::Block::from_id(id);
        Ok(block_ref.states.iter().map(|s| s.id.as_u16()).collect())
    }

    async fn get_block_properties(
        &mut self,
        state_id: u16,
    ) -> wasmtime::Result<Vec<(String, String)>> {
        let bsid = BlockStateId::new_or_air(state_id);
        let block = papokin_data::Block::from_state_id(bsid);
        let props = block
            .properties(bsid)
            .map(|p| {
                p.to_props()
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        Ok(props)
    }

    async fn get_block_from_state_id(
        &mut self,
        state_id: u16,
    ) -> wasmtime::Result<Option<WitBlock>> {
        let bsid = BlockStateId::new(state_id);
        Ok(bsid.map(|id| to_wit_block(papokin_data::Block::from_state_id(id))))
    }

    async fn get_block_from_state(&mut self, state: WitBlockState) -> wasmtime::Result<WitBlock> {
        let bsid = BlockStateId::new_or_air(state.id);
        Ok(to_wit_block(papokin_data::Block::from_state_id(bsid)))
    }

    async fn get_default_state_from_block(
        &mut self,
        block: WitBlock,
    ) -> wasmtime::Result<WitBlockState> {
        let block_id = BlockId::new_or_air(block.id);
        let block_ref = papokin_data::Block::from_id(block_id);
        Ok(to_wit_block_state(block_ref.default_state, None))
    }

    async fn get_default_state_from_block_id(
        &mut self,
        block_id: u16,
    ) -> wasmtime::Result<Option<WitBlockState>> {
        let block_id = BlockId::new(block_id);
        Ok(block_id.map(|id| {
            let block = papokin_data::Block::from_id(id);
            to_wit_block_state(block.default_state, None)
        }))
    }

    async fn get_block_state_by_id(
        &mut self,
        state_id: u16,
    ) -> wasmtime::Result<Option<WitBlockState>> {
        let bsid = BlockStateId::new(state_id);
        Ok(bsid.map(|id| {
            let state = papokin_data::BlockState::from_id(id);
            to_wit_block_state(state, None)
        }))
    }
}
impl papokin::plugin::particles::Host for PluginHostState {}
impl papokin::plugin::sounds::Host for PluginHostState {}

impl papokin::plugin::world::HostWorld for PluginHostState {
    async fn get_id(&mut self, world: Resource<World>) -> wasmtime::Result<String> {
        Ok(self
            .get_world_res(&world)?
            .provider
            .get_world_name()
            .to_string())
    }

    async fn get_border(
        &mut self,
        world: Resource<World>,
    ) -> wasmtime::Result<Resource<WitWorldBorder>> {
        self.get_world_border(world).await
    }

    async fn get_world_border(
        &mut self,
        world: Resource<World>,
    ) -> wasmtime::Result<Resource<WitWorldBorder>> {
        let world_res = self.get_world_res(&world)?;
        self.add_world_border(world_res.provider.clone())
    }

    async fn get_spawn_location(
        &mut self,
        world: Resource<World>,
    ) -> wasmtime::Result<WitWorldSpawnLocation> {
        let (pos, yaw, pitch) = self.get_world_res(&world)?.provider.get_spawn_location();
        Ok(WitWorldSpawnLocation {
            pos: WitBlockPos {
                x: pos.0.x,
                y: pos.0.y,
                z: pos.0.z,
            },
            yaw,
            pitch,
        })
    }

    async fn get_chunk(
        &mut self,
        world: Resource<World>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<Option<Resource<WitChunk>>> {
        let world_res = self.get_world_res(&world)?;
        let world_provider = world_res.provider.clone();
        let pos = papokin_util::math::vector2::Vector2::new(x, z);

        let chunk = world_provider
            .level
            .loaded_chunks
            .get(&pos)
            .map(|c| c.value().clone());
        if let Some(chunk) = chunk {
            let res = self.add_chunk(world_provider, std::sync::Arc::downgrade(&chunk))?;
            Ok(Some(res))
        } else {
            Ok(None)
        }
    }

    async fn is_chunk_loaded(
        &mut self,
        world: Resource<World>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<bool> {
        let world_res = self.get_world_res(&world)?;
        let pos = papokin_util::math::vector2::Vector2::new(x, z);
        Ok(world_res.provider.level.is_chunk_loaded(&pos))
    }

    async fn get_chunk_snapshot(
        &mut self,
        world: Resource<World>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<Option<Resource<WitChunkSnapshot>>> {
        let world_res = self.get_world_res(&world)?;
        let pos = papokin_util::math::vector2::Vector2::new(x, z);

        let Some(chunk) = world_res
            .provider
            .level
            .loaded_chunks
            .get(&pos)
            .map(|c| c.value().clone())
        else {
            return Ok(None);
        };

        // 读取即复制：快照拥有自己的方块/生物群系数据，因此它保持
        // 在存活区块变化或卸载后仍然有效。
        let section = &chunk.section;
        let blocks = section
            .dump_blocks()
            .into_iter()
            .map(papokin_data::BlockStateId::as_u16)
            .collect();
        let biomes = section.dump_biomes();
        let snapshot = HostChunkSnapshotData {
            x: chunk.x,
            z: chunk.z,
            min_y: section.min_y,
            section_count: section.count as u32,
            blocks,
            biomes,
        };
        let res = self.add_chunk_snapshot(snapshot)?;
        Ok(Some(res))
    }

    async fn set_chunk_forced(
        &mut self,
        world: Resource<World>,
        x: i32,
        z: i32,
        forced: bool,
    ) -> wasmtime::Result<()> {
        let world_res = self.get_world_res(&world)?;
        let pos = papokin_util::math::vector2::Vector2::new(x, z);
        world_res.provider.set_chunk_forced(pos, forced);
        Ok(())
    }

    async fn is_chunk_forced(
        &mut self,
        world: Resource<World>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<bool> {
        let world_res = self.get_world_res(&world)?;
        let pos = papokin_util::math::vector2::Vector2::new(x, z);
        Ok(world_res.provider.is_chunk_forced(&pos))
    }

    async fn load_chunk(
        &mut self,
        world: Resource<World>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<bool> {
        let world_res = self.get_world_res(&world)?;
        let pos = papokin_util::math::vector2::Vector2::new(x, z);
        let level = &world_res.provider.level;

        // 每次调用持有一个插件票证（由 `unload-chunk` 释放）；
        // 票据驱动异步的生成/加载管线。
        let mut chunk_loading = level
            .chunk_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        chunk_loading.add_ticket(
            pos,
            papokin_world::chunk_system::ChunkLoading::FULL_CHUNK_LEVEL,
        );
        chunk_loading.send_change();
        drop(chunk_loading);
        Ok(level.is_chunk_loaded(&pos))
    }

    async fn unload_chunk(
        &mut self,
        world: Resource<World>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<()> {
        let world_res = self.get_world_res(&world)?;
        let pos = papokin_util::math::vector2::Vector2::new(x, z);
        let level = &world_res.provider.level;

        // 释放一个插件票据；未持有时为空操作。强制
        // 标记是独立的标志，此处不会清除。
        let mut chunk_loading = level
            .chunk_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        chunk_loading.remove_ticket(
            pos,
            papokin_world::chunk_system::ChunkLoading::FULL_CHUNK_LEVEL,
        );
        chunk_loading.send_change();
        Ok(())
    }

    async fn get_block_state_id(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<u16> {
        let world_ref = self.get_world_res(&world)?;
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        Ok(world_ref
            .provider
            .get_block_state_id(&internal_pos)
            .as_u16())
    }

    async fn get_block_state(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<WitBlockState> {
        let world_ref = self.get_world_res(&world)?;
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        let state = world_ref.provider.get_block_state(&internal_pos);
        Ok(to_wit_block_state(state, Some(&internal_pos)))
    }

    async fn get_block(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<WitBlock> {
        let world_ref = self.get_world_res(&world)?;
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        let state = world_ref.provider.get_block_state(&internal_pos);
        let block = papokin_data::Block::from_state_id(state.id);
        Ok(to_wit_block(block))
    }

    async fn get_block_id(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<u16> {
        let world_ref = self.get_world_res(&world)?;
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        let state = world_ref.provider.get_block_state(&internal_pos);
        Ok(papokin_data::BlockId::from_state_id(state.id).as_u16())
    }

    async fn get_time_of_day(&mut self, world: Resource<World>) -> wasmtime::Result<u64> {
        Ok(self.get_world_res(&world)?.provider.get_time_of_day() as u64)
    }

    async fn set_time_of_day(&mut self, world: Resource<World>, time: u64) -> wasmtime::Result<()> {
        self.get_world_res(&world)?
            .provider
            .set_time_of_day(time as i64);
        Ok(())
    }

    async fn get_world_age(&mut self, world: Resource<World>) -> wasmtime::Result<u64> {
        Ok(self.get_world_res(&world)?.provider.get_world_age() as u64)
    }

    async fn get_dimension(&mut self, world: Resource<World>) -> wasmtime::Result<String> {
        Ok(self
            .get_world_res(&world)?
            .provider
            .dimension
            .minecraft_name
            .to_string())
    }

    async fn get_top_block_y(
        &mut self,
        world: Resource<World>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<i32> {
        Ok(self
            .get_world_res(&world)?
            .provider
            .get_top_block(papokin_util::math::vector2::Vector2::new(x, z)))
    }

    async fn get_motion_blocking_height(
        &mut self,
        world: Resource<World>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<i32> {
        Ok(self.get_world_res(&world)?.provider.get_heightmap_height(
            ChunkHeightmapType::MotionBlocking,
            x,
            z,
        ))
    }

    async fn is_raining(&mut self, world: Resource<World>) -> wasmtime::Result<bool> {
        Ok(self.get_world_res(&world)?.provider.is_raining())
    }

    async fn is_thundering(&mut self, world: Resource<World>) -> wasmtime::Result<bool> {
        Ok(self.get_world_res(&world)?.provider.is_thundering())
    }

    async fn broadcast_system_message(
        &mut self,
        world: Resource<World>,
        message: Resource<papokin::plugin::text::TextComponent>,
        overlay: bool,
    ) -> wasmtime::Result<()> {
        let msg = self.get_text_provider(&message)?;
        self.get_world_res(&world)?
            .provider
            .broadcast_system_message(&msg, overlay);
        Ok(())
    }

    async fn get_scoreboard(
        &mut self,
        world: Resource<World>,
    ) -> wasmtime::Result<Resource<papokin::plugin::scoreboard::Scoreboard>> {
        let world_provider = self.get_world_res(&world)?.provider.clone();
        self.add_scoreboard(
            crate::plugin::loader::wasm::wasm_host::state::ScoreboardProvider::World(
                world_provider,
            ),
        )
    }

    async fn play_sound(
        &mut self,
        world: Resource<World>,
        sound: papokin::plugin::sounds::Sound,
        category: papokin::plugin::sounds::SoundCategory,
        pos: papokin::plugin::common::Position,
        volume: f32,
        pitch: f32,
    ) -> wasmtime::Result<()> {
        let world_ref = self.get_world_res(&world)?;
        let sound_name = format!("{sound:?}").to_lowercase().replace('_', ".");
        let sound_data = papokin_data::sound::Sound::from_name(&sound_name)
            .ok_or_else(|| wasmtime::Error::msg(format!("未知的声音：{sound_name}")))?;

        let internal_category = from_wit_sound_category(category);

        world_ref.provider.play_sound_raw(
            sound_data as u16,
            internal_category,
            &papokin_util::math::vector3::Vector3::new(pos.0, pos.1, pos.2),
            volume,
            pitch,
        );
        Ok(())
    }

    async fn play_custom_sound(
        &mut self,
        world: Resource<World>,
        sound_name: String,
        category: papokin::plugin::sounds::SoundCategory,
        pos: papokin::plugin::common::Position,
        volume: f32,
        pitch: f32,
    ) -> wasmtime::Result<()> {
        let world_ref = self.get_world_res(&world)?;
        let internal_category = from_wit_sound_category(category);
        world_ref.provider.play_custom_sound(
            &sound_name,
            internal_category,
            &papokin_util::math::vector3::Vector3::new(pos.0, pos.1, pos.2),
            volume,
            pitch,
        );
        Ok(())
    }

    async fn spawn_particle(
        &mut self,
        world: Resource<World>,
        particle: papokin::plugin::particles::Particle,
        pos: papokin::plugin::common::Position,
        offset: papokin::plugin::common::Position,
        max_speed: f32,
        count: i32,
    ) -> wasmtime::Result<()> {
        let world_ref = self.get_world_res(&world)?;
        let particle_data = papokin_data::particle::Particle::from_id(particle as u16)
            .ok_or_else(|| wasmtime::Error::msg(format!("未知的粒子 ID：{}", particle as u16)))?;

        world_ref.provider.spawn_particle(
            papokin_util::math::vector3::Vector3::new(pos.0, pos.1, pos.2),
            papokin_util::math::vector3::Vector3::new(
                offset.0 as f32,
                offset.1 as f32,
                offset.2 as f32,
            ),
            max_speed,
            count,
            particle_data,
        );
        Ok(())
    }

    async fn get_sea_level(&mut self, world: Resource<World>) -> wasmtime::Result<i32> {
        Ok(self.get_world_res(&world)?.provider.sea_level)
    }

    async fn get_min_y(&mut self, world: Resource<World>) -> wasmtime::Result<i32> {
        Ok(self.get_world_res(&world)?.provider.min_y)
    }

    async fn get_sky_light(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<u8> {
        let world_ref = self.get_world_res(&world)?;
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        Ok(world_ref.provider.get_sky_light_level(&internal_pos))
    }

    async fn set_sky_light(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
        level: u8,
    ) -> wasmtime::Result<()> {
        let world_ref = self.get_world_res(&world)?;
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        world_ref.provider.set_sky_light_level(&internal_pos, level);
        Ok(())
    }

    async fn get_block_light(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<u8> {
        let world_ref = self.get_world_res(&world)?;
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        Ok(world_ref
            .provider
            .get_block_light_level(&internal_pos)
            .unwrap_or(0))
    }

    async fn set_block_light(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
        level: u8,
    ) -> wasmtime::Result<()> {
        let world_ref = self.get_world_res(&world)?;
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        world_ref
            .provider
            .set_block_light_level(&internal_pos, level);
        Ok(())
    }

    async fn get_biome(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<papokin::plugin::biomes::Biome> {
        let world_ref = self.get_world_res(&world)?;
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        let biome = world_ref.provider.get_biome(&internal_pos);

        Self::get_wit_biome(biome)
    }

    async fn get_entities(
        &mut self,
        world: Resource<World>,
    ) -> wasmtime::Result<Vec<Resource<papokin::plugin::entity::Entity>>> {
        let world_provider = self.get_world_res(&world)?.provider.clone();
        let mut entities = Vec::new();

        // 将玩家作为实体添加
        for player in world_provider.players.load().iter() {
            entities.push(self.add_entity(player.clone() as Arc<dyn crate::entity::EntityBase>)?);
        }

        // 添加其他实体
        for entity in world_provider.entities.load().iter() {
            entities.push(self.add_entity(entity.clone())?);
        }

        Ok(entities)
    }

    async fn ray_trace_blocks(
        &mut self,
        world: Resource<World>,
        start: WitPosition,
        end: WitPosition,
    ) -> wasmtime::Result<Option<WitPosition>> {
        let world_provider = self.get_world_res(&world)?.provider.clone();
        let start_pos = super::events::from_wasm_position(start);
        let end_pos = super::events::from_wasm_position(end);
        let res = world_provider.raycast(start_pos, end_pos, |pos, w| {
            !w.get_block_state(pos).is_air()
        });
        Ok(res.map(|(p, _)| {
            super::events::to_wasm_position(papokin_util::math::vector3::Vector3::new(
                f64::from(p.0.x),
                f64::from(p.0.y),
                f64::from(p.0.z),
            ))
        }))
    }

    async fn ray_trace_block(
        &mut self,
        world: Resource<World>,
        start: WitPosition,
        end: WitPosition,
        include_fluids: bool,
    ) -> wasmtime::Result<Option<WitRayTraceBlockResult>> {
        let world_provider = self.get_world_res(&world)?.provider.clone();
        let start_pos = super::events::from_wasm_position(start);
        let end_pos = super::events::from_wasm_position(end);
        let res = world_provider.ray_trace_block(start_pos, end_pos, include_fluids);
        Ok(res.map(|(pos, face, hit_pos)| WitRayTraceBlockResult {
            pos: WitBlockPos {
                x: pos.0.x,
                y: pos.0.y,
                z: pos.0.z,
            },
            face: to_wasm_block_direction(face),
            hit_pos: super::events::to_wasm_position(hit_pos),
        }))
    }

    async fn ray_trace_entity(
        &mut self,
        world: Resource<World>,
        start: WitPosition,
        end: WitPosition,
    ) -> wasmtime::Result<Option<WitRayTraceEntityResult>> {
        let world_provider = self.get_world_res(&world)?.provider.clone();
        let start_pos = super::events::from_wasm_position(start);
        let end_pos = super::events::from_wasm_position(end);
        if let Some((entity, hit_pos, distance)) =
            world_provider.ray_trace_entity(start_pos, end_pos)
        {
            let entity_res = self
                .add_entity(entity)
                .map_err(|_| wasmtime::Error::msg("添加实体资源失败"))?;
            Ok(Some(WitRayTraceEntityResult {
                entity: entity_res,
                hit_pos: super::events::to_wasm_position(hit_pos),
                distance,
            }))
        } else {
            Ok(None)
        }
    }

    async fn ray_trace_entities(
        &mut self,
        world: Resource<World>,
        start: WitPosition,
        end: WitPosition,
    ) -> wasmtime::Result<Vec<WitRayTraceEntityResult>> {
        let world_provider = self.get_world_res(&world)?.provider.clone();
        let start_pos = super::events::from_wasm_position(start);
        let end_pos = super::events::from_wasm_position(end);
        let hits = world_provider.ray_trace_entities(start_pos, end_pos);
        let mut results = Vec::with_capacity(hits.len());
        for (entity, hit_pos, distance) in hits {
            let entity_res = self
                .add_entity(entity)
                .map_err(|_| wasmtime::Error::msg("添加实体资源失败"))?;
            results.push(WitRayTraceEntityResult {
                entity: entity_res,
                hit_pos: super::events::to_wasm_position(hit_pos),
                distance,
            });
        }
        Ok(results)
    }

    async fn get_block_entity(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<Option<BlockEntityType>> {
        let world_provider = self.get_world_res(&world)?.provider.clone();
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);
        let block_entity = world_provider.get_block_entity(&internal_pos);

        block_entity.map_or_else(|| Ok(None), |be| self.get_wit_block_entity(be))
    }

    async fn get_block_entity_nbt(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<Option<Vec<u8>>> {
        let world_ref = self.get_world_res(&world)?.provider.clone();
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);

        let Some(entity) = world_ref.get_block_entity(&internal_pos) else {
            return Ok(None);
        };

        let mut nbt = papokin_nbt::NbtCompound::new();
        entity.write_internal(&mut nbt);

        let bytes = papokin_nbt::Nbt::from(nbt).write_unnamed();
        Ok(Some(bytes.to_vec()))
    }

    async fn set_block_entity_nbt(
        &mut self,
        world: Resource<World>,
        pos: WitBlockPos,
        nbt_data: Vec<u8>,
    ) -> wasmtime::Result<Result<(), String>> {
        let world_ref = self.get_world_res(&world)?.provider.clone();
        let internal_pos = BlockPos::new(pos.x, pos.y, pos.z);

        let mut cursor = std::io::Cursor::new(&nbt_data[..]);
        let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
            papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
        );
        let mut nbt: papokin_nbt::NbtCompound = papokin_nbt::Nbt::read_unnamed(&mut reader)
            .map_err(|e| wasmtime::Error::msg(format!("无效的 NBT：{e}")))?
            .root_tag;

        // 用调用方提供的位置覆盖 NBT 位置，以便方块实体
        // 实体在粘贴原理图时落在正确的坐标上。
        nbt.put_int("x", pos.x);
        nbt.put_int("y", pos.y);
        nbt.put_int("z", pos.z);

        // 使用 add_block_entity_nbt 进行懒加载——避免广播
        // 批量操作期间为每个方块实体各发一个数据包。
        world_ref.add_block_entity_nbt(internal_pos, &nbt);
        Ok(Ok(()))
    }

    async fn set_chunk_generator(
        &mut self,
        world: Resource<World>,
        generator_id: u32,
    ) -> wasmtime::Result<()> {
        let world_ref = self.get_world_res(&world)?.provider.clone();
        let Some(plugin_weak) = self.plugin.as_ref() else {
            return Ok(());
        };
        let Some(plugin) = plugin_weak.upgrade() else {
            return Ok(());
        };
        let Some(server) = self.server.clone() else {
            return Ok(());
        };

        let wasm_gen = Arc::new(WasmChunkGenerator {
            generator_id,
            plugin,
            dimension: world_ref.dimension.clone(),
            seed: world_ref.level.seed.0,
            server,
        });

        world_ref.level.set_world_gen(Arc::new(
            papokin_world::generation::generator::WorldGenerator::Custom(wasm_gen),
        ));
        Ok(())
    }

    async fn get_name(&mut self, world: Resource<World>) -> wasmtime::Result<String> {
        Ok(self
            .get_world_res(&world)?
            .provider
            .get_world_name()
            .to_string())
    }

    async fn set_custom_data(
        &mut self,
        world: Resource<World>,
        namespace: String,
        key: String,
        value: super::common::WitNbtTree,
    ) -> wasmtime::Result<()> {
        let world_res = self.get_world_res(&world)?;
        let tag = super::common::from_wit_nbt_tree(&value).map_err(wasmtime::Error::msg)?;
        world_res.provider.set_custom_data(&namespace, &key, tag);
        Ok(())
    }

    async fn get_custom_data(
        &mut self,
        world: Resource<World>,
        namespace: String,
        key: String,
    ) -> wasmtime::Result<Option<super::common::WitNbtTree>> {
        let world_res = self.get_world_res(&world)?;
        let tag = world_res.provider.get_custom_data(&namespace, &key);
        Ok(tag.map(super::common::to_wit_nbt_tree))
    }

    async fn remove_custom_data(
        &mut self,
        world: Resource<World>,
        namespace: String,
        key: String,
    ) -> wasmtime::Result<()> {
        let world_res = self.get_world_res(&world)?;
        world_res.provider.remove_custom_data(&namespace, &key);
        Ok(())
    }

    async fn has_custom_data(
        &mut self,
        world: Resource<World>,
        namespace: String,
        key: String,
    ) -> wasmtime::Result<bool> {
        let world_res = self.get_world_res(&world)?;
        Ok(world_res.provider.has_custom_data(&namespace, &key))
    }

    async fn get_game_rule(
        &mut self,
        world: Resource<World>,
        rule: WitGameRule,
    ) -> wasmtime::Result<WitGameRuleValue> {
        let world_res = self.get_world_res(&world)?;
        let internal_rule = from_wit_game_rule(rule);
        let value = world_res.provider.get_game_rule(&internal_rule);
        Ok(to_wit_game_rule_value(&value))
    }

    async fn set_game_rule(
        &mut self,
        world: Resource<World>,
        rule: WitGameRule,
        value: WitGameRuleValue,
    ) -> wasmtime::Result<()> {
        let world_res = self.get_world_res(&world)?;
        let internal_rule = from_wit_game_rule(rule);
        let internal_value = from_wit_game_rule_value(value);
        world_res
            .provider
            .set_game_rule(&internal_rule, internal_value);
        Ok(())
    }

    async fn drop(&mut self, rep: Resource<World>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<WorldResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

impl papokin::plugin::world::HostWorldWithStore<PluginHostState> for HasSelf<PluginHostState> {
    async fn set_raining(
        mut host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        raining: bool,
    ) -> wasmtime::Result<()> {
        let (world, plugin) = world_and_plugin(host.get(), &world)?;
        plugin
            .store
            .pump_blocking(&mut host, move || world.set_raining(raining))
            .await
    }

    async fn set_thundering(
        mut host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        thundering: bool,
    ) -> wasmtime::Result<()> {
        let (world, plugin) = world_and_plugin(host.get(), &world)?;
        plugin
            .store
            .pump_blocking(&mut host, move || world.set_thundering(thundering))
            .await
    }

    async fn create_explosion(
        mut host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        pos: papokin::plugin::common::Position,
        power: f32,
        _create_fire: bool,
        interaction: papokin::plugin::world::ExplosionInteraction,
    ) -> wasmtime::Result<()> {
        let (world, plugin) = world_and_plugin(host.get(), &world)?;
        let interaction = match interaction {
            papokin::plugin::world::ExplosionInteraction::None => ExplosionInteraction::None,
            papokin::plugin::world::ExplosionInteraction::Block => ExplosionInteraction::Block,
            papokin::plugin::world::ExplosionInteraction::Mob => ExplosionInteraction::Mob,
            papokin::plugin::world::ExplosionInteraction::Tnt => ExplosionInteraction::Tnt,
            papokin::plugin::world::ExplosionInteraction::Trigger => ExplosionInteraction::Trigger,
        };
        let pos = papokin_util::math::vector3::Vector3::new(pos.0, pos.1, pos.2);
        plugin
            .store
            .pump_blocking(&mut host, move || world.explode(pos, power, interaction))
            .await
    }

    async fn spawn_entity(
        mut host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        entity_type: papokin::plugin::entity_types::EntityType,
        pos: papokin::plugin::common::Position,
    ) -> wasmtime::Result<Resource<papokin::plugin::world::Entity>> {
        let (world, plugin) = world_and_plugin(host.get(), &world)?;

        let internal_type = super::entity::from_wit_entity_type(entity_type)?;
        let pos = papokin_util::math::vector3::Vector3::new(pos.0, pos.1, pos.2);
        let entity =
            crate::entity::r#type::from_type(internal_type, pos, &world, uuid::Uuid::new_v4());
        let spawned_entity = Arc::clone(&entity);
        plugin
            .store
            .pump_blocking(&mut host, move || world.spawn_entity(spawned_entity))
            .await?;
        host.get().add_entity(entity)
    }

    async fn spawn_entity_from_snapshot(
        mut host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        nbt: Vec<u8>,
        pos: papokin::plugin::common::Position,
    ) -> wasmtime::Result<Option<Resource<papokin::plugin::world::Entity>>> {
        let (world, plugin) = world_and_plugin(host.get(), &world)?;

        let mut cursor = std::io::Cursor::new(&nbt[..]);
        let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
            papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
        );
        let Ok(snapshot) = papokin_nbt::Nbt::read_unnamed(&mut reader) else {
            return Ok(None);
        };

        let pos = papokin_util::math::vector3::Vector3::new(pos.0, pos.1, pos.2);
        let Some(entity) = crate::entity::r#type::from_nbt(&snapshot.root_tag, pos, &world) else {
            return Ok(None);
        };
        let spawned_entity = Arc::clone(&entity);
        plugin
            .store
            .pump_blocking(&mut host, move || world.spawn_entity(spawned_entity))
            .await?;
        Ok(Some(host.get().add_entity(entity)?))
    }

    async fn strike_lightning(
        mut host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        pos: WitPosition,
        effect_only: bool,
    ) -> wasmtime::Result<()> {
        let (world, plugin) = world_and_plugin(host.get(), &world)?;
        let pos = super::events::from_wasm_position(pos);
        plugin
            .store
            .pump_blocking(&mut host, move || world.strike_lightning(pos, effect_only))
            .await
    }

    async fn save(
        mut host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
    ) -> wasmtime::Result<Result<(), String>> {
        let (world, plugin) = world_and_plugin(host.get(), &world)?;
        plugin
            .store
            .pump_reentry(&mut host, async move { world.save().await })
            .await?;
        Ok(Ok(()))
    }

    async fn set_block(
        host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        pos: WitBlockPos,
        block: WitBlock,
        update_flags: WitBlockFlags,
    ) -> wasmtime::Result<()> {
        let block_id = BlockId::new_or_air(block.id);
        let default_state_id = papokin_data::Block::from_id(block_id)
            .default_state
            .id
            .as_u16();
        set_block_state_with_store(host, world, pos, default_state_id, update_flags).await
    }

    async fn set_block_by_id(
        host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        pos: WitBlockPos,
        block_id: u16,
        update_flags: WitBlockFlags,
    ) -> wasmtime::Result<()> {
        let Some(id) = BlockId::new(block_id) else {
            return Err(wasmtime::Error::msg("无效的 BlockId"));
        };
        let default_state_id = papokin_data::Block::from_id(id).default_state.id.as_u16();
        set_block_state_with_store(host, world, pos, default_state_id, update_flags).await
    }

    async fn set_block_by_name(
        host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        pos: WitBlockPos,
        name: String,
        update_flags: WitBlockFlags,
    ) -> wasmtime::Result<bool> {
        let Some(block) = papokin_data::Block::from_name(&name) else {
            return Ok(false);
        };
        let default_state_id = block.default_state.id.as_u16();
        set_block_state_with_store(host, world, pos, default_state_id, update_flags).await?;
        Ok(true)
    }

    async fn set_block_state(
        host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        pos: WitBlockPos,
        state: u16,
        update_flags: WitBlockFlags,
    ) -> wasmtime::Result<()> {
        set_block_state_with_store(host, world, pos, state, update_flags).await
    }
}

impl papokin::plugin::world::HostChunk for PluginHostState {
    async fn get_x(&mut self, chunk: Resource<WitChunk>) -> wasmtime::Result<i32> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        Ok(chunk_data.x)
    }

    async fn get_z(&mut self, chunk: Resource<WitChunk>) -> wasmtime::Result<i32> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        Ok(chunk_data.z)
    }

    async fn get_block_state_id(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<u16> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        Ok(chunk_data
            .section
            .get_block_absolute_y(pos.x as usize, pos.y, pos.z as usize)
            .unwrap_or(BlockStateId::AIR)
            .as_u16())
    }

    async fn get_block_state(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<WitBlockState> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        let id = chunk_data
            .section
            .get_block_absolute_y(pos.x as usize, pos.y, pos.z as usize)
            .unwrap_or(BlockStateId::AIR);
        let state = id.to_state();
        let world_pos = BlockPos::new(chunk_data.x * 16 + pos.x, pos.y, chunk_data.z * 16 + pos.z);
        Ok(to_wit_block_state(state, Some(&world_pos)))
    }

    async fn get_block(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<WitBlock> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        let id = chunk_data
            .section
            .get_block_absolute_y(pos.x as usize, pos.y, pos.z as usize)
            .unwrap_or(BlockStateId::AIR);
        let block = papokin_data::Block::from_state_id(id);
        Ok(to_wit_block(block))
    }

    async fn set_block(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
        block: WitBlock,
    ) -> wasmtime::Result<()> {
        let block_id = BlockId::new_or_air(block.id);
        let default_state_id = papokin_data::Block::from_id(block_id)
            .default_state
            .id
            .as_u16();
        self.set_block_state(chunk, pos, default_state_id).await
    }

    async fn set_block_by_id(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
        block_id: u16,
    ) -> wasmtime::Result<()> {
        let Some(id) = BlockId::new(block_id) else {
            return Err(wasmtime::Error::msg("无效的 BlockId"));
        };
        let default_state_id = papokin_data::Block::from_id(id).default_state.id.as_u16();
        self.set_block_state(chunk, pos, default_state_id).await
    }

    async fn set_block_state(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
        state: u16,
    ) -> wasmtime::Result<()> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (world, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };

        let Some(state) = BlockStateId::new(state) else {
            return Err(wasmtime::Error::msg("无效的 BlockStateId"));
        };

        let replaced =
            chunk_data.set_block_absolute_y(pos.x as usize, pos.y, pos.z as usize, state);

        if replaced != state {
            chunk_data.mark_dirty(true);
            let absolute_pos =
                BlockPos::new(chunk_data.x * 16 + pos.x, pos.y, chunk_data.z * 16 + pos.z);
            world.register_block_change(absolute_pos, state);
        }

        Ok(())
    }

    async fn get_biome(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<papokin::plugin::biomes::Biome> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        let id = chunk_data
            .section
            .get_rough_biome_absolute_y(pos.x as usize, pos.y, pos.z as usize)
            .unwrap_or(0);
        let biome = papokin_data::biome::Biome::from_id(id)
            .unwrap_or(&papokin_data::biome::Biome::THE_VOID);

        Self::get_wit_biome(biome)
    }

    async fn get_block_entity(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<Option<BlockEntityType>> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (world, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        let absolute_pos =
            BlockPos::new(chunk_data.x * 16 + pos.x, pos.y, chunk_data.z * 16 + pos.z);
        let block_entity = world.get_block_entity(&absolute_pos);

        block_entity.map_or_else(|| Ok(None), |be| self.get_wit_block_entity(be))
    }

    async fn get_top_block_y(
        &mut self,
        chunk: Resource<WitChunk>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<i32> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        Ok(chunk_data
            .heightmap
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(
                ChunkHeightmapType::WorldSurface,
                x,
                z,
                chunk_data.section.min_y,
            ))
    }

    async fn get_sky_light(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<u8> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        let section_index = (pos.y - chunk_data.section.min_y) as usize / 16;
        Ok(chunk_data
            .light_engine
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .sky_light
            .get(section_index)
            .map_or(0, |c| {
                c.get(pos.x as usize, pos.y as usize % 16, pos.z as usize)
            }))
    }

    async fn get_block_light(
        &mut self,
        chunk: Resource<WitChunk>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<u8> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        let section_index = (pos.y - chunk_data.section.min_y) as usize / 16;
        Ok(chunk_data
            .light_engine
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .block_light
            .get(section_index)
            .map_or(0, |c| {
                c.get(pos.x as usize, pos.y as usize % 16, pos.z as usize)
            }))
    }

    async fn set_custom_data(
        &mut self,
        chunk: Resource<WitChunk>,
        namespace: String,
        key: String,
        value: super::common::WitNbtTree,
    ) -> wasmtime::Result<()> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        let tag = super::common::from_wit_nbt_tree(&value).map_err(wasmtime::Error::msg)?;
        chunk_data.set_custom_data(&namespace, &key, tag);
        Ok(())
    }

    async fn get_custom_data(
        &mut self,
        chunk: Resource<WitChunk>,
        namespace: String,
        key: String,
    ) -> wasmtime::Result<Option<super::common::WitNbtTree>> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        let tag = chunk_data.get_custom_data(&namespace, &key);
        Ok(tag.map(super::common::to_wit_nbt_tree))
    }

    async fn remove_custom_data(
        &mut self,
        chunk: Resource<WitChunk>,
        namespace: String,
        key: String,
    ) -> wasmtime::Result<()> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        chunk_data.remove_custom_data(&namespace, &key);
        Ok(())
    }

    async fn has_custom_data(
        &mut self,
        chunk: Resource<WitChunk>,
        namespace: String,
        key: String,
    ) -> wasmtime::Result<bool> {
        let chunk_res = self.get_chunk_res(&chunk)?;
        let (_, chunk_data) = &chunk_res.provider;
        let Some(chunk_data) = chunk_data.upgrade() else {
            return Err(wasmtime::Error::msg("区块已卸载"));
        };
        Ok(chunk_data.has_custom_data(&namespace, &key))
    }

    async fn drop(&mut self, rep: Resource<WitChunk>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<ChunkResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

impl PluginHostState {
    /// 将快照局部位置解析为线性方块索引，否则为 `None`
    /// 当位置超出已复制数据的范围时。
    fn snapshot_block_index(snapshot: &HostChunkSnapshotData, pos: &WitBlockPos) -> Option<usize> {
        if !(0..16).contains(&pos.x) || !(0..16).contains(&pos.z) {
            return None;
        }
        let relative_y = pos.y - snapshot.min_y;
        if relative_y < 0 {
            return None;
        }
        let relative_y = relative_y as usize;
        let section = relative_y / 16;
        if section >= snapshot.section_count as usize {
            return None;
        }
        Some(section * 4096 + (relative_y % 16) * 256 + pos.z as usize * 16 + pos.x as usize)
    }
}

impl papokin::plugin::world::HostChunkSnapshot for PluginHostState {
    async fn get_x(&mut self, snapshot: Resource<WitChunkSnapshot>) -> wasmtime::Result<i32> {
        Ok(self.get_chunk_snapshot_res(&snapshot)?.provider.x)
    }

    async fn get_z(&mut self, snapshot: Resource<WitChunkSnapshot>) -> wasmtime::Result<i32> {
        Ok(self.get_chunk_snapshot_res(&snapshot)?.provider.z)
    }

    async fn get_min_y(&mut self, snapshot: Resource<WitChunkSnapshot>) -> wasmtime::Result<i32> {
        Ok(self.get_chunk_snapshot_res(&snapshot)?.provider.min_y)
    }

    async fn get_section_count(
        &mut self,
        snapshot: Resource<WitChunkSnapshot>,
    ) -> wasmtime::Result<u32> {
        Ok(self
            .get_chunk_snapshot_res(&snapshot)?
            .provider
            .section_count)
    }

    async fn get_block_state_id(
        &mut self,
        snapshot: Resource<WitChunkSnapshot>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<u16> {
        let snapshot_res = &self.get_chunk_snapshot_res(&snapshot)?.provider;
        let Some(index) = Self::snapshot_block_index(snapshot_res, &pos) else {
            return Ok(BlockStateId::AIR.as_u16());
        };
        Ok(snapshot_res.blocks[index])
    }

    async fn get_block_state(
        &mut self,
        snapshot: Resource<WitChunkSnapshot>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<WitBlockState> {
        let (x, z) = {
            let snapshot_res = &self.get_chunk_snapshot_res(&snapshot)?.provider;
            (snapshot_res.x, snapshot_res.z)
        };
        let id = self.get_block_state_id(snapshot, pos).await?;
        let state = BlockStateId::new_or_air(id).to_state();
        let world_pos = BlockPos::new(x * 16 + pos.x, pos.y, z * 16 + pos.z);
        Ok(to_wit_block_state(state, Some(&world_pos)))
    }

    async fn get_block(
        &mut self,
        snapshot: Resource<WitChunkSnapshot>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<WitBlock> {
        let id = self.get_block_state_id(snapshot, pos).await?;
        let block = papokin_data::Block::from_state_id(BlockStateId::new_or_air(id));
        Ok(to_wit_block(block))
    }

    async fn get_biome(
        &mut self,
        snapshot: Resource<WitChunkSnapshot>,
        pos: WitBlockPos,
    ) -> wasmtime::Result<papokin::plugin::biomes::Biome> {
        let snapshot_res = &self.get_chunk_snapshot_res(&snapshot)?.provider;
        let biome_id = if !(0..16).contains(&pos.x) || !(0..16).contains(&pos.z) {
            0
        } else {
            let relative_y = pos.y - snapshot_res.min_y;
            if relative_y < 0 {
                0
            } else {
                let relative_y = relative_y as usize;
                let section = relative_y / 16;
                if section >= snapshot_res.section_count as usize {
                    0
                } else {
                    let quart_y = (relative_y >> 2) & 3;
                    snapshot_res.biomes[section * 64
                        + quart_y * 16
                        + (pos.z as usize >> 2) * 4
                        + (pos.x as usize >> 2)]
                }
            }
        };
        let biome = papokin_data::biome::Biome::from_id(biome_id)
            .unwrap_or(&papokin_data::biome::Biome::THE_VOID);
        Self::get_wit_biome(biome)
    }

    async fn get_top_block_y(
        &mut self,
        snapshot: Resource<WitChunkSnapshot>,
        x: i32,
        z: i32,
    ) -> wasmtime::Result<i32> {
        let snapshot_res = &self.get_chunk_snapshot_res(&snapshot)?.provider;
        if !(0..16).contains(&x) || !(0..16).contains(&z) {
            return Ok(snapshot_res.min_y - 1);
        }
        let height = snapshot_res.section_count as usize * 16;
        for relative_y in (0..height).rev() {
            let index =
                (relative_y / 16) * 4096 + (relative_y % 16) * 256 + z as usize * 16 + x as usize;
            if snapshot_res.blocks[index] != BlockStateId::AIR.as_u16() {
                return Ok(snapshot_res.min_y + relative_y as i32);
            }
        }
        Ok(snapshot_res.min_y - 1)
    }

    async fn dump_section_block_states(
        &mut self,
        snapshot: Resource<WitChunkSnapshot>,
        section_index: u32,
    ) -> wasmtime::Result<Vec<u16>> {
        let snapshot_res = &self.get_chunk_snapshot_res(&snapshot)?.provider;
        if section_index >= snapshot_res.section_count {
            return Err(wasmtime::Error::msg(format!(
                "区段索引 {section_index} 超出范围（共 {} 个区段）",
                snapshot_res.section_count
            )));
        }
        let start = section_index as usize * 4096;
        Ok(snapshot_res.blocks[start..start + 4096].to_vec())
    }

    async fn dump_section_biomes(
        &mut self,
        snapshot: Resource<WitChunkSnapshot>,
        section_index: u32,
    ) -> wasmtime::Result<Vec<u8>> {
        let snapshot_res = &self.get_chunk_snapshot_res(&snapshot)?.provider;
        if section_index >= snapshot_res.section_count {
            return Err(wasmtime::Error::msg(format!(
                "区段索引 {section_index} 超出范围（共 {} 个区段）",
                snapshot_res.section_count
            )));
        }
        let start = section_index as usize * 64;
        Ok(snapshot_res.biomes[start..start + 64].to_vec())
    }

    async fn drop(&mut self, rep: Resource<WitChunkSnapshot>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<ChunkSnapshotResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

impl papokin::plugin::world::HostWorldBorder for PluginHostState {
    async fn get_center_x(&mut self, border: Resource<WitWorldBorder>) -> wasmtime::Result<f64> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .center_x)
    }

    async fn get_center_z(&mut self, border: Resource<WitWorldBorder>) -> wasmtime::Result<f64> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .center_z)
    }

    async fn get_center(
        &mut self,
        border: Resource<WitWorldBorder>,
    ) -> wasmtime::Result<WitPosition> {
        let border_res = self.get_world_border_res(&border)?;
        let guard = border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(super::events::to_wasm_position(
            papokin_util::math::vector3::Vector3::new(guard.center_x, 0.0, guard.center_z),
        ))
    }

    async fn set_center(
        &mut self,
        border: Resource<WitWorldBorder>,
        x: f64,
        z: f64,
    ) -> wasmtime::Result<()> {
        let border_res = self.get_world_border_res(&border)?;
        let world = border_res.provider.clone();
        world
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_center(&world, x, z);
        Ok(())
    }

    async fn get_diameter(&mut self, border: Resource<WitWorldBorder>) -> wasmtime::Result<f64> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .new_diameter)
    }

    async fn get_size(&mut self, border: Resource<WitWorldBorder>) -> wasmtime::Result<f64> {
        self.get_diameter(border).await
    }

    async fn set_diameter(
        &mut self,
        border: Resource<WitWorldBorder>,
        diameter: f64,
        speed: Option<u64>,
    ) -> wasmtime::Result<()> {
        let border_res = self.get_world_border_res(&border)?;
        let world = border_res.provider.clone();
        world
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_diameter(&world, diameter, speed.map(|s| s as i64));
        Ok(())
    }

    async fn set_size(
        &mut self,
        border: Resource<WitWorldBorder>,
        size: f64,
    ) -> wasmtime::Result<()> {
        self.set_diameter(border, size, None).await
    }

    async fn set_size_transition(
        &mut self,
        border: Resource<WitWorldBorder>,
        new_size: f64,
        time_seconds: u64,
    ) -> wasmtime::Result<()> {
        let speed_millis = time_seconds.saturating_mul(1000);
        self.set_diameter(border, new_size, Some(speed_millis))
            .await
    }

    async fn get_target_diameter(
        &mut self,
        border: Resource<WitWorldBorder>,
    ) -> wasmtime::Result<f64> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .new_diameter)
    }

    async fn get_target_speed(
        &mut self,
        border: Resource<WitWorldBorder>,
    ) -> wasmtime::Result<i64> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .speed)
    }

    async fn get_warning_distance(
        &mut self,
        border: Resource<WitWorldBorder>,
    ) -> wasmtime::Result<i32> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .warning_blocks)
    }

    async fn set_warning_distance(
        &mut self,
        border: Resource<WitWorldBorder>,
        distance: i32,
    ) -> wasmtime::Result<()> {
        let border_res = self.get_world_border_res(&border)?;
        let world = border_res.provider.clone();
        world
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_warning_distance(&world, distance);
        Ok(())
    }

    async fn get_warning_delay(
        &mut self,
        border: Resource<WitWorldBorder>,
    ) -> wasmtime::Result<i32> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .warning_time)
    }

    async fn set_warning_delay(
        &mut self,
        border: Resource<WitWorldBorder>,
        delay: i32,
    ) -> wasmtime::Result<()> {
        let border_res = self.get_world_border_res(&border)?;
        let world = border_res.provider.clone();
        world
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_warning_delay(&world, delay);
        Ok(())
    }

    async fn get_warning_time(
        &mut self,
        border: Resource<WitWorldBorder>,
    ) -> wasmtime::Result<i32> {
        self.get_warning_delay(border).await
    }

    async fn set_warning_time(
        &mut self,
        border: Resource<WitWorldBorder>,
        time: i32,
    ) -> wasmtime::Result<()> {
        self.set_warning_delay(border, time).await
    }

    async fn get_damage_buffer(
        &mut self,
        border: Resource<WitWorldBorder>,
    ) -> wasmtime::Result<f64> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(f64::from(
            border_res
                .provider
                .worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .buffer,
        ))
    }

    async fn set_damage_buffer(
        &mut self,
        border: Resource<WitWorldBorder>,
        buffer: f64,
    ) -> wasmtime::Result<()> {
        let border_res = self.get_world_border_res(&border)?;
        border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_damage_buffer(buffer as f32);
        Ok(())
    }

    async fn get_damage_amount(
        &mut self,
        border: Resource<WitWorldBorder>,
    ) -> wasmtime::Result<f64> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(f64::from(
            border_res
                .provider
                .worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .damage_per_block,
        ))
    }

    async fn set_damage_amount(
        &mut self,
        border: Resource<WitWorldBorder>,
        damage: f64,
    ) -> wasmtime::Result<()> {
        let border_res = self.get_world_border_res(&border)?;
        border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_damage_per_block(damage as f32);
        Ok(())
    }

    async fn contains(
        &mut self,
        border: Resource<WitWorldBorder>,
        x: f64,
        z: f64,
    ) -> wasmtime::Result<bool> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(x, z))
    }

    async fn contains_pos(
        &mut self,
        border: Resource<WitWorldBorder>,
        pos: WitPosition,
    ) -> wasmtime::Result<bool> {
        let border_res = self.get_world_border_res(&border)?;
        Ok(border_res
            .provider
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(pos.0, pos.2))
    }

    async fn reset(&mut self, border: Resource<WitWorldBorder>) -> wasmtime::Result<()> {
        let border_res = self.get_world_border_res(&border)?;
        let world = border_res.provider.clone();
        world
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reset(&world);
        Ok(())
    }

    async fn drop(&mut self, rep: Resource<WitWorldBorder>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<WorldBorderResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

impl papokin::plugin::world::HostChunkBuffer for PluginHostState {
    async fn get_x(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
    ) -> wasmtime::Result<i32> {
        let res = self.get_chunk_buffer_res(&this)?;
        Ok(res.provider.x)
    }

    async fn get_z(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
    ) -> wasmtime::Result<i32> {
        let res = self.get_chunk_buffer_res(&this)?;
        Ok(res.provider.z)
    }

    async fn get_min_y(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
    ) -> wasmtime::Result<i32> {
        let res = self.get_chunk_buffer_res(&this)?;
        Ok(res.provider.min_y)
    }

    async fn get_height(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
    ) -> wasmtime::Result<u32> {
        let res = self.get_chunk_buffer_res(&this)?;
        Ok(res.provider.height)
    }

    async fn set_block_state_id(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
        x: u8,
        y: i32,
        z: u8,
        state_id: u16,
    ) -> wasmtime::Result<()> {
        let res = self.get_chunk_buffer_res(&this)?;
        if x < 16 && z < 16 {
            let world_x =
                papokin_world::generation::positions::chunk_pos::start_block_x(res.provider.x)
                    + x as i32;
            let world_z =
                papokin_world::generation::positions::chunk_pos::start_block_z(res.provider.z)
                    + z as i32;
            let mut proto = res
                .provider
                .proto_chunk
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let block_state = papokin_data::BlockState::from_id(
                papokin_data::BlockStateId::new(state_id)
                    .unwrap_or(papokin_data::BlockStateId::AIR),
            );
            proto.set_block_state(world_x, y, world_z, block_state);
        }
        Ok(())
    }

    async fn get_block_state_id(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
        x: u8,
        y: i32,
        z: u8,
    ) -> wasmtime::Result<u16> {
        let res = self.get_chunk_buffer_res(&this)?;
        if x < 16 && z < 16 {
            let proto = res
                .provider
                .proto_chunk
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let local_y = y - proto.bottom_y() as i32;
            if local_y >= 0 && local_y < proto.height() as i32 {
                Ok(proto
                    .get_block_state_raw(x as i32, local_y, z as i32)
                    .as_u16())
            } else {
                Ok(0)
            }
        } else {
            Ok(0)
        }
    }

    async fn fill_layer(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
        y: i32,
        state_id: u16,
    ) -> wasmtime::Result<()> {
        let res = self.get_chunk_buffer_res(&this)?;
        let start_x =
            papokin_world::generation::positions::chunk_pos::start_block_x(res.provider.x);
        let start_z =
            papokin_world::generation::positions::chunk_pos::start_block_z(res.provider.z);
        let block_state = papokin_data::BlockState::from_id(
            papokin_data::BlockStateId::new(state_id).unwrap_or(papokin_data::BlockStateId::AIR),
        );
        let mut proto = res
            .provider
            .proto_chunk
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for x in 0..16 {
            for z in 0..16 {
                proto.set_block_state(start_x + x, y, start_z + z, block_state);
            }
        }
        Ok(())
    }

    async fn fill_range(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
        x: u8,
        min_y: i32,
        max_y: i32,
        z: u8,
        state_id: u16,
    ) -> wasmtime::Result<()> {
        let res = self.get_chunk_buffer_res(&this)?;
        if x < 16 && z < 16 {
            let world_x =
                papokin_world::generation::positions::chunk_pos::start_block_x(res.provider.x)
                    + x as i32;
            let world_z =
                papokin_world::generation::positions::chunk_pos::start_block_z(res.provider.z)
                    + z as i32;
            let block_state = papokin_data::BlockState::from_id(
                papokin_data::BlockStateId::new(state_id)
                    .unwrap_or(papokin_data::BlockStateId::AIR),
            );
            let mut proto = res
                .provider
                .proto_chunk
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for y in min_y..=max_y {
                proto.set_block_state(world_x, y, world_z, block_state);
            }
        }
        Ok(())
    }

    async fn fill_cuboid(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
        min_x: u8,
        min_y: i32,
        min_z: u8,
        max_x: u8,
        max_y: i32,
        max_z: u8,
        state_id: u16,
    ) -> wasmtime::Result<()> {
        let res = self.get_chunk_buffer_res(&this)?;
        let start_x =
            papokin_world::generation::positions::chunk_pos::start_block_x(res.provider.x);
        let start_z =
            papokin_world::generation::positions::chunk_pos::start_block_z(res.provider.z);
        let block_state = papokin_data::BlockState::from_id(
            papokin_data::BlockStateId::new(state_id).unwrap_or(papokin_data::BlockStateId::AIR),
        );
        let mut proto = res
            .provider
            .proto_chunk
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let max_x = max_x.min(15);
        let max_z = max_z.min(15);
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    proto.set_block_state(start_x + x as i32, y, start_z + z as i32, block_state);
                }
            }
        }
        Ok(())
    }

    async fn set_biome(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
        x: u8,
        y: i32,
        z: u8,
        biome: papokin::plugin::biomes::Biome,
    ) -> wasmtime::Result<()> {
        let res = self.get_chunk_buffer_res(&this)?;
        if x < 16 && z < 16 {
            let biome_id = biome as u8;
            let mut proto = res
                .provider
                .proto_chunk
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let biome_x = x as i32 / 4;
            let biome_z = z as i32 / 4;
            let biome_y = (y - proto.bottom_y() as i32) / 4;
            if biome_y >= 0 && (biome_y as usize) < (proto.height() as usize / 4) {
                let index = proto.local_biome_pos_to_biome_index(biome_x, biome_y, biome_z);
                if index < proto.flat_biome_map.len() {
                    proto.flat_biome_map[index] = biome_id;
                }
            }
        }
        Ok(())
    }

    async fn fill_biome(
        &mut self,
        this: Resource<papokin::plugin::world::ChunkBuffer>,
        biome: papokin::plugin::biomes::Biome,
    ) -> wasmtime::Result<()> {
        let res = self.get_chunk_buffer_res(&this)?;
        let biome_id = biome as u8;
        let mut proto = res
            .provider
            .proto_chunk
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        proto.flat_biome_map.fill(biome_id);
        Ok(())
    }

    async fn drop(
        &mut self,
        rep: Resource<papokin::plugin::world::ChunkBuffer>,
    ) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<crate::plugin::loader::wasm::wasm_host::state::ChunkBufferResource>(
                Resource::new_own(rep.rep()),
            )
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

struct ProtoChunkRestore<'a> {
    target: &'a mut papokin_world::ProtoChunk,
    source: Arc<StdMutex<papokin_world::ProtoChunk>>,
}

impl Drop for ProtoChunkRestore<'_> {
    fn drop(&mut self) {
        let mut source = self
            .source
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::swap(self.target, &mut source);
    }
}

#[derive(Clone)]
pub struct WasmChunkGenerator {
    pub generator_id: u32,
    pub plugin: Arc<crate::plugin::loader::wasm::wasm_host::WasmPlugin>,
    pub dimension: papokin_data::dimension::Dimension,
    pub seed: u64,
    pub server: Arc<crate::server::Server>,
}

impl WasmChunkGenerator {
    fn invoke_phase(
        &self,
        phase: papokin::plugin::world::GenerationPhase,
        proto_chunk: &mut papokin_world::ProtoChunk,
    ) {
        let x = proto_chunk.x;
        let z = proto_chunk.z;
        let min_y = proto_chunk.bottom_y() as i32;
        let height = proto_chunk.height() as u32;
        let placeholder_generator =
            papokin_world::generation::generator::WorldGenerator::Custom(Arc::new(self.clone()));
        let placeholder = papokin_world::ProtoChunk::new(x, z, &placeholder_generator);
        let owned_proto_chunk = std::mem::replace(proto_chunk, placeholder);
        let shared_proto_chunk = Arc::new(StdMutex::new(owned_proto_chunk));
        let _restore = ProtoChunkRestore {
            target: proto_chunk,
            source: Arc::clone(&shared_proto_chunk),
        };
        let chunk_buffer = crate::plugin::loader::wasm::wasm_host::state::ChunkBuffer {
            x,
            z,
            min_y,
            height,
            proto_chunk: Arc::clone(&shared_proto_chunk),
        };

        let function = match self.plugin.plugin_instance.as_ref() {
            crate::plugin::loader::wasm::wasm_host::PluginInstance::V0_1(plugin) => {
                plugin.func_handle_generate_phase()
            }
        };
        let generator_id = self.generator_id;
        let run = async {
            let result = self
                .plugin
                .store
                .call_guest(move |mut guest| {
                    Box::pin(async move {
                        let (buffer_resource, buffer_rep) = guest.with(|mut store| {
                            let resource = store.data_mut().add_chunk_buffer(chunk_buffer)?;
                            let rep = resource.rep();
                            Ok::<_, wasmtime::Error>((resource, rep))
                        })?;
                        let result = guest
                            .call(function, (generator_id, phase, buffer_resource))
                            .await;
                        guest.with(|mut store| {
                            let _ = store.data_mut().resource_table.delete::<
                                crate::plugin::loader::wasm::wasm_host::state::ChunkBufferResource,
                            >(wasmtime::component::Resource::new_own(buffer_rep));
                        });
                        result
                    })
                })
                .await;
            if let Err(error) = result {
                panic!("生成器 {generator_id} 的 Wasm 区块生成阶段失败：{error}");
            }
        };

        // Tokio 运行时很可能不可用，因为区块生成是在 rayon 上进行的
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| {
                self.server.runtime.block_on(run);
            });
        } else {
            self.server.runtime.block_on(run);
        }
    }
}

impl papokin_world::generation::generator::CustomChunkGenerator for WasmChunkGenerator {
    fn dimension(&self) -> &papokin_data::dimension::Dimension {
        &self.dimension
    }

    fn seed(&self) -> u64 {
        self.seed
    }

    fn step_to_biomes(&self, chunk: &mut papokin_world::ProtoChunk) {
        self.invoke_phase(papokin::plugin::world::GenerationPhase::Biomes, chunk);
        chunk.stage = papokin_world::chunk_system::StagedChunkEnum::Biomes;
    }

    fn step_to_noise(&self, chunk: &mut papokin_world::ProtoChunk) {
        self.invoke_phase(papokin::plugin::world::GenerationPhase::Noise, chunk);
        chunk.stage = papokin_world::chunk_system::StagedChunkEnum::Noise;
    }

    fn step_to_surface(&self, chunk: &mut papokin_world::ProtoChunk) {
        self.invoke_phase(papokin::plugin::world::GenerationPhase::Surface, chunk);
        chunk.stage = papokin_world::chunk_system::StagedChunkEnum::Surface;
    }

    fn step_to_carvers(&self, chunk: &mut papokin_world::ProtoChunk) {
        chunk.stage = papokin_world::chunk_system::StagedChunkEnum::Carvers;
    }

    fn step_to_features(
        &self,
        cache: &mut papokin_world::chunk_system::generation_cache::Cache,
        _block_registry: &dyn papokin_world::world::WorldPortalExt,
    ) {
        let mid = ((cache.size * cache.size) >> 1) as usize;
        let chunk = cache.chunks[mid].get_proto_chunk_mut();
        self.invoke_phase(papokin::plugin::world::GenerationPhase::Features, chunk);
        chunk.stage = papokin_world::chunk_system::StagedChunkEnum::Features;
    }
}

#[must_use]
pub const fn from_wit_sound_category(
    category: papokin::plugin::sounds::SoundCategory,
) -> papokin_data::sound::SoundCategory {
    match category {
        papokin::plugin::sounds::SoundCategory::Master => {
            papokin_data::sound::SoundCategory::Master
        }
        papokin::plugin::sounds::SoundCategory::Music => papokin_data::sound::SoundCategory::Music,
        papokin::plugin::sounds::SoundCategory::Records => {
            papokin_data::sound::SoundCategory::Records
        }
        papokin::plugin::sounds::SoundCategory::Weather => {
            papokin_data::sound::SoundCategory::Weather
        }
        papokin::plugin::sounds::SoundCategory::Blocks => {
            papokin_data::sound::SoundCategory::Blocks
        }
        papokin::plugin::sounds::SoundCategory::Hostile => {
            papokin_data::sound::SoundCategory::Hostile
        }
        papokin::plugin::sounds::SoundCategory::Neutral => {
            papokin_data::sound::SoundCategory::Neutral
        }
        papokin::plugin::sounds::SoundCategory::Players => {
            papokin_data::sound::SoundCategory::Players
        }
        papokin::plugin::sounds::SoundCategory::Ambient => {
            papokin_data::sound::SoundCategory::Ambient
        }
        papokin::plugin::sounds::SoundCategory::Voice => papokin_data::sound::SoundCategory::Voice,
        papokin::plugin::sounds::SoundCategory::Ui => papokin_data::sound::SoundCategory::Ui,
    }
}

pub(crate) fn to_wit_biome(
    biome: &papokin_data::biome::Biome,
) -> wasmtime::Result<papokin::plugin::biomes::Biome> {
    let name = biome
        .registry_id
        .strip_prefix("minecraft:")
        .unwrap_or(biome.registry_id);
    let index = papokin_data::biome::Biome::ALL
        .binary_search_by_key(&name, |b| b.registry_id)
        .map_err(|_| wasmtime::Error::msg(format!("未知的生物群系：{}", biome.registry_id)))?;

    // SAFETY: WIT 枚举的变体与 Biome::ALL 按字母顺序一一对应生成。
    Ok(unsafe { std::mem::transmute::<u8, papokin::plugin::biomes::Biome>(index as u8) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wit_particle_ids_match_internal_particle_ids() {
        let cases = [
            (
                papokin::plugin::particles::Particle::AngryVillager,
                papokin_data::particle::Particle::AngryVillager,
            ),
            (
                papokin::plugin::particles::Particle::HappyVillager,
                papokin_data::particle::Particle::HappyVillager,
            ),
            (
                papokin::plugin::particles::Particle::SulfurCubeGoo,
                papokin_data::particle::Particle::SulfurCubeGoo,
            ),
        ];

        for (wit, internal) in cases {
            assert_eq!(
                papokin_data::particle::Particle::from_id(wit as u16),
                Some(internal)
            );
        }
    }
}
