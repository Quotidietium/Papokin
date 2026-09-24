pub mod advancement;
pub mod statistics;

use core::f32;
use std::collections::{HashMap, VecDeque};
use std::f64::consts::TAU;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, AtomicI8, AtomicI32, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use crate::entity::attributes::{Modifier, ModifierOperation};
use crate::plugin::api::events::enchantment::{EnchantItemEvent, PrepareItemEnchantEvent};
use crate::world::scoreboard::Scoreboard;
use advancement::PlayerAdvancement;
use arc_swap::ArcSwap;
use crossbeam::atomic::AtomicCell;
use crossbeam::channel::Receiver;
use crossbeam::queue::SegQueue;
use papokin_data::dimension::Dimension;
use papokin_inventory::Inventory;
use papokin_inventory::merchant::merchant_screen_handler::MerchantScreenHandler;
use papokin_inventory::player::ender_chest_inventory::EnderChestInventory;
use papokin_protocol::RawPacket;
use papokin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use papokin_util::version::JavaMinecraftVersion;
use papokin_world::chunk::ChunkData;
use tokio::task::JoinHandle;
use tracing::{debug, warn};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum CustomScoreboard {
    Java(Scoreboard),
}

impl From<Scoreboard> for CustomScoreboard {
    fn from(sb: Scoreboard) -> Self {
        Self::Java(sb)
    }
}

#[derive(Copy, Clone)]
pub struct JavaPlayer<'a>(pub &'a Player);

impl JavaPlayer<'_> {
    pub async fn send_packet<C: papokin_protocol::ClientPacket + Sync>(&self, packet: &C) {
        let client = &self.0.client;
        if let Ok(data) = client.serialize_packet(packet) {
            client.enqueue_packet(data).await;
        }
    }

    pub async fn send_custom_payload(&self, channel: &str, data: &[u8]) {
        let packet = CCustomPayload::new(channel, data);
        self.send_packet(&packet).await;
    }

    pub fn send_stats(&self) {
        self.0.send_stats();
    }

    pub fn set_scoreboard(&self, scoreboard: Option<Scoreboard>) {
        *self
            .0
            .custom_scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            scoreboard.map(CustomScoreboard::Java);
        self.0.send_scoreboard();
    }

    pub fn reset_scoreboard(&self) {
        self.set_scoreboard(None);
    }

    pub fn get_scoreboard(&self) -> Option<Scoreboard> {
        let guard = self
            .0
            .custom_scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(CustomScoreboard::Java(sb)) = guard.as_ref() {
            Some(sb.clone())
        } else {
            None
        }
    }
}
use papokin_data::attributes::Attributes;
use papokin_data::block_properties::HorizontalFacing;
use papokin_data::damage::DamageType;
use papokin_data::data_component_impl::{AttributeModifiersImpl, EnchantmentsImpl, Operation};
use papokin_data::data_component_impl::{EquipmentSlot, EquippableImpl, ToolImpl, WeaponImpl};
use papokin_data::effect::StatusEffect;
use papokin_data::entity::{EntityPose, EntityStatus, EntityType};
use papokin_data::item_stack::ItemStack;
use papokin_data::particle::Particle;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_data::statistic::StatisticCategory;
use papokin_data::tag::Taggable;
use papokin_data::{Block, BlockState, Enchantment, screen::WindowType, tag, translation};
use papokin_inventory::player::{
    player_inventory::PlayerInventory, player_screen_handler::PlayerScreenHandler,
};
use papokin_inventory::screen_handler::{
    ClickType, InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour, ScreenHandlerFactory,
    ScreenHandlerListener,
};
use papokin_inventory::sync_handler::SyncHandler;
use papokin_macros::send_cancellable;
use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;
use papokin_protocol::IdOr;
use papokin_protocol::PositionFlag;
use papokin_protocol::SoundEvent;
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::{
    Animation, CAcknowledgeBlockChange, CActionBar, CAwardStats, CBlockUpdate, CChangeDifficulty,
    CCloseContainer, CCombatDeath, CCustomPayload, CDisguisedChatMessage, CEntityAnimation,
    CEntityPositionSync, CEntityVelocity, CGameEvent, CHurtAnimation, CItemCooldown, CMapItemData,
    COpenBook, COpenScreen, COpenSignEditor, CParticle, CPlayServerLinks, CPlayerAbilities,
    CPlayerInfoUpdate, CPlayerPosition, CPlayerSpawnPosition, CRespawn, CSetCamera,
    CSetContainerContent, CSetContainerProperty, CSetContainerSlot, CSetCursorItem, CSetExperience,
    CSetHealth, CSetPlayerInventory, CSetSelectedSlot, CSoundEffect, CStopSound, CSubtitle,
    CSystemChatMessage, CTabList, CTitleAnimation, CTitleText, CUnloadChunk, CUpdateMobEffect,
    CUpdateTime, GameEvent, MapIcon, MapPatch, PlayerAction, PlayerInfoFlags, PlayerSpawnData,
    PreviousMessage, Statistic,
};
use papokin_protocol::java::server::play::{
    SClickSlot, SContainerButtonClick, SRenameItem, SlotActionType,
};
use papokin_util::math::{
    boundingbox::BoundingBox, experience, position::BlockPos, vector2::Vector2, vector3::Vector3,
};
use papokin_util::permission::PermissionLvl;
use papokin_util::resource_location::ResourceLocation;
use papokin_util::text::TextComponent;
use papokin_util::text::click::ClickEvent;
use papokin_util::text::hover::HoverEvent;
use papokin_util::{Difficulty, GameMode, Hand};
use papokin_world::biome;
use papokin_world::cylindrical_chunk_iterator::Cylindrical;

use crate::block;
use crate::block::blocks::straw_bed::StrawBedBlock;
use crate::command::context::command_source::CommandSource;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::{CommandSender, client_suggestions};
use crate::data::SaveJSONConfiguration;
use crate::net::java::JavaClient;
use crate::net::{GameProfile, PlayerConfig};
use crate::plugin::player::exp_change::PlayerExpChangeEvent;
use crate::plugin::player::inventory_interact::InventoryClickEvent;
use crate::plugin::player::player_change_world::PlayerChangeWorldEvent;
use crate::plugin::player::player_gamemode_change::PlayerGamemodeChangeEvent;
use crate::plugin::player::player_permission_check::PlayerPermissionCheckEvent;
use crate::plugin::player::player_teleport::PlayerTeleportEvent;
use crate::plugin::server::packet::PacketSentEvent;
use crate::server::Server;
use crate::world::{BlockBreakingProgress, World};
use bytes::Bytes;

use super::breath::BreathManager;
use super::combat::{self, AttackType, player_attack_sound};
use super::hunger::HungerManager;
use super::item::ItemEntity;
use super::living::LivingEntity;
use super::{Entity, EntityBase, NBTStorage, NBTStorageInit, finite_non_negative_f32_or};
use papokin_data::potion::Effect;
const MAX_CACHED_SIGNATURES: u8 = 128; // 原版：128
const MAX_PREVIOUS_MESSAGES: u8 = 20; // 原版：20

fn write_root_vehicle(nbt: &mut NbtCompound, uuid: Uuid) {
    let value = uuid.as_u128();
    let mut root_vehicle = NbtCompound::new();
    root_vehicle.put(
        "Attach",
        NbtTag::IntArray(vec![
            (value >> 96) as i32,
            (value >> 64) as i32,
            (value >> 32) as i32,
            value as i32,
        ]),
    );
    nbt.put("RootVehicle", NbtTag::Compound(root_vehicle));
}

fn read_root_vehicle(nbt: &NbtCompound) -> Option<Uuid> {
    let root_vehicle = nbt.get_compound("RootVehicle")?;
    let uuid = if let Some([most, more, less, least]) = root_vehicle.get_int_array("Attach") {
        [*most, *more, *less, *least]
    } else {
        let [most, more, less, least] = root_vehicle.get_list("Attach")? else {
            return None;
        };
        [
            most.extract_int()?,
            more.extract_int()?,
            less.extract_int()?,
            least.extract_int()?,
        ]
    };

    Some(Uuid::from_u128(
        (uuid[0] as u32 as u128) << 96
            | (uuid[1] as u32 as u128) << 64
            | (uuid[2] as u32 as u128) << 32
            | uuid[3] as u32 as u128,
    ))
}

pub const DATA_VERSION: i32 = 4671; // 1.21.11

/// 玩家每挖掘一个方块所增加的食物疲劳值。
///
/// 原版：`Block#playerDestroy` 调用 `player.causeFoodExhaustion(0.005F)`。
/// `ServerPlayerGameMode#destroyBlock` 仅对
/// 非创造模式且手持能采收该方块工具的玩家，因此调用方
/// 必须应用相同的门控逻辑。
pub const MINE_BLOCK_EXHAUSTION: f32 = 0.005; // 原版：0.005F

/// 表示一个 Minecraft 玩家实体。
///
/// `Player` 是一种特殊的实体，表示连接到服务器的真人玩家。
#[derive(Clone, Copy, Debug)]
pub struct ItemCooldown {
    pub start_tick: i32,
    pub duration: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerWeather {
    Clear,
    Downfall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpamType {
    Chat,
    Command,
}

pub struct Player {
    /// 代表玩家的底层生物实体对象。
    pub living_entity: LivingEntity,
    /// 玩家的游戏档案信息，包括用户名和 UUID。
    pub gameprofile: GameProfile,
    /// 与玩家关联的客户端连接。
    pub client: Arc<JavaClient>,
    /// 玩家的物品栏。
    pub inventory: Arc<PlayerInventory>,
    /// 玩家的 `EnderChest` 物品栏。
    pub ender_chest_inventory: Arc<EnderChestInventory>,
    /// 玩家的配置设置。当玩家更改其设置时会变化。
    pub config: ArcSwap<PlayerConfig>,
    /// 玩家当前的游戏模式（例如生存、创造、冒险）。
    pub gamemode: AtomicCell<GameMode>,
    /// 玩家之前的游戏模式
    pub previous_gamemode: AtomicCell<Option<GameMode>>,
    /// 玩家当前正在旁观/镜头所对准的实体的实体 ID。
    pub camera_target_id: AtomicCell<Option<i32>>,
    /// 玩家的生成点
    pub respawn_point: std::sync::Mutex<Option<RespawnPoint>>,
    /// 玩家的睡眠状态
    pub sleeping_since: AtomicCell<Option<u8>>,
    /// 玩家当前所睡床的床头位置。
    pub sleeping_bed_pos: AtomicCell<Option<BlockPos>>,
    /// 管理玩家的呼吸值
    pub breath_manager: BreathManager,
    /// 管理玩家的饥饿值。
    pub hunger_manager: HungerManager,
    /// 当前打开的容器 ID（如有）。
    pub open_container: AtomicCell<Option<u64>>,
    /// 当前打开的容器界面的方块位置（如有）。
    pub open_container_pos: AtomicCell<Option<BlockPos>>,
    /// 触发袭击之兆（Raid Omen）的村庄位置。
    pub raid_omen_position: AtomicCell<Option<BlockPos>>,
    /// 玩家当前正在持有的物品。
    pub carried_item: Mutex<Option<ItemStack>>,
    /// 玩家的能力与特殊技能。
    ///
    /// 此字段表示玩家拥有的各种能力，例如飞行、无敌以及其他特殊效果。
    ///
    /// **注意：** 当 `abilities` 字段更新时，服务器应向客户端发送 `send_abilities_update` 数据包，以通知其变更。
    pub abilities: std::sync::Mutex<Abilities>,
    /// 玩家统计信息
    pub stats: std::sync::Mutex<statistics::Statistics>,
    /// 玩家正在破坏的方块的当前破坏阶段。
    pub current_block_destroy_stage: AtomicI32,
    /// 上次发送给客户端的每刻方块破坏进度。
    pub current_block_breaking_speed: AtomicU32,
    /// 所持物品的效率附魔等级，最近一次通过 `mining_efficiency` 同步到客户端
    /// 属性。-1 表示从不同步
    pub synced_mining_efficiency_level: AtomicI32,
    /// 表示玩家当前是否正在挖掘方块。
    pub mining: AtomicBool,
    pub start_mining_time: AtomicI32,
    pub tick_counter: AtomicI32,
    pub mining_pos: Mutex<BlockPos>,
    pub last_input: AtomicI8,
    /// 传送 ID 的计数器，用于跟踪待处理的传送。
    pub teleport_id_count: AtomicI32,
    /// 待处理的传送信息，包括传送 ID 和目标位置。
    pub awaiting_teleport: Mutex<Option<(VarInt, Vector3<f64>)>>,
    /// 玩家当前正在查看的区块段的坐标。
    pub watched_section: AtomicCell<Cylindrical>,
    /// 玩家上次执行操作的时间（用于空闲超时判断）。
    pub last_action_time: AtomicCell<Instant>,
    /// 以毫秒为单位的延迟。
    pub ping: AtomicU32,
    /// 距玩家上次攻击经过的刻数。
    pub last_attacked_ticks: AtomicU32,
    /// 玩家最后已知的经验等级。
    pub last_sent_xp: AtomicI32,
    pub last_sent_health: AtomicI32,
    pub last_sent_food: AtomicU8,
    pub last_food_saturation: AtomicBool,
    /// 玩家的权限等级。
    pub permission_lvl: AtomicCell<PermissionLvl>,
    pub subscribed_debug_sample: AtomicBool,
    /// 客户端是否已报告其已加载。
    pub client_loaded: AtomicBool,
    /// 玩家是否被固定在原地（用于对话/过场动画的移动锁定）。
    pub is_movement_locked: AtomicBool,
    /// 客户端在被判定超时之前可用于报告已完成加载的时间（以刻为单位）。
    pub client_loaded_timeout: AtomicU32,
    /// 用于追踪聊天和命令刷屏的计数器。每个服务器刻衰减一次。
    pub chat_spam_tick_count: AtomicU32,
    /// 弓、弩等物品的使用跟踪。
    pub using_item: AtomicBool,
    pub item_use_start_time: AtomicI32,
    pub using_hand: AtomicCell<Option<Hand>>,
    /// 玩家的经验等级。
    pub experience_level: AtomicI32,
    /// 玩家的经验进度（`0.0` 到 `1.0`）
    pub experience_progress: AtomicCell<f32>,
    /// 玩家的总经验值。
    pub experience_points: AtomicI32,
    pub item_cooldowns: std::sync::Mutex<HashMap<String, ItemCooldown>>,
    pub experience_pick_up_delay: Mutex<u32>,
    pub chunk_sender: Mutex<crate::net::ChunkSender>,
    pub chunk_listener: Mutex<Receiver<(Vector2<i32>, Weak<ChunkData>)>>,
    /// 区块网络编码缓存（按区块位置复用序列化字节），跨 tick 持久。
    /// 条目以对区块数据的弱引用判新鲜度；容量超限时整体清空兜底。
    pub chunk_encode_cache: Mutex<rustc_hash::FxHashMap<Vector2<i32>, crate::net::EncodedChunk>>,
    pub held_chunk_tickets: Mutex<Option<(Option<i8>, Option<i8>)>>,
    pub chunk_send_epoch: AtomicU32,
    pub has_played_before: AtomicBool,
    root_vehicle_uuid: AtomicCell<Option<Uuid>>,
    pub chat_session: Arc<Mutex<ChatSession>>,
    pub signature_cache: Mutex<MessageCache>,
    pub player_screen_handler: Arc<std::sync::Mutex<PlayerScreenHandler>>,
    pub current_screen_handler: std::sync::Mutex<Arc<std::sync::Mutex<dyn ScreenHandler>>>,
    pub screen_handler_sync_id: AtomicU8,
    pub screen_handler_listener: Arc<dyn ScreenHandlerListener>,
    pub inventory_changed: Arc<AtomicBool>,
    pub screen_handler_sync_handler: Arc<SyncHandler>,
    pub tab_list_header: Mutex<TextComponent>,
    pub tab_list_footer: Mutex<TextComponent>,
    pub display_name: std::sync::Mutex<Option<TextComponent>>,
    pub tab_list_name: Mutex<Option<TextComponent>>,
    pub tab_list_order: AtomicI32,
    pub tab_list_latency: AtomicI32,
    pub tab_list_listed: AtomicBool,
    pub per_player_time: AtomicCell<Option<(u64, bool)>>,
    pub per_player_weather: AtomicCell<Option<PlayerWeather>>,
    pub custom_scoreboard: std::sync::Mutex<Option<CustomScoreboard>>,
    pub compass_target: AtomicCell<Option<papokin_util::math::position::BlockPos>>,
    pub respawn_location: AtomicCell<Option<papokin_util::math::position::BlockPos>>,
    pub hidden_players: Mutex<std::collections::HashSet<uuid::Uuid>>,
    pub advancements: Arc<Mutex<PlayerAdvancement>>,
    pub enchantment_seed: AtomicI32,
    pub fishing_bobber: AtomicI32,
    pub seen_credits: AtomicBool,
    pub score: AtomicI32,
    pub spawn_extra_particles_on_fall: AtomicBool,
    /// 玩家刻期间等待处理的入站数据包。
    pub inbound_packets: SegQueue<RawPacket>,
}

impl Player {
    #[expect(clippy::too_many_lines, clippy::items_after_statements)]
    pub fn new(
        client: Arc<JavaClient>,
        gameprofile: GameProfile,
        config: PlayerConfig,
        world: &Arc<World>,
        gamemode: GameMode,
    ) -> Self {
        let inventory_changed = Arc::new(AtomicBool::new(true));

        struct ScreenListener {
            inventory_changed: Arc<AtomicBool>,
            world: Weak<World>,
            player_uuid: Uuid,
        }

        impl ScreenHandlerListener for ScreenListener {
            fn on_slot_update(
                &self,
                screen_handler: &ScreenHandlerBehaviour,
                slot: u8,
                stack: ItemStack,
            ) {
                self.inventory_changed.store(true, Ordering::Relaxed);

                // 物品栏槽位变更钩子：高频通知，
                // 在做任何工作前先确认确实存在监听器。
                let Some(world) = self.world.upgrade() else {
                    return;
                };
                let Some(server) = world.server.upgrade() else {
                    return;
                };
                if !server
                    .plugin_manager
                    .has_handlers::<crate::plugin::api::events::player::player_inventory_slot_change::PlayerInventorySlotChangeEvent>()
                {
                    return;
                }
                // 仅玩家物品栏槽位（位于容器自身槽位之后的
                // 槽位）都会被上报。
                let slot_index = slot as usize;
                if slot_index < screen_handler.container_slots {
                    return;
                }
                let Some(player) = world.get_player_by_uuid(self.player_uuid) else {
                    return;
                };
                let old_item = screen_handler
                    .previous_tracked_stacks
                    .get(slot_index)
                    .and_then(|tracked| tracked.received_stack.clone());
                let new_item = if stack.is_empty() { None } else { Some(stack) };
                let mut slot_event = crate::plugin::api::events::player::player_inventory_slot_change::PlayerInventorySlotChangeEvent::new(
                    player,
                    i32::from(slot),
                    old_item,
                    new_item,
                );
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut slot_event);
            }
        }

        let server = world.server.upgrade().unwrap_or_else(|| {
            tracing::error!("服务器已停用");
            std::process::exit(1);
        });

        let player_uuid = gameprofile.id;

        let living_entity = LivingEntity::new(Entity::from_uuid(
            player_uuid,
            world.clone(),
            Vector3::new(0.0, 100.0, 0.0),
            &EntityType::PLAYER,
        ));
        living_entity.entity.invulnerable.store(
            matches!(gamemode, GameMode::Creative | GameMode::Spectator),
            Ordering::Relaxed,
        );
        living_entity
            .entity
            .no_physics
            .store(gamemode == GameMode::Spectator, Ordering::Relaxed);
        if gamemode == GameMode::Spectator {
            living_entity
                .entity
                .on_ground
                .store(false, Ordering::Relaxed);
        }

        let inventory = Arc::new(PlayerInventory::new(
            living_entity.entity_equipment.clone(),
            living_entity.equipment_slots.clone(),
        ));

        let ender_chest_inventory = Arc::new(EnderChestInventory::new());

        let player_screen_handler = Arc::new(std::sync::Mutex::new(PlayerScreenHandler::new(
            &inventory,
            None,
            0,
            Some(server.recipe_manager.clone()),
        )));

        // 根据游戏模式初始化能力（类似原版的 GameMode.setAbilities()）
        let mut abilities = Abilities::default();
        abilities.set_for_gamemode(gamemode);

        let supports_player_loaded = client.version.load() >= JavaMinecraftVersion::V_1_21_4;
        let initially_loaded = !supports_player_loaded;

        Self {
            living_entity,
            config: ArcSwap::new(Arc::new(config)),
            advancements: Arc::new(Mutex::new(
                server
                    .advancement_manager
                    .clone()
                    .new_player_advancement(gameprofile.id),
            )),
            gameprofile,
            client,
            awaiting_teleport: Mutex::new(None),
            breath_manager: BreathManager::default(),
            // TODO: 从上一个实例加载此项
            hunger_manager: HungerManager::default(),
            current_block_destroy_stage: AtomicI32::new(-1),
            current_block_breaking_speed: AtomicU32::new(0),
            synced_mining_efficiency_level: AtomicI32::new(-1),
            enchantment_seed: AtomicI32::new(rand::random()),
            open_container: AtomicCell::new(None),
            open_container_pos: AtomicCell::new(None),
            raid_omen_position: AtomicCell::new(None),
            tick_counter: AtomicI32::new(0),
            start_mining_time: AtomicI32::new(0),
            last_input: AtomicI8::new(0),
            carried_item: Mutex::new(None),
            experience_pick_up_delay: Mutex::new(0),
            teleport_id_count: AtomicI32::new(0),
            mining: AtomicBool::new(false),
            mining_pos: Mutex::new(BlockPos::ZERO),
            abilities: std::sync::Mutex::new(abilities),
            stats: std::sync::Mutex::new(statistics::Statistics::default()),
            gamemode: AtomicCell::new(gamemode),
            previous_gamemode: AtomicCell::new(None),
            camera_target_id: AtomicCell::new(None),
            is_movement_locked: AtomicBool::new(false),
            // TODO: 客户端连接时发送携带正确数值的 CPlayerSpawnPosition 数据包
            respawn_point: std::sync::Mutex::new(None),
            sleeping_since: AtomicCell::new(None),
            sleeping_bed_pos: AtomicCell::new(None),
            // 我们希望它成为一个不可能被观察的区块段，使 `chunker::update_position`
            // 会把区块标记为因新加入而非重生而被观看。
            // （我们左移一位，以便围绕该区块进行搜索）
            watched_section: AtomicCell::new(Cylindrical::new(
                Vector2::new(0, 0),
                // 由于 1 在原版中不可能出现，因此它被用作未初始化标记
                NonZero::new(1).unwrap_or(NonZero::<u8>::MIN),
            )),
            last_action_time: AtomicCell::new(std::time::Instant::now()),
            ping: AtomicU32::new(0),
            last_attacked_ticks: AtomicU32::new(0),
            client_loaded: AtomicBool::new(initially_loaded),
            client_loaded_timeout: AtomicU32::new(if initially_loaded { 0 } else { 60 }),
            chat_spam_tick_count: AtomicU32::new(0),
            // 物品使用追踪
            using_item: AtomicBool::new(false),
            item_use_start_time: AtomicI32::new(0),
            using_hand: AtomicCell::new(None),
            // Minecraft 无法更改新玩家的默认权限等级。
            // Minecraft 的默认权限等级为 0。
            permission_lvl: server
                .data
                .operator_config
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get_entry(&player_uuid)
                .map_or(
                    AtomicCell::new(server.advanced_config.commands.default_op_level),
                    |op| AtomicCell::new(op.level),
                ),
            inventory,
            ender_chest_inventory,
            experience_level: AtomicI32::new(0),
            experience_progress: AtomicCell::new(0.0),
            experience_points: AtomicI32::new(0),
            item_cooldowns: std::sync::Mutex::new(HashMap::new()),
            chunk_sender: Mutex::new({
                let mut sender = crate::net::ChunkSender::new();
                sender.set_owner(world, player_uuid);
                sender
            }),
            chunk_listener: Mutex::new(world.level.chunk_listener.add_global_chunk_listener()),
            chunk_encode_cache: Mutex::new(rustc_hash::FxHashMap::default()),
            held_chunk_tickets: Mutex::new(None),
            chunk_send_epoch: AtomicU32::new(0),
            last_sent_xp: AtomicI32::new(-1),
            last_sent_health: AtomicI32::new(-1),
            last_sent_food: AtomicU8::new(0),
            last_food_saturation: AtomicBool::new(true),
            subscribed_debug_sample: AtomicBool::new(false),
            has_played_before: AtomicBool::new(false),
            root_vehicle_uuid: AtomicCell::new(None),
            chat_session: Arc::new(Mutex::new(ChatSession::default())), // 在玩家真正设置其会话 ID 之前的占位值
            signature_cache: Mutex::new(MessageCache::default()),
            player_screen_handler: player_screen_handler.clone(),
            current_screen_handler: std::sync::Mutex::new(player_screen_handler),
            screen_handler_sync_id: AtomicU8::new(0),
            screen_handler_listener: Arc::new(ScreenListener {
                inventory_changed: inventory_changed.clone(),
                world: Arc::downgrade(world),
                player_uuid,
            }),
            inventory_changed,
            screen_handler_sync_handler: Arc::new(SyncHandler::new()),
            tab_list_header: Mutex::new(TextComponent::text("")),
            tab_list_footer: Mutex::new(TextComponent::text("")),
            display_name: std::sync::Mutex::new(None),
            tab_list_name: Mutex::new(None),
            tab_list_order: AtomicI32::new(0),
            tab_list_latency: AtomicI32::new(0),
            tab_list_listed: AtomicBool::new(true),
            per_player_time: AtomicCell::new(None),
            per_player_weather: AtomicCell::new(None),
            custom_scoreboard: std::sync::Mutex::new(None),
            compass_target: AtomicCell::new(None),
            respawn_location: AtomicCell::new(None),
            hidden_players: Mutex::new(std::collections::HashSet::new()),
            fishing_bobber: AtomicI32::new(-1),
            seen_credits: AtomicBool::new(false),
            score: AtomicI32::new(0),
            spawn_extra_particles_on_fall: AtomicBool::new(false),
            inbound_packets: SegQueue::new(),
        }
    }

    /// 设置 Tab 列表页眉与页脚。
    pub fn set_tab_list(&self, tab_list: impl Into<crate::plugin::api::tab_list::TabList>) {
        let list = tab_list.into();
        self.set_tab_list_header_footer(&list.header, &list.footer);
    }

    pub fn set_tab_list_header_footer(&self, header: &TextComponent, footer: &TextComponent) {
        *self
            .tab_list_header
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = header.clone();
        *self
            .tab_list_footer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = footer.clone();
        self.try_send_client_packet(&CTabList::new(header, footer));
    }

    pub fn start_cooldown(&self, group: String, duration: i32) {
        // 冷却钩子：group 变体携带原始冷却组；
        // 物品变体的事件也会触发（本引擎以
        // 组（对于普通物品冷却而言即物品 id）。
        let world = self.world();
        let server_player = world
            .server
            .upgrade()
            .zip(world.get_player_by_uuid(self.gameprofile.id));
        if let Some((server, player_arc)) = server_player {
            let mut group_event =
                crate::plugin::api::events::player::player_item_group_cooldown::PlayerItemGroupCooldownEvent::new(
                    player_arc.clone(),
                    group.clone(),
                    duration,
                );
            server
                .plugin_manager
                .fire_blocking(&server, &mut group_event);
            if group_event.cancelled {
                return;
            }
            let duration = group_event.cooldown;

            let mut item_event =
                crate::plugin::api::events::player::player_item_cooldown::PlayerItemCooldownEvent::new(
                    player_arc,
                    group.clone(),
                    duration,
                );
            server
                .plugin_manager
                .fire_blocking(&server, &mut item_event);
            if item_event.cancelled {
                return;
            }
            let duration = item_event.cooldown;

            self.insert_cooldown(group, duration);
            return;
        }

        self.insert_cooldown(group, duration);
    }

    fn insert_cooldown(&self, group: String, duration: i32) {
        let mut cooldowns = self
            .item_cooldowns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cooldowns.insert(
            group.clone(),
            ItemCooldown {
                start_tick: self.tick_counter.load(Ordering::Relaxed),
                duration,
            },
        );
        self.try_send_client_packet(&CItemCooldown::new(group, VarInt(duration)));
    }

    pub fn get_cooldown(&self, group: &str) -> f32 {
        let cooldowns = self
            .item_cooldowns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cooldown) = cooldowns.get(group) {
            let current_tick = self.tick_counter.load(Ordering::Relaxed);
            let elapsed = current_tick - cooldown.start_tick;
            if elapsed < cooldown.duration {
                return 1.0 - (elapsed as f32 / cooldown.duration as f32);
            }
        }
        0.0
    }

    pub fn is_on_cooldown(&self, group: &str) -> bool {
        let mut cooldowns = self
            .item_cooldowns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cooldown) = cooldowns.get(group) {
            let current_tick = self.tick_counter.load(Ordering::Relaxed);
            if current_tick - cooldown.start_tick < cooldown.duration {
                return true;
            }
            cooldowns.remove(group);
        }
        false
    }

    pub fn set_display_name(&self, display_name: Option<TextComponent>) {
        let mut guard = self
            .display_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = display_name;
        // 为所有人更新 Tab 列表
        let world = self.world();
        world.broadcast_packet_all(&CPlayerInfoUpdate::new(
            PlayerInfoFlags::UPDATE_DISPLAY_NAME.bits(),
            &[papokin_protocol::java::client::play::Player {
                uuid: self.gameprofile.id,
                actions: &[PlayerAction::UpdateDisplayName(guard.as_ref())],
            }],
        ));
    }

    pub fn get_tab_list_name(&self) -> Option<TextComponent> {
        self.tab_list_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_tab_list_name(&self, name: Option<TextComponent>) {
        let mut guard = self
            .tab_list_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = name;
        let world = self.world();
        world.broadcast_packet_all(&CPlayerInfoUpdate::new(
            PlayerInfoFlags::UPDATE_DISPLAY_NAME.bits(),
            &[papokin_protocol::java::client::play::Player {
                uuid: self.gameprofile.id,
                actions: &[PlayerAction::UpdateDisplayName(guard.as_ref())],
            }],
        ));
    }

    pub fn set_tab_list_order(&self, order: i32) {
        self.tab_list_order.store(order, Ordering::Relaxed);
        let world = self.world();
        world.broadcast_packet_all(&CPlayerInfoUpdate::new(
            PlayerInfoFlags::UPDATE_LIST_PRIORITY.bits(),
            &[papokin_protocol::java::client::play::Player {
                uuid: self.gameprofile.id,
                actions: &[PlayerAction::UpdateListOrder(VarInt(order))],
            }],
        ));
    }

    pub fn set_tab_list_latency(&self, latency: i32) {
        self.tab_list_latency.store(latency, Ordering::Relaxed);
        let world = self.world();
        world.broadcast_packet_all(&CPlayerInfoUpdate::new(
            PlayerInfoFlags::UPDATE_LATENCY.bits(),
            &[papokin_protocol::java::client::play::Player {
                uuid: self.gameprofile.id,
                actions: &[PlayerAction::UpdateLatency(VarInt(latency))],
            }],
        ));
    }

    pub fn set_tab_list_listed(&self, listed: bool) {
        self.tab_list_listed.store(listed, Ordering::Relaxed);
        let world = self.world();
        world.broadcast_packet_all(&CPlayerInfoUpdate::new(
            PlayerInfoFlags::UPDATE_LISTED.bits(),
            &[papokin_protocol::java::client::play::Player {
                uuid: self.gameprofile.id,
                actions: &[PlayerAction::UpdateListed(listed)],
            }],
        ));
    }

    /// 生成与此玩家客户端关联的任务。使用此方法生成的所有任务都会被等待
    /// 当客户端断开连接时。这意味着任务应在合理的时间内完成，或者选择
    /// 依赖 `Self::await_close_interrupt` 在客户端关闭时取消任务
    ///
    ///返回 `Option<JoinHandle<F::Output>>`。若客户端已关闭，则返回 `None`。
    pub fn spawn_task<F>(&self, task: F) -> Option<JoinHandle<F::Output>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.client.spawn_task(task)
    }

    /// 将刚为某个玩家排入队列的区块中的已跟踪实体进行配对。
    fn pair_entities_in_chunks(
        &self,
        world: &crate::world::World,
        chunks: &[papokin_util::math::vector2::Vector2<i32>],
    ) {
        if chunks.is_empty() {
            return;
        }
        if let Some(player) = world.get_player_by_uuid(self.gameprofile.id) {
            world
                .entity_tracker
                .update_player_chunks(&player, world, chunks);
        }
    }

    pub const fn inventory(&self) -> &Arc<PlayerInventory> {
        &self.inventory
    }

    pub const fn ender_chest_inventory(&self) -> &Arc<EnderChestInventory> {
        &self.ender_chest_inventory
    }

    /// 打开玩家的末影箱界面。
    pub fn open_ender_chest(self: &Arc<Self>) -> Option<u8> {
        self.increment_stat(
            papokin_data::statistic::StatisticCategory::Custom,
            papokin_data::statistic::CustomStatistic::OpenEnderchest as i32,
            1,
        );
        let inventory = self.ender_chest_inventory();
        self.open_handled_screen(
            &crate::block::blocks::ender_chest::EnderChestScreenFactory {
                inventory: inventory.clone(),
                tracker: None,
            },
            None,
        )
    }

    /// 将 [`Player`] 从当前 [`World`] 中移除。
    pub async fn remove(self: &Arc<Self>) {
        if !self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_any()
            .is::<PlayerScreenHandler>()
        {
            self.on_handled_screen_closed();
        }

        let vehicle = self
            .living_entity
            .entity
            .vehicle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(vehicle) = vehicle {
            self.root_vehicle_uuid
                .store(Some(vehicle.get_entity().entity_uuid));
            vehicle
                .get_entity()
                .remove_passenger_on_disconnect(self.entity_id());
        }

        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .increment_custom(statistics::CustomStatistic::LeaveGame, 1);
        let world = self.world();
        world.remove_player(self, true).await;

        let cylindrical = self.watched_section.load();
        self.clean_up_chunk_tickets(&world.level);
        if let Ok(mut sender) = self.chunk_sender.lock() {
            sender.reset();
        }

        // 径向区块是玩家理论上正在观看的所有区块。
        // 给定足够时间，所有这些区块都会在内存中。
        let radial_chunks = cylindrical.all_chunks_within();

        debug!(
            "正在移除玩家 {}，取消监视 {} 个区块",
            self.gameprofile.name,
            radial_chunks.len()
        );

        let level = &world.level;

        // 递减被监视区块的值
        let chunks_to_clean = level.mark_chunks_as_not_watched(radial_chunks).await;
        // 从缓存中移除没有观察者的区块
        if !chunks_to_clean.is_empty() {
            world.remove_entities_in_chunks(&chunks_to_clean).await;
            level.clean_entity_chunks(&chunks_to_clean);
        }
        // 从所有可能已加载的区块中移除残留条目
        let cleaned_chunks = level.clean_memory();
        if !cleaned_chunks.is_empty() {
            world.remove_entities_in_chunks(&cleaned_chunks).await;
            level.clean_entity_chunks(&cleaned_chunks);
        }

        debug!(
            "已将玩家 {} 从世界 {} 移除（仍缓存 {} 个区块）",
            self.gameprofile.name,
            self.world().get_world_name(),
            level.loaded_chunk_count(),
        );

        //self.world().level.list_cached();
    }

    pub(crate) fn try_restore_vehicle(self: &Arc<Self>, vehicle: &Arc<dyn EntityBase>) {
        // 以原子方式认领 UUID，否则将其置空
        // 在不匹配的交换与恢复之间。
        if self
            .root_vehicle_uuid
            .compare_exchange(Some(vehicle.get_entity().entity_uuid), None)
            .is_err()
        {
            return;
        }

        vehicle
            .get_entity()
            .add_passenger(vehicle.clone(), self.clone());
    }

    pub fn clean_up_chunk_tickets(&self, level: &Arc<papokin_world::level::Level>) {
        let mut lock = level
            .chunk_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let held = self
            .held_chunk_tickets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some((view_level, sim_level)) = held {
            let center = self.get_entity().chunk_pos.load();
            if let Some(view) = view_level {
                lock.remove_ticket(center, view);
            }
            if let Some(sim) = sim_level {
                lock.remove_ticket(center, sim);
            }
        }
        lock.send_change();
        level.should_unload.store(true, Ordering::Relaxed);
        level.level_channel.notify();
    }

    pub fn update_chunk_tickets_for_gamemode(self: &Arc<Self>) {
        crate::world::chunker::update_position(self);
    }

    pub fn change_world_chunks(
        &self,
        old_level: &Arc<papokin_world::level::Level>,
        new_world: &Arc<crate::world::World>,
    ) {
        self.clean_up_chunk_tickets(old_level);
        if let Ok(mut listener) = self.chunk_listener.lock() {
            *listener = new_world.level.chunk_listener.add_global_chunk_listener();
        }
        self.chunk_send_epoch.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut sender) = self.chunk_sender.lock() {
            sender.reset();
        }
        // 旧世界的编码缓存条目已全部作废，直接清空，
        // 不必等容量兜底触发。
        if let Ok(mut cache) = self.chunk_encode_cache.lock() {
            cache.clear();
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn attack(&self, victim: &Arc<dyn EntityBase>) {
        let world = self.world();
        let Some(server) = world.server.upgrade() else {
            return;
        };
        let victim_entity = victim.get_entity();
        let attacker_entity = &self.living_entity.entity;
        let config = &server.advanced_config.pvp;

        // 攻击前钩子：处理器可取消或重定向此次攻击。
        let target_id = victim_entity.entity_id;
        let Some(player_arc) = world.get_player_by_uuid(self.gameprofile.id) else {
            return;
        };
        let mut pre_attack =
            crate::plugin::api::events::player::pre_player_attack_entity::PrePlayerAttackEntityEvent::new(
                player_arc.clone(),
                target_id,
            );
        server
            .plugin_manager
            .fire_blocking(&server, &mut pre_attack);
        if pre_attack.cancelled {
            return;
        }

        let inventory = self.inventory();
        let item_stack = inventory.held_item();
        if !item_stack.is_empty() {
            self.increment_stat(
                statistics::StatisticCategory::Used,
                item_stack.item.id as i32,
                1,
            );
        }

        let base_damage = self
            .living_entity
            .get_attribute_value(&Attributes::ATTACK_DAMAGE);
        let base_attack_speed = 4.0;

        let mut damage_multiplier = 1.0;
        let mut add_damage = 0.0;
        let mut add_speed = 0.0;
        let mut extra_ench_damage = 0.0;
        let mut knockback_level = 0u32;

        {
            let stack = &item_stack;
            if stack.is_empty() {
                // 原版空手：base_attack_speed = -2.4
                add_speed = -2.4;
            } else if let Some(modifiers) = stack.get_data_component::<AttributeModifiersImpl>() {
                for item_mod in modifiers.attribute_modifiers.iter() {
                    if item_mod.operation == Operation::AddValue {
                        if item_mod.id == "minecraft:base_attack_damage" {
                            add_damage = item_mod.amount;
                        } else if item_mod.id == "minecraft:base_attack_speed" {
                            add_speed = item_mod.amount;
                        }
                    }
                }
            }
            if let Some(enchantments) = stack.get_data_component::<EnchantmentsImpl>() {
                for (enchantment, level) in enchantments.enchantment.iter() {
                    if **enchantment == Enchantment::SHARPNESS {
                        extra_ench_damage += 0.5 * f64::from(*level) + 0.5;
                    } else if **enchantment == Enchantment::SMITE {
                        let target_type = victim_entity.entity_type.id;
                        let is_undead = target_type == EntityType::ZOMBIE.id
                            || target_type == EntityType::DROWNED.id
                            || target_type == EntityType::HUSK.id
                            || target_type == EntityType::ZOMBIE_VILLAGER.id
                            || target_type == EntityType::ZOMBIFIED_PIGLIN.id
                            || target_type == EntityType::SKELETON.id
                            || target_type == EntityType::BOGGED.id
                            || target_type == EntityType::PARCHED.id
                            || target_type == EntityType::WITHER_SKELETON.id
                            || target_type == EntityType::STRAY.id
                            || target_type == EntityType::PHANTOM.id
                            || target_type == EntityType::WITHER.id
                            || target_type == EntityType::ZOMBIE_HORSE.id
                            || target_type == EntityType::SKELETON_HORSE.id;
                        if is_undead {
                            extra_ench_damage += 2.5 * f64::from(*level);
                        }
                    } else if **enchantment == Enchantment::BANE_OF_ARTHROPODS {
                        let target_type = victim_entity.entity_type.id;
                        let is_arthropod = target_type == EntityType::SPIDER.id
                            || target_type == EntityType::CAVE_SPIDER.id
                            || target_type == EntityType::SILVERFISH.id
                            || target_type == EntityType::ENDERMITE.id
                            || target_type == EntityType::BEE.id;
                        if is_arthropod {
                            extra_ench_damage += 2.5 * f64::from(*level);
                        }
                    } else if **enchantment == Enchantment::KNOCKBACK {
                        knockback_level = *level as u32;
                    }
                }
            }
        }

        let attack_speed = base_attack_speed + add_speed;

        let attack_cooldown_progress = self.get_attack_cooldown_progress(
            f64::from(server.basic_config.tps),
            0.5,
            attack_speed,
        );
        // 攻击冷却重置钩子：在攻击重置
        // 冷却；取消则保留之前的冷却值。
        let mut cooldown_reset =
            crate::plugin::api::events::player::player_attack_entity_cooldown_reset::PlayerAttackEntityCooldownResetEvent::new(
                player_arc,
                target_id,
            );
        server
            .plugin_manager
            .fire_blocking(&server, &mut cooldown_reset);
        if !cooldown_reset.cancelled {
            self.last_attacked_ticks.store(0, Ordering::Relaxed);
        }

        // 仅在冷却中时才降低攻击伤害
        // TODO: 附魔也按同样方式折减，只是不做平方。
        if attack_cooldown_progress < 1.0 {
            damage_multiplier = attack_cooldown_progress.powi(2).mul_add(0.8, 0.2);
        }

        // 根据倍率修改附加伤害。
        let mut damage = (base_damage + add_damage) * damage_multiplier;
        damage += extra_ench_damage * attack_cooldown_progress;

        if let Some(strength) = self
            .living_entity
            .get_effect(&papokin_data::effect::StatusEffect::STRENGTH)
        {
            damage += 3.0 * (f64::from(strength.amplifier) + 1.0);
        }
        if let Some(weakness) = self
            .living_entity
            .get_effect(&papokin_data::effect::StatusEffect::WEAKNESS)
        {
            damage -= 4.0 * (f64::from(weakness.amplifier) + 1.0);
        }
        damage = damage.max(0.0);

        let pos = victim_entity.pos.load();
        let attack_type = AttackType::new(self, attack_cooldown_progress as f32);

        if matches!(attack_type, AttackType::Critical) {
            damage *= 1.5;
        }

        let mut is_mace_smash = matches!(attack_type, AttackType::MaceSmash);
        if is_mace_smash {
            // 让插件否决粉碎攻击（其额外伤害和
            // MACE_SMASH 伤害类型）；被拒绝的猛击退化为普通
            // 攻击。`server` 来自上方 pre-attack 事件的作用域。
            let mut smash_event = crate::plugin::api::events::entity::entity_attempt_smash_attack::EntityAttemptSmashAttackEvent::new(
                self.entity_id(),
                victim_entity.entity_id,
                crate::plugin::api::events::entity::entity_attempt_smash_attack::SmashAttackResult::Allowed,
            );
            server
                .plugin_manager
                .fire_blocking(&server, &mut smash_event);
            if smash_event.result
                == crate::plugin::api::events::entity::entity_attempt_smash_attack::SmashAttackResult::Denied
            {
                is_mace_smash = false;
            }
        }
        if is_mace_smash {
            let fall_distance = self.living_entity.fall_distance.load();
            damage += 1.5 * f64::from(fall_distance);
        }

        if !victim.damage_with_context(
            victim.as_ref(),
            damage as f32,
            if is_mace_smash {
                DamageType::MACE_SMASH
            } else {
                DamageType::PLAYER_ATTACK
            },
            None,
            Some(self),
            Some(self),
        ) {
            world.play_sound(
                Sound::EntityPlayerAttackNodamage,
                SoundCategory::Players,
                &self.living_entity.entity.pos.load(),
            );
            return;
        }

        if damage >= 100.0 {
            self.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::DealtOverkillDamage);
        }

        if let Some(enchantments) = item_stack.get_data_component::<EnchantmentsImpl>() {
            for (enchantment, level) in enchantments.enchantment.iter() {
                if **enchantment == Enchantment::FIRE_ASPECT {
                    let mut combust_event =
                        crate::plugin::api::events::entity::entity_combust_by_entity::EntityCombustByEntityEvent::new(
                            victim_entity.entity_id,
                            self.living_entity.entity.entity_id,
                            (*level as u32 * 80) as f32 / 20.0,
                        );
                    if let Some(server) = world.server.upgrade() {
                        server
                            .plugin_manager
                            .fire_blocking(&server, &mut combust_event);
                    }
                    if !combust_event.cancelled {
                        victim_entity
                            .set_on_fire_for_ticks((combust_event.duration * 20.0).max(0.0) as u32);
                    }
                }
            }
        }

        if is_mace_smash {
            let fall_distance = self.living_entity.fall_distance.load();
            self.living_entity.fall_distance.store(0.0);
            world.play_sound(
                if fall_distance > 5.0 {
                    Sound::ItemMaceSmashGroundHeavy
                } else {
                    Sound::ItemMaceSmashGround
                },
                SoundCategory::Players,
                &pos,
            );
        }

        player_attack_sound(&pos, &world, attack_type);

        if matches!(attack_type, AttackType::Critical) {
            let je_packet =
                CEntityAnimation::new(victim_entity.entity_id.into(), Animation::CriticalEffect);
            world.broadcast_packet_all(&je_packet);
        }

        self.living_entity.last_attacking_id.store(
            victim_entity.entity_id,
            std::sync::atomic::Ordering::Relaxed,
        );
        self.living_entity.last_attack_time.store(
            self.living_entity
                .entity
                .age
                .load(std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );

        if victim.get_living_entity().is_some() {
            // 原版 `Player.attack` 会加上 `LivingEntity.getKnockback()`——击退
            // 附魔加成减半 —— 冲刺攻击额外 +0.5，叠加在基础值之上
            // 受害者的伤害处理所施加的击退。普通一击不会附加任何击退。
            // `handle_knockback` 会把 `strength` 减半，因此这些数值是原版的两倍。
            let mut knockback_strength = f64::from(knockback_level);
            match attack_type {
                AttackType::Knockback => knockback_strength += 1.0,
                AttackType::Sweeping => {
                    combat::spawn_sweep_particle(attacker_entity, &world, &pos);

                    let mut sweep_damage = 1.0;
                    if let Some(enchantments) = item_stack.get_data_component::<EnchantmentsImpl>()
                    {
                        for (enchantment, level) in enchantments.enchantment.iter() {
                            if **enchantment == Enchantment::SWEEPING_EDGE {
                                sweep_damage +=
                                    damage as f32 * (*level as f32 / (*level as f32 + 1.0));
                            }
                        }
                    }

                    let search_box = BoundingBox::new(
                        Vector3::new(pos.x - 1.0, pos.y - 0.5, pos.z - 1.0),
                        Vector3::new(pos.x + 1.0, pos.y + 0.5, pos.z + 1.0),
                    );
                    let victims = world.get_all_at_box(&search_box);
                    for other_victim in victims {
                        if other_victim.get_entity().entity_id != victim_entity.entity_id
                            && other_victim.get_entity().entity_id != attacker_entity.entity_id
                        {
                            other_victim.damage_with_context(
                                other_victim.as_ref(),
                                sweep_damage,
                                DamageType::PLAYER_ATTACK,
                                None,
                                Some(self),
                                Some(self),
                            );
                        }
                    }
                }
                _ => {}
            }
            // 原版只在额外击退非零时才推开受击者；
            // `Entity::knockback` 会把当前速度减半，因此用 0.0 调用它
            // 仍会拖慢受害者的速度。
            if config.knockback && knockback_strength > 0.0 {
                combat::handle_knockback(attacker_entity, victim.as_ref(), knockback_strength);
            }
        }

        // NOTE: 单人模式下的 TOCTOU 竞态条件。
        // 在锁定 item_stack 的情况下计算武器消耗（cost = 1 或 2），然后 damage_held_item
        // 重新获取锁。在异步多任务场景下，理论上另一个任务可能
        // 在这些操作之间交换手持物品，导致消耗被应用到错误的物品上。
        // 缓解选项（按优先级顺序）：
        // 1. 创建 damage_held_item_with_lock(&self, item_stack: MutexGuard, amount) 变体
        //    以便在计算和应用期间都持有锁。
        // 2. 将代价计算重构为闭包：damage_held_item(self, |stack| -> i32 { ... })
        // 3. 实践中，单人场景是安全的（这不是多人游戏）。文档
        //    若认为重构过于侵入，则作为已知限制保留。
        self.damage_held_item(Self::combat_weapon_durability_cost(&item_stack));

        // 原版 `Player#attack` 在命中成功分支的末尾执行
        // `causeFoodExhaustion(0.1F)`。只有命中的攻击才造成饱食消耗；未命中/无伤害
        // 该分支已在上方提前返回。
        self.add_exhaustion(0.1);

        if config.swing {}
    }

    /// 返回在战斗中将手持物品用作武器时的耐久度消耗。
    /// 派生自 `Weapon` 数据组件：没有该组件的物品（例如剪刀、工具
    /// 非战斗用途的工具）攻击时不损失耐久度。
    /// 拥有该组件的物品使用其 `item_damage_per_attack` 值（默认为 1；
    /// 斧、镐、锹和锄的该值为 2）。
    fn combat_weapon_durability_cost(stack: &ItemStack) -> i32 {
        stack
            .get_data_component::<WeaponImpl>()
            .map_or(0, |w| w.item_damage_per_attack as i32)
    }

    /// 通过 `CONTAINER_SET_SLOT` 将当前物品栏内容推送给客户端。
    ///
    /// `minecraft:set_player_inventory` 在 1.21.0/1.21.1 上缺失，且
    /// 应用到较新的 1.21.x 客户端的本地快捷栏上。屏幕处理器槽位
    /// 更新走原版路径，（应该）在所有受支持的版本上都可用。
    pub fn sync_inventory_to_client(&self) {
        if let Ok(mut handler) = self.player_screen_handler.try_lock() {
            handler.send_content_updates();
        }

        let Ok(current_guard) = self.current_screen_handler.try_lock() else {
            return;
        };
        let current = current_guard.clone();
        drop(current_guard);

        let player_screen_ptr = Arc::as_ptr(&self.player_screen_handler).cast::<()>();
        let current_ptr = Arc::as_ptr(&current).cast::<()>();
        if player_screen_ptr == current_ptr {
            return;
        }

        if let Ok(mut handler) = current.try_lock() {
            handler.send_content_updates();
        }
    }

    pub fn try_send_slot_set_packet(&self, packet: &CSetPlayerInventory) {
        self.client.try_send_packet(packet);
    }

    pub fn sync_hand_slot(&self, slot_index: usize, stack: ItemStack) {
        self.try_send_slot_set_packet(&CSetPlayerInventory::new(
            (slot_index as i32).into(),
            &ItemStackSerializer::from(stack.clone()),
        ));
        self.sync_inventory_to_client();

        if slot_index == self.inventory.get_selected_slot() as usize {
            self.living_entity
                .send_equipment_changes(&[(EquipmentSlot::MAIN_HAND, stack)]);
        } else if slot_index == PlayerInventory::OFF_HAND_SLOT {
            self.living_entity
                .send_equipment_changes(&[(EquipmentSlot::OFF_HAND, stack)]);
        }
    }

    /// 对 `slot` 中的物品施加 `amount` 点耐久损耗。
    /// 广播 [`EntityStatus`] 破坏事件，并在物品被销毁时同步槽位。
    pub fn damage_item_in_slot(&self, slot: &EquipmentSlot, amount: i32) -> bool {
        if matches!(
            self.gamemode.load(),
            GameMode::Creative | GameMode::Spectator
        ) {
            return false;
        }

        // 直接的 PlayerInventory 槽位索引（与 build_equipment_slots 对应）。
        let slot_index: usize = match slot {
            EquipmentSlot::MainHand(_) => self.inventory.get_selected_slot() as usize,
            EquipmentSlot::OffHand(_) => PlayerInventory::OFF_HAND_SLOT, // 40
            EquipmentSlot::Feet(_) => 36,
            EquipmentSlot::Legs(_) => 37,
            EquipmentSlot::Chest(_) => 38,
            EquipmentSlot::Head(_) => 39,
            // 玩家没有 Body 或 Saddle 装备槽位；
            // 这些仅由非玩家实体使用（例如马）。
            EquipmentSlot::Body(_) | EquipmentSlot::Saddle(_) => return false,
        };

        let mut stack = self.inventory.get_slot(slot_index);
        let original_item = stack.item;
        let result = stack.damage_item(amount);
        let updated = (result != papokin_data::item_stack::DamageResult::Untouched)
            .then_some((result, stack.clone()));

        if let Some((result, updated_stack)) = updated {
            self.inventory.set_slot(slot_index, updated_stack.clone());
            if let Some(server) = self.world().server.upgrade()
                && let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
            {
                let mut event = crate::plugin::api::events::player::player_item_damage::PlayerItemDamageEvent::new(
                    player_arc,
                    original_item.registry_key.to_string(),
                    amount,
                );
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if result == papokin_data::item_stack::DamageResult::Broken {
                if let Some(server) = self.world().server.upgrade()
                    && let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
                {
                    let mut event = crate::plugin::api::events::player::player_item_break::PlayerItemBreakEvent::new(
                        player_arc,
                        original_item.registry_key.to_string(),
                    );
                    server.plugin_manager.fire_blocking(&server, &mut event);
                }
                self.increment_stat(
                    statistics::StatisticCategory::Broken,
                    original_item.id as i32,
                    1,
                );
                self.world().send_entity_status(
                    &self.living_entity.entity,
                    super::equipment_break_status(slot),
                );
            }

            self.try_send_slot_set_packet(&CSetPlayerInventory::new(
                (slot_index as i32).into(),
                &ItemStackSerializer::from(updated_stack.clone()),
            ));
            self.sync_inventory_to_client();

            self.living_entity
                .send_equipment_changes(&[(slot.clone(), updated_stack)]);

            return true;
        }

        false
    }

    /// 检查并触发玩家所穿盔甲上的位置类附魔（例如冰霜行者）。
    pub fn check_location_enchantments(&self, pos: Vector3<f64>, on_ground: bool) {
        if on_ground {
            let boots = self.inventory.get_slot(36);
            if !boots.is_empty() {
                crate::enchantment::EnchantmentHelper::on_location_changed(
                    self.get_entity(),
                    &boots,
                    pos,
                );
            }
        }
    }

    /// 便捷封装——损耗当前手持（主手）的物品。
    pub fn damage_held_item(&self, amount: i32) -> bool {
        self.damage_item_in_slot(&EquipmentSlot::MAIN_HAND, amount)
    }

    pub fn apply_tool_damage_for_block_break(&self, state: &BlockState) {
        if matches!(
            self.gamemode.load(),
            GameMode::Creative | GameMode::Spectator
        ) {
            return;
        }

        if state.hardness <= 0.0 {
            return;
        }

        let damage = self
            .inventory()
            .held_item()
            .get_data_component::<ToolImpl>()
            .map_or(0, |tool| tool.damage_per_block as i32);

        if damage > 0 {
            self.damage_held_item(damage);
        }
    }

    pub fn set_respawn_point(
        &self,
        dimension: Dimension,
        block_pos: BlockPos,
        yaw: f32,
        pitch: f32,
        forced: bool,
    ) -> bool {
        if !forced
            && let Some(respawn_point) = self
                .respawn_point
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
            && dimension == respawn_point.dimension
            && block_pos == respawn_point.position
        {
            return false;
        }

        let mut final_block_pos = block_pos;
        if let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
            && let Some(server) = self.world().server.upgrade()
        {
            let mut event =
                crate::plugin::api::events::player::player_spawn_change::PlayerSpawnChangeEvent {
                    player: player_arc,
                    new_spawn: Some(block_pos),
                    forced,
                    cancelled: false,
                };
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return false;
            }
            if let Some(pos) = event.new_spawn {
                final_block_pos = pos;
            }
        }

        self.client.try_send_packet(&CPlayerSpawnPosition::new(
            final_block_pos,
            yaw,
            pitch,
            dimension.minecraft_name.to_owned(),
        ));

        *self
            .respawn_point
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(RespawnPoint {
            dimension,
            position: final_block_pos,
            yaw,
            force: forced,
        });
        true
    }

    /// 根据已存储的出生点数据计算玩家的重生点。
    ///
    ///若存在有效的重生点，则返回 `Some(CalculatedRespawnPoint)`，否则返回 `None`。
    ///
    /// # Behavior
    /// - 若设置了 `force` 标志（通过 `/spawnpoint` 命令），则验证生成位置是否安全
    ///   (该方块及其上方方块均允许生物生成)。
    /// - 对于床：验证床方块仍然存在，并在其周围找到有效的生成位置。
    /// - 对于重生锚（下界）：验证重生锚具有充能，并找到有效的生成位置。
    /// - 如果生成方块无效/缺失则返回 `None`（调用方应发送
    ///   `NoRespawnBlockAvailable` 游戏事件并改用世界出生点）。
    ///
    /// # Note
    /// 此函数不会发送任何数据包。调用方负责
    /// 若此处返回 `None`，则发送 `NoRespawnBlockAvailable`。
    pub async fn calculate_respawn_point(&self) -> Option<CalculatedRespawnPoint> {
        type BedProperties = papokin_data::block_properties::WhiteBedLikeProperties;
        type AnchorProperties = papokin_data::block_properties::RespawnAnchorLikeProperties;

        let respawn_point = {
            let respawn_guard = self
                .respawn_point
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            respawn_guard.clone()?
        };
        let world = if self.world().dimension == respawn_point.dimension {
            self.world()
        } else if let Some(server) = self.world().server.upgrade() {
            server.get_world_from_dimension(&respawn_point.dimension)
        } else {
            self.world()
        };
        let pos = &respawn_point.position;

        // 确保已获取生成位置周围的区块
        let min_chunk_x = (pos.0.x - 2) >> 4;
        let max_chunk_x = (pos.0.x + 2) >> 4;
        let min_chunk_z = (pos.0.z - 2) >> 4;
        let max_chunk_z = (pos.0.z + 2) >> 4;
        for cx in min_chunk_x..=max_chunk_x {
            for cz in min_chunk_z..=max_chunk_z {
                world
                    .level
                    .get_or_fetch_chunk(Vector2::new(cx, cz), |_| ())
                    .await;
            }
        }

        let (block, state_id) = world.get_block_and_state_id(pos);

        // 如果设置了 force（来自 /spawnpoint 命令），则校验位置是否安全
        if respawn_point.force {
            // 对于强制生成，检查方块及其上方方块是否都允许生物生成
            let block_state = world.get_block_state(pos);
            let above_state = world.get_block_state(&pos.up());

            // 检查方块是否可通行（非实心或为空气）
            let block_safe = block_state.is_air() || !block_state.is_solid();
            let above_safe = above_state.is_air() || !above_state.is_solid();

            if block_safe && above_safe {
                let position = Vector3::new(
                    f64::from(pos.0.x) + 0.5,
                    f64::from(pos.0.y) + 0.1,
                    f64::from(pos.0.z) + 0.5,
                );
                debug!(
                    "返回强制重生点 {:?}，维度：{:?}",
                    position, respawn_point.dimension
                );
                return Some(CalculatedRespawnPoint {
                    position,
                    yaw: respawn_point.yaw,
                    pitch: 0.0,
                    dimension: respawn_point.dimension.clone(),
                });
            }
            return None;
        }

        // 处理床重生
        if block.has_tag(&tag::Block::MINECRAFT_BEDS) {
            let bed_props = BedProperties::from_state_id(state_id);
            let facing = bed_props.facing;

            // 根据朝向尝试床周围的位置
            // 原版尝试多种偏移模式；我们使用简化版本
            if let Some(spawn_pos) = Self::find_bed_spawn_position(&world, pos, facing) {
                return Some(CalculatedRespawnPoint {
                    position: spawn_pos,
                    yaw: respawn_point.yaw,
                    pitch: 0.0,
                    dimension: respawn_point.dimension.clone(),
                });
            }
            return None;
        }

        // 处理重生锚（下界）
        if block == &Block::RESPAWN_ANCHOR {
            let anchor_props = AnchorProperties::from_state_id(state_id);
            let charges = anchor_props.charges;

            // 重生锚至少需要 1 个充能才能工作
            if charges == 0 {
                return None;
            }

            // 尝试锚点周围的位置
            if let Some(spawn_pos) = Self::find_anchor_spawn_position(&world, pos) {
                // 找到成功的重生位置后减少充能
                let new_charges = charges - 1;
                let mut new_props = anchor_props;
                new_props.charges = new_charges;
                world.set_block_state(
                    pos,
                    new_props.to_state_id(block),
                    papokin_world::world::BlockFlags::NOTIFY_ALL,
                );

                return Some(CalculatedRespawnPoint {
                    position: spawn_pos,
                    yaw: respawn_point.yaw,
                    pitch: 0.0,
                    dimension: respawn_point.dimension.clone(),
                });
            }
            return None;
        }

        None
    }

    /// 在床周围寻找有效的生成位置。
    /// 原版使用基于床朝向的复杂算法。
    /// 我们使用简化版本，优先尝试四个基本方向。
    fn find_bed_spawn_position(
        world: &Arc<crate::world::World>,
        bed_pos: &BlockPos,
        facing: HorizontalFacing,
    ) -> Option<Vector3<f64>> {
        // 根据床的朝向获取偏移（按原版顺序）
        let offsets = Self::get_bed_spawn_offsets(facing);

        for (dx, dz) in offsets {
            let check_pos = BlockPos(Vector3::new(
                bed_pos.0.x + dx,
                bed_pos.0.y,
                bed_pos.0.z + dz,
            ));

            if let Some(pos) = Self::find_respawn_pos(world, &check_pos) {
                return Some(pos);
            }

            // 同时尝试下一格（用于高台上的床）
            let check_pos_down = BlockPos(Vector3::new(
                bed_pos.0.x + dx,
                bed_pos.0.y - 1,
                bed_pos.0.z + dz,
            ));
            if let Some(pos) = Self::find_respawn_pos(world, &check_pos_down) {
                return Some(pos);
            }
        }

        // 最后再尝试床本身
        if let Some(pos) = Self::find_respawn_pos(world, bed_pos) {
            return Some(pos);
        }

        None
    }

    /// 根据朝向获取床周围的生成位置偏移。
    /// 这是原版 getAroundBedOffsets 的简化版本。
    fn get_bed_spawn_offsets(facing: HorizontalFacing) -> Vec<(i32, i32)> {
        let (fx, fz) = match facing {
            HorizontalFacing::North => (0, -1),
            HorizontalFacing::South => (0, 1),
            HorizontalFacing::West => (-1, 0),
            HorizontalFacing::East => (1, 0),
        };

        // 顺时针旋转
        let (rx, rz) = (-fz, fx);

        vec![
            (rx, rz),                   // 床的右侧
            (-rx, -rz),                 // 床的左侧
            (rx - fx, rz - fz),         // 右后
            (-rx - fx, -rz - fz),       // 左后
            (-fx, -fz),                 // 脚后方
            (-fx * 2, -fz * 2),         // 更靠后
            (rx + fx, rz + fz),         // 右前
            (-rx + fx, -rz + fz),       // 左前
            (fx, fz),                   // 在前方
            (rx - fx * 2, rz - fz * 2), // 右后最远端
        ]
    }

    /// 在重生锚周围寻找有效的生成位置。
    fn find_anchor_spawn_position(
        world: &Arc<crate::world::World>,
        anchor_pos: &BlockPos,
    ) -> Option<Vector3<f64>> {
        // 原版 VALID_HORIZONTAL_SPAWN_OFFSETS
        let horizontal_offsets: [(i32, i32); 8] = [
            (0, -1),
            (-1, 0),
            (0, 1),
            (1, 0),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ];

        // 先尝试同一层，再下一层，再上一层
        for dy in [0, -1, 1] {
            for (dx, dz) in horizontal_offsets {
                let check_pos = BlockPos(Vector3::new(
                    anchor_pos.0.x + dx,
                    anchor_pos.0.y + dy,
                    anchor_pos.0.z + dz,
                ));

                if let Some(pos) = Self::find_respawn_pos(world, &check_pos) {
                    return Some(pos);
                }
            }
        }

        // 同时尝试锚点正上方
        let above_pos = anchor_pos.up();
        Self::find_respawn_pos(world, &above_pos)
    }

    /// 检查某位置是否为有效的重生位置（原版 Dismounting.findRespawnPos 逻辑）。
    /// 若有效则返回生成位置，否则返回 None。
    fn find_respawn_pos(world: &Arc<crate::world::World>, pos: &BlockPos) -> Option<Vector3<f64>> {
        let (block, state) = world.get_block_and_state(pos);
        let below_state = world.get_block_state(&pos.down());

        // 检查该位置的方块是否不适合生成（例如在实心方块内部）
        if block.has_tag(&tag::Block::MINECRAFT_INVALID_SPAWN_INSIDE) {
            return None;
        }

        // 检查上方方块是否同样无效
        let above_block = world.get_block(&pos.up());
        if above_block.has_tag(&tag::Block::MINECRAFT_INVALID_SPAWN_INSIDE) {
            return None;
        }

        // 需要下方或当前位置为实心地板
        let has_floor = below_state.is_solid() || state.is_solid();
        if !has_floor {
            return None;
        }

        // 位置不得位于固体方块内
        if state.is_solid() && !state.is_air() {
            return None;
        }

        // 在该位置创建玩家大小的包围盒
        let x = f64::from(pos.0.x) + 0.5;
        let y = f64::from(pos.0.y) + 0.1;
        let z = f64::from(pos.0.z) + 0.5;
        let spawn_pos = Vector3::new(x, y, z);

        // 玩家尺寸：宽 0.6，高 1.8
        let half_width = 0.3;
        let height = 1.8;
        let player_box = BoundingBox::new(
            Vector3::new(x - half_width, y, z - half_width),
            Vector3::new(x + half_width, y + height, z + half_width),
        );

        // 检查空间是否为空（无方块碰撞）
        if !world.is_space_empty(player_box) {
            return None;
        }

        Some(spawn_pos)
    }

    pub fn sleep(&self, bed_head_pos: BlockPos) {
        // TODO: 停止骑乘

        self.get_entity().set_pose(EntityPose::Sleeping);
        self.living_entity
            .entity
            .set_pos(bed_head_pos.to_f64().add_raw(0.5, 0.6875, 0.5));
        self.get_entity().set_synced_data(
            papokin_data::tracked_data::player::SLEEPING_POS_ID,
            Some(bed_head_pos),
        );
        self.get_entity().set_velocity(Vector3::default());

        self.sleeping_since.store(Some(0));
        self.sleeping_bed_pos.store(Some(bed_head_pos));
        self.set_stat(
            statistics::StatisticCategory::Custom,
            statistics::CustomStatistic::TimeSinceRest as i32,
            0,
        );
    }

    pub fn get_off_ground_speed(&self) -> f64 {
        let sprinting = self.get_entity().is_sprinting();

        if !self.get_entity().has_vehicle() {
            let fly_speed =
                self.abilities.try_lock().ok().and_then(|abilities| {
                    abilities.flying.then_some(f64::from(abilities.fly_speed))
                });

            if let Some(flying) = fly_speed {
                return if sprinting { flying * 2.0 } else { flying };
            }
        }

        if sprinting { 0.025_999_999 } else { 0.02 }
    }

    pub fn is_flying(&self) -> bool {
        self.abilities.try_lock().is_ok_and(|a| a.flying)
    }

    pub fn set_sprinting(&self, is_sprinting: bool) {
        self.living_entity.set_sprinting(is_sprinting);
    }

    #[must_use]
    pub fn get_block_speed_factor(&self) -> f32 {
        self.living_entity.get_block_speed_factor()
    }

    fn is_sleeping(&self) -> bool {
        // TODO: 显式跟踪睡觉位置状态（原版检查 sleepingPosition.isPresent()）。
        self.sleeping_since.load().is_some()
    }

    #[must_use]
    pub fn is_swimming(&self) -> bool {
        !self.is_flying()
            && self.gamemode.load() != GameMode::Spectator
            && self.get_entity().is_swimming()
    }

    pub fn update_swimming(&self) {
        if self.is_flying() {
            self.get_entity().set_swimming(false);
        } else {
            let entity = self.get_entity();
            let is_sprinting = entity.is_sprinting();
            let in_water = entity.is_in_water();
            let is_passenger = entity.has_vehicle();

            if entity.is_swimming() {
                entity.set_swimming(is_sprinting && in_water && !is_passenger);
            } else {
                let is_under_water = entity.is_under_water();
                let block_pos = entity.block_pos.load();
                let world = entity.world.load();
                let (fluid, _) = world.get_fluid_and_fluid_state(&block_pos);
                let is_water_block = fluid.id == papokin_data::fluid::Fluid::WATER.id
                    || fluid.id == papokin_data::fluid::Fluid::FLOWING_WATER.id;

                entity.set_swimming(
                    is_sprinting && is_under_water && !is_passenger && is_water_block,
                );
            }
        }
    }

    const fn is_auto_spin_attack() -> bool {
        // TODO: 跟踪激活中的自动旋转/激流状态，并在其激活期间返回 true。
        false
    }

    fn can_fit_pose(&self, pose: EntityPose) -> bool {
        let entity = self.get_entity();
        let dimensions = Entity::get_entity_dimensions(pose);
        let position = entity.pos.load();
        let aabb = BoundingBox::new_from_pos(position.x, position.y, position.z, &dimensions);
        entity
            .world
            .load()
            .is_space_empty(aabb.contract_all(1.0E-7))
    }

    #[must_use]
    pub fn get_desired_pose(&self) -> EntityPose {
        let entity = self.get_entity();
        if self.is_sleeping() {
            EntityPose::Sleeping
        } else if self.is_swimming() {
            EntityPose::Swimming
        } else if entity.is_fall_flying() {
            EntityPose::FallFlying
        } else if Self::is_auto_spin_attack() {
            EntityPose::SpinAttack
        } else if entity.is_sneaking() && !self.is_flying() {
            EntityPose::Crouching
        } else {
            EntityPose::Standing
        }
    }

    pub fn update_player_pose(&self) {
        if !self.can_fit_pose(EntityPose::Swimming) {
            return;
        }

        self.update_swimming();
        let desired_pose = self.get_desired_pose();
        let actual_pose = if self.gamemode.load() == GameMode::Spectator
            || self.get_entity().has_vehicle()
            || self.can_fit_pose(desired_pose)
        {
            desired_pose
        } else if self.can_fit_pose(EntityPose::Crouching) {
            EntityPose::Crouching
        } else {
            EntityPose::Swimming
        };

        self.get_entity().set_pose(actual_pose);
    }

    pub fn wake_up(&self) {
        let world = self.world();
        let Some(bed_pos) = self.sleeping_bed_pos.load() else {
            self.living_entity.entity.set_pose(EntityPose::Standing);
            self.sleeping_since.store(None);
            return;
        };

        if let Some(server) = world.server.upgrade()
            && let Some(player_arc) = world.get_player_by_uuid(self.gameprofile.id)
        {
            let mut event =
                crate::plugin::api::events::player::player_bed::PlayerBedLeaveEvent::new(
                    player_arc, bed_pos,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
        }

        let (bed, bed_state) = world.get_block_and_state_id(&bed_pos);
        if bed == &Block::STRAW_BED {
            StrawBedBlock::destroy_after_use(&world, bed_pos);
        } else if bed.has_tag(&tag::Block::MINECRAFT_BEDS) {
            crate::block::blocks::bed::BedBlock::set_occupied(
                false, &world, bed, &bed_pos, bed_state,
            );
        }

        self.living_entity.entity.set_pose(EntityPose::Standing);
        self.living_entity.entity.set_pos(self.position());
        self.living_entity.entity.set_synced_data(
            papokin_data::tracked_data::player::SLEEPING_POS_ID,
            None::<BlockPos>,
        );

        self.set_stat(
            statistics::StatisticCategory::Custom,
            statistics::CustomStatistic::TimeSinceRest as i32,
            0,
        );

        let chunk_pos = self.living_entity.entity.chunk_pos.load();
        world.broadcast_to_chunk(
            chunk_pos,
            &CEntityAnimation::new(self.entity_id().into(), Animation::LeaveBed),
        );

        self.sleeping_since.store(None);
        self.sleeping_bed_pos.store(None);
    }

    pub fn show_title(&self, text: &TextComponent, mode: &TitleMode) {
        match mode {
            TitleMode::Title => {
                self.client.try_send_packet(&CTitleText::new(text));
            }
            TitleMode::SubTitle => {
                self.client.try_send_packet(&CSubtitle::new(text));
            }
            TitleMode::ActionBar => {
                self.client.try_send_packet(&CActionBar::new(text));
            }
        }
    }

    pub fn send_title_animation(&self, fade_in: i32, stay: i32, fade_out: i32) {
        self.client
            .try_send_packet(&CTitleAnimation::new(fade_in, stay, fade_out));
    }

    pub fn spawn_particle(
        &self,
        position: Vector3<f64>,
        offset: Vector3<f32>,
        max_speed: f32,
        particle_count: i32,
        particle: Particle,
    ) {
        let packet = CParticle::new(
            false,
            false,
            position,
            offset,
            max_speed,
            particle_count,
            VarInt(particle as i32),
            &[],
        );
        self.client.try_send_packet(&packet);
    }

    pub fn play_sound(
        &self,
        sound_id: u16,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
        seed: i64,
    ) {
        let packet = CSoundEffect::new(IdOr::Id(sound_id), category, position, volume, pitch, seed);
        self.try_send_client_packet(&packet);
    }

    pub fn play_sound_event(
        &self,
        sound: SoundEvent,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
        seed: i64,
    ) {
        let packet = CSoundEffect::new(IdOr::Value(sound), category, position, volume, pitch, seed);
        self.try_send_client_packet(&packet);
    }

    /// 停止在客户端上播放的声音。
    ///
    /// # Arguments
    ///
    /// * `sound_id`: 可选的 [`ResourceLocation`]，指定要停止的声音。若为 [`None`]，则指定类别（如有）中的所有声音都会被停止。
    /// * `category`: 可选的 [`SoundCategory`]，指定要停止的声音类别。若为 [`None`]，则具有指定资源位置（如果给出）的所有声音都将被停止。
    pub fn stop_sound(&self, sound_id: Option<ResourceLocation>, category: Option<SoundCategory>) {
        let packet = CStopSound::new(sound_id, category);
        self.try_send_client_packet(&packet);
    }

    /// 按标识符为该玩家播放自定义音效事件。
    pub fn play_custom_sound(
        &self,
        sound_name: &str,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        self.play_sound_event(
            papokin_protocol::SoundEvent {
                sound_name: sound_name.into(),
                range: None,
            },
            category,
            position,
            volume,
            pitch,
            rand::random::<i64>(),
        );
    }

    pub fn spawn_particles(
        &self,
        particle: Particle,
        pos: Vector3<f64>,
        count: u32,
        offset: Vector3<f32>,
        max_speed: f32,
    ) {
        let packet = CParticle::new(
            false,
            false,
            pos,
            offset,
            max_speed,
            count as i32,
            (particle.to_id() as i32).into(),
            &[],
        );
        self.try_send_client_packet(&packet);
    }

    pub fn send_block_change(&self, location: BlockPos, state_id: u16) {
        let packet = CBlockUpdate::new(location, (state_id as i32).into());
        self.try_send_client_packet(&packet);
    }

    pub fn reset_block_change(&self, location: BlockPos) {
        let state_id = self.world().get_block_state_id(&location);
        let packet = CBlockUpdate::new(location, (state_id.as_u16() as i32).into());
        self.try_send_client_packet(&packet);
    }

    pub fn send_hurt_animation(&self, yaw: f32) {
        let packet = CHurtAnimation::new(self.entity_id().into(), yaw);
        self.try_send_client_packet(&packet);
    }

    pub fn open_book(&self, hand: Hand) {
        let hand_val = match hand {
            Hand::Right => 0,
            Hand::Left => 1,
        };
        let packet = COpenBook::new(hand_val.into());
        self.try_send_client_packet(&packet);
    }

    pub fn open_sign_editor(&self, location: BlockPos, is_front_text: bool) {
        if let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
            && let Some(server) = self.world().server.upgrade()
        {
            let mut event =
                crate::plugin::api::events::player::player_open_sign::PlayerOpenSignEvent {
                    player: player_arc,
                    block_pos: location,
                    is_front: is_front_text,
                    cancelled: false,
                };
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        let packet = COpenSignEditor::new(location, is_front_text);
        self.try_send_client_packet(&packet);
    }

    pub fn set_velocity(&self, mut velocity: Vector3<f64>) {
        if let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
            && let Some(server) = self.world().server.upgrade()
        {
            let mut event =
                crate::plugin::api::events::player::player_velocity::PlayerVelocityEvent {
                    player: player_arc,
                    velocity,
                    cancelled: false,
                };
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
            velocity = event.velocity;
        }
        self.living_entity.entity.set_velocity(velocity);
        self.try_send_client_packet(&CEntityVelocity::new(self.entity_id().into(), velocity));
    }

    pub fn apply_knockback(&self, strength: f64, x: f64, z: f64) {
        let current_vel = self.living_entity.entity.velocity.load();
        let norm = x.hypot(z);
        if norm > 0.0 {
            let vx = current_vel.x / 2.0 - (x / norm) * strength;
            let vz = current_vel.z / 2.0 - (z / norm) * strength;
            let vy = (current_vel.y / 2.0 + strength).min(0.4);
            self.set_velocity(Vector3::new(vx, vy, vz));
        }
    }

    pub fn set_movement_locked(&self, locked: bool) {
        self.is_movement_locked.store(locked, Ordering::Relaxed);
    }

    pub fn is_movement_locked(&self) -> bool {
        self.is_movement_locked.load(Ordering::Relaxed)
    }

    pub fn set_freeze_ticks(&self, ticks: i32) {
        self.living_entity.entity.set_frozen_ticks(ticks);
    }

    pub fn get_freeze_ticks(&self) -> i32 {
        self.living_entity.entity.get_frozen_ticks()
    }

    pub fn send_game_event(
        &self,
        event: papokin_protocol::java::client::play::GameEvent,
        value: f32,
    ) {
        let packet = CGameEvent::new(event, value);
        self.try_send_client_packet(&packet);
    }

    /// 向玩家发送自定义服务器链接（显示在客户端的 Esc 暂停菜单中）。
    pub fn set_server_links(&self, links: &[papokin_protocol::Link<'_>]) {
        if let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
            && let Some(server) = self.world().server.upgrade()
        {
            let link_strings = links.iter().map(|l| l.url.clone()).collect();
            let mut event =
                crate::plugin::api::events::player::player_links_send::PlayerLinksSendEvent::new(
                    player_arc,
                    link_strings,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        let packet = CPlayServerLinks::new(links);
        self.try_send_client_packet(&packet);
    }

    pub fn process_inbound_packets(&self) {
        const MAX_PACKETS_PER_TICK: usize = 64;

        // Player::tick 在世界的方块更新冲刷之后运行。确认上一刻的
        // 预测，让 Java 客户端在解析之前先收到权威的方块状态
        // 这些预测。从数据包循环发送 ACK 会让门和其他
        // 预测的方块会短暂回退，因为其更新要到下一刻才会刷新。
        let client = &self.client;
        let seq = client.packet_sequence.swap(-1, Ordering::Relaxed);
        if seq != -1 {
            client.try_send_packet(&CAcknowledgeBlockChange::new(seq.into()));
        }

        let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id) else {
            return;
        };
        let Some(server_arc) = self.world().server.upgrade() else {
            return;
        };

        let mut count = 0;

        while let Some(packet) = self.inbound_packets.pop() {
            if self.client.is_closed() {
                break;
            }

            let client = self.client.clone();
            if let Err(e) = client.handle_play_packet(&player_arc, &server_arc, &packet) {
                if e.is_kick() {
                    if let Some(kick_reason) = e.client_kick_reason() {
                        client.try_kick(&TextComponent::text(kick_reason));
                    } else {
                        client.try_kick(&TextComponent::text(format!("处理传入数据包时出错：{e}")));
                    }
                }
                tracing::error!(
                    "处理游玩阶段数据包 id {} 失败（载荷 {} 字节）：{}",
                    packet.id,
                    packet.payload.len(),
                    e
                );
            }

            count += 1;
            if count >= MAX_PACKETS_PER_TICK {
                break;
            }
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn tick<'a>(&'a self, server: &'a Server) {
        self.process_inbound_packets();

        if self.is_spectator() {
            self.living_entity
                .entity
                .on_ground
                .store(false, Ordering::Relaxed);
        }

        if let Some(camera_id) = self.camera_target_id.load() {
            if camera_id == self.entity_id() {
                self.camera_target_id.store(None);
            } else {
                let world = self.world();
                let target = world
                    .get_player_by_id(camera_id)
                    .map(|p| Arc::clone(&p) as Arc<dyn EntityBase>)
                    .or_else(|| world.get_entity_by_id(camera_id));
                if let Some(target) = target {
                    let target_pos = target.get_entity().pos.load();
                    let player_pos = self.living_entity.entity.pos.load();
                    if player_pos != target_pos {
                        self.living_entity.entity.set_pos(target_pos);
                        if let Some(p) = self.world().get_player_by_uuid(self.gameprofile.id) {
                            crate::world::chunker::update_position(&p);
                        }
                    }
                } else {
                    // 目标已不存在：需先触发停止旁观
                    // 将相机重置回玩家。已取消的
                    // 事件在本刻保留（过时的）摄像机目标。
                    let world = self.world();
                    let stop_event_fired = world.server.upgrade().and_then(|server_arc| {
                        world.get_player_by_uuid(self.gameprofile.id).map(|player_arc| {
                            let mut stop_event =
                                crate::plugin::api::events::player::player_stop_spectating_entity::PlayerStopSpectatingEntityEvent::new(
                                    player_arc,
                                    camera_id,
                                );
                            server_arc
                                .plugin_manager
                                .fire_blocking(&server_arc, &mut stop_event);
                            stop_event.cancelled
                        })
                    });
                    if stop_event_fired != Some(true) {
                        self.camera_target_id.store(None);
                        self.try_send_client_packet(&CSetCamera::new(self.entity_id().into()));
                    }
                }
            }
        }

        if let Ok(current_screen_handler_guard) = self.current_screen_handler.try_lock() {
            let current_screen_handler = current_screen_handler_guard.clone();
            drop(current_screen_handler_guard);
            let is_invalid = current_screen_handler
                .try_lock()
                .is_ok_and(|screen_handler| {
                    screen_handler.as_any().is::<MerchantScreenHandler>()
                        && !screen_handler.can_use(self)
                });

            if is_invalid {
                if let Some(p) = self.world().get_player_by_uuid(self.gameprofile.id) {
                    p.close_handled_screen();
                }
            } else if let Ok(mut screen_handler) = current_screen_handler.try_lock() {
                screen_handler.send_content_updates();
            }
        }

        // 统计更新
        if let Ok(mut stats) = self.stats.try_lock() {
            stats.increment_custom(statistics::CustomStatistic::PlayTime, 1);
            stats.increment_custom(statistics::CustomStatistic::TotalWorldTime, 1);
            if !self.living_entity.dead.load(Ordering::Relaxed)
                && self.living_entity.health.load() > 0.0
            {
                stats.increment_custom(statistics::CustomStatistic::TimeSinceDeath, 1);
            }
            if !self.is_sleeping() {
                stats.increment_custom(statistics::CustomStatistic::TimeSinceRest, 1);
            }
            if self.living_entity.entity.sneaking.load(Ordering::Relaxed) {
                stats.increment_custom(statistics::CustomStatistic::SneakTime, 1);
            }
        }

        if let Ok(mut xp) = self.experience_pick_up_delay.try_lock()
            && *xp > 0
        {
            *xp -= 1;
        }
        if let Ok(listener) = self.chunk_listener.try_lock()
            && let Ok(mut sender) = self.chunk_sender.try_lock()
        {
            let watched = self.watched_section.load();
            while let Ok((pos, _)) = listener.try_recv() {
                if watched.is_within_distance(pos.x, pos.y) {
                    sender.enqueue_chunk(pos);
                }
            }
        }

        let world = self.world();
        let player_chunk = self.get_entity().chunk_pos.load();
        let epoch = self.chunk_send_epoch.load(Ordering::Relaxed);
        let version = self.client.version.load();

        let view_distance = self.watched_section.load().view_distance;
        let prepared_batch = self.chunk_sender.try_lock().ok().and_then(|mut sender| {
            sender.prepare_batch(&world.level, player_chunk, view_distance, epoch, version)
        });

        let _total_sent_chunks = prepared_batch.map_or_else(
            || {
                self.chunk_sender
                    .try_lock()
                    .map_or(0, |s| s.sent_chunks_count())
            },
            |batch| {
                // 跨 tick 复用的编码缓存：每个区块仅在数据变化
                // （弱引用失效）或未缓存时重新序列化。容量超限时
                // 整体清空兜底，防止跨世界移动无限积累。
                const MAX_ENCODE_CACHE_ENTRIES: usize = 8192;

                let mut cache_guard = self.chunk_encode_cache.try_lock().ok();
                if let Some(cache) = cache_guard.as_deref_mut()
                    && cache.len() > MAX_ENCODE_CACHE_ENTRIES
                {
                    cache.clear();
                }
                let encoded = cache_guard.as_deref_mut().map_or_else(
                    || {
                        let mut scratch = rustc_hash::FxHashMap::default();
                        crate::net::ChunkSender::encode_batch(&batch, &mut scratch)
                    },
                    |cache| crate::net::ChunkSender::encode_batch(&batch, cache),
                );
                let current_epoch = self.chunk_send_epoch.load(Ordering::Relaxed);
                let (sent, total_sent_chunks) = self.chunk_sender.try_lock().map_or_else(
                    |_| (Vec::new(), 0),
                    |mut sender| {
                        let sent =
                            sender.commit_batch(&batch, &encoded, &self.client, current_epoch);
                        (sent, sender.sent_chunks_count())
                    },
                );
                self.pair_entities_in_chunks(&world, &sent);
                total_sent_chunks
            },
        );

        self.tick_counter.fetch_add(1, Ordering::Relaxed);
        self.living_entity
            .entity
            .age
            .fetch_add(1, Ordering::Relaxed);
        if let Some(sleeping_since) = self.sleeping_since.load()
            && sleeping_since < 101
        {
            self.sleeping_since.store(Some(sleeping_since + 1));
            // 深度睡眠在计数器达到 100 时恰好触发一次。
            if sleeping_since + 1 == 100 {
                let world = self.world();
                if let Some(server_arc) = world.server.upgrade()
                    && let Some(player_arc) = world.get_player_by_uuid(self.gameprofile.id)
                {
                    let mut deep_sleep =
                        crate::plugin::api::events::player::player_deep_sleep::PlayerDeepSleepEvent::new(
                            player_arc,
                        );
                    server_arc
                        .plugin_manager
                        .fire_blocking(&server_arc, &mut deep_sleep);
                }
            }
        }

        if self.mining.load(Ordering::Relaxed)
            && let Some(p) = self.world().get_player_by_uuid(self.gameprofile.id)
        {
            let pos = *p
                .mining_pos
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let world = p.world();
            let state = world.get_block_state(&pos);
            // 方块被破坏了吗？
            if state.is_air() {
                p.stop_mining();
            } else {
                p.continue_mining(
                    pos,
                    &world,
                    state,
                    p.start_mining_time.load(Ordering::Relaxed),
                );
            }
        }
        self.last_attacked_ticks.fetch_add(1, Ordering::Relaxed);

        self.living_entity.tick(self, server);

        self.breath_manager.tick(self);

        let level_info = self.world().level_info.load();
        if level_info.difficulty == Difficulty::Peaceful
            && level_info.game_rules.natural_health_regeneration
        {
            let tick_count = self.tick_counter.load(Ordering::Relaxed);
            if tick_count % 20 == 0 {
                if self.can_food_heal() {
                    self.heal(1.0);
                }

                let saturation = self.hunger_manager.saturation.load();
                if saturation < 20.0 {
                    self.hunger_manager.set_saturation(saturation + 1.0);
                }
            }

            if tick_count % 10 == 0 && self.hunger_manager.level.load() < 20 {
                self.hunger_manager.add_hunger(1);
            }
        }

        self.hunger_manager.tick(self);

        // 原版在 PlayerEntity#tick 中于 super.tick() 之后更新姿态。
        self.update_player_pose();
        self.check_inventory_advancements();
        if let Ok(mut adv) = self.advancements.try_lock() {
            adv.flush_dirty(self, true);
        }

        // 经验处理
        self.tick_experience();
        self.tick_health();
        self.tick_raid_omen();
        self.tick_maps(server);

        // 防刷屏计数器衰减
        let anti_spam = &server.advanced_config.chat.anti_spam;
        if anti_spam.enabled && anti_spam.decay_per_tick > 0 {
            let _ = self.chat_spam_tick_count.fetch_update(
                Ordering::Relaxed,
                Ordering::Relaxed,
                |count| Some(count.saturating_sub(anti_spam.decay_per_tick)),
            );
        }

        // 超时/保活处理
        self.tick_client_load_timeout();
        // 空闲超时处理
        let now = Instant::now();
        let idle_timeout_minutes = server.player_idle_timeout.load(Ordering::Relaxed);
        if idle_timeout_minutes > 0 {
            let idle_duration = now.duration_since(self.last_action_time.load());
            if idle_duration >= Duration::from_secs(idle_timeout_minutes as u64 * 60) {
                self.kick(&TextComponent::translate(
                    translation::java::MULTIPLAYER_DISCONNECT_IDLING,
                    [],
                ));
            }
        }
    }

    fn continue_mining(
        &self,
        location: BlockPos,
        world: &World,
        state: &BlockState,
        starting_time: i32,
    ) -> bool {
        let time = self.tick_counter.load(Ordering::Relaxed) - starting_time;
        let speed = block::calc_block_breaking(self, state, Block::from_state_id(state.id));
        let total_progress = speed * (time + 1) as f32;
        let stage = (total_progress * 10.0) as i32;
        let stage = stage.min(9);
        let old_speed = self
            .current_block_breaking_speed
            .swap(speed.to_bits(), Ordering::Relaxed);
        let speed_changed = old_speed != speed.to_bits();
        if stage != self.current_block_destroy_stage.load(Ordering::Relaxed) || speed_changed {
            world.set_block_breaking(
                &self.living_entity.entity,
                location,
                BlockBreakingProgress::Update { stage },
            );
            self.current_block_destroy_stage
                .store(stage, Ordering::Relaxed);
        }
        total_progress >= 1.0
    }

    pub(crate) fn stop_mining(&self) {
        let was_mining = self.mining.swap(false, Ordering::Relaxed);
        let stage = self.current_block_destroy_stage.swap(-1, Ordering::Relaxed);
        self.current_block_breaking_speed
            .store(0, Ordering::Relaxed);

        if was_mining || stage >= 0 {
            let pos = *self
                .mining_pos
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.world().set_block_breaking(
                &self.living_entity.entity,
                pos,
                BlockBreakingProgress::Stop,
            );
        }
    }

    pub fn jump(&self) {
        // 与 Paper 对齐：from == to == 玩家当前位置；
        // 取消跳跃仅跳过统计与饥饿消耗的簿记。
        // 世界查询会提供规范的 `Arc<Player>`；当
        // 玩家尚未注册（早期登录刻）时，该事件会被
        // 被跳过，因为没有玩家句柄时任何合理的处理器都无法运行。
        let from = self.position();
        let world = self.world();
        let Some(player_arc) = world.get_player_by_uuid(self.gameprofile.id) else {
            return;
        };
        let mut jump_event = crate::plugin::api::events::player::player_jump::PlayerJumpEvent::new(
            player_arc, from, from,
        );
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut jump_event);
        }
        if jump_event.cancelled {
            return;
        }
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .increment_custom(statistics::CustomStatistic::Jump, 1);
        if self.living_entity.entity.is_sprinting() {
            self.add_exhaustion(0.2);
        } else {
            self.add_exhaustion(0.05);
        }
    }

    pub fn progress_motion(&self, delta_pos: Vector3<f64>) {
        // TODO: 滑翔……
        let entity = &self.living_entity.entity;
        let (rate, distance) = if self.is_swimming() || entity.is_submerged_in_water() {
            (0.01, delta_pos.length())
        } else if entity.is_in_water() {
            (0.01, delta_pos.horizontal_length())
        } else if self.living_entity.climbing.load(Ordering::Relaxed) {
            return;
        } else if entity.on_ground.load(Ordering::Relaxed) {
            let rate = if entity.is_sprinting() { 0.1 } else { 0.0 };
            (rate, delta_pos.horizontal_length())
        } else {
            return;
        };

        let delta = (distance * 100.0).round() as f32;
        if delta > 0.0 {
            self.add_exhaustion(rate * delta * 0.01);
        }
    }

    #[must_use]
    pub fn is_spectator(&self) -> bool {
        self.gamemode.load() == GameMode::Spectator
    }

    #[must_use]
    pub fn supports_player_loaded(&self) -> bool {
        self.client.version.load() >= JavaMinecraftVersion::V_1_21_4
    }

    #[must_use]
    pub fn has_client_loaded(&self) -> bool {
        if !self.supports_player_loaded() {
            return true;
        }
        self.client_loaded.load(Ordering::Relaxed)
            || self.client_loaded_timeout.load(Ordering::Relaxed) == 0
    }

    pub fn set_client_loaded(&self, loaded: bool) {
        if !self.supports_player_loaded() {
            self.client_loaded.store(true, Ordering::Relaxed);
            self.client_loaded_timeout.store(0, Ordering::Relaxed);
            return;
        }
        if !loaded {
            self.client_loaded_timeout.store(60, Ordering::Relaxed);
        }
        self.client_loaded.store(loaded, Ordering::Relaxed);
    }

    pub fn get_attack_cooldown_progress(&self, tps: f64, base_time: f64, attack_speed: f64) -> f64 {
        let x = f64::from(self.last_attacked_ticks.load(Ordering::Acquire)) + base_time;

        let progress_per_tick = tps / attack_speed;
        let progress = x / progress_per_tick;
        progress.clamp(0.0, 1.0)
    }

    pub async fn fire_packet_sent<P: Send + Sync + std::any::Any>(
        self: &Arc<Self>,
        packet: P,
        packet_id: i32,
        payload: Bytes,
    ) -> bool {
        let server = self.world().server.upgrade();
        if let Some(server) = server {
            let mut event =
                PacketSentEvent::new(self.clone(), packet_id, payload, Arc::new(packet));
            server.plugin_manager.fire(&server, &mut event).await;
            return event.cancelled;
        }
        false
    }

    pub(crate) async fn fire_packet_sent_event_no_obj(
        self: &Arc<Self>,
        packet_id: i32,
        payload: Bytes,
    ) -> PacketSentEvent {
        // 这是一个用于满足 WIT 中非可选要求的占位对象
        // 未来我们应让所有数据包变为 'static，或提供在 WIT 中表示原始数据包的方式
        struct RawPacket;

        let mut event = PacketSentEvent::new(self.clone(), packet_id, payload, Arc::new(RawPacket));
        if let Some(server) = self.world().server.upgrade() {
            server.plugin_manager.fire(&server, &mut event).await;
        }
        event
    }

    pub async fn fire_packet_sent_no_obj(self: &Arc<Self>, packet_id: i32, payload: Bytes) -> bool {
        self.fire_packet_sent_event_no_obj(packet_id, payload)
            .await
            .cancelled
    }

    pub const fn entity_id(&self) -> i32 {
        self.living_entity.entity.entity_id
    }

    /// 设置玩家相机目标实体的 ID。
    /// 若 `target_id` 与玩家自身的实体 ID 匹配，则将摄像机重置回玩家。
    pub fn set_camera_entity_id(&self, target_id: i32) {
        if target_id == self.entity_id() {
            self.camera_target_id.store(None);
            self.try_send_client_packet(&CSetCamera::new(self.entity_id().into()));
        } else {
            self.camera_target_id.store(Some(target_id));
            self.try_send_client_packet(&CSetCamera::new(target_id.into()));
        }
    }

    /// 将玩家的摄像机重置回其本人视角。
    pub fn reset_camera(&self) {
        // 停止旁观钩子：取消操作会让镜头留在目标身上。
        if let Some(target_id) = self.camera_target_id.load() {
            let world = self.world();
            let cancelled = world.server.upgrade().is_some_and(|server_arc| {
                world.get_player_by_uuid(self.gameprofile.id).is_some_and(|player_arc| {
                    let mut stop_event =
                        crate::plugin::api::events::player::player_stop_spectating_entity::PlayerStopSpectatingEntityEvent::new(
                            player_arc,
                            target_id,
                        );
                    server_arc
                        .plugin_manager
                        .fire_blocking(&server_arc, &mut stop_event);
                    stop_event.cancelled
                })
            });
            if cancelled {
                return;
            }
        }
        self.camera_target_id.store(None);
        self.try_send_client_packet(&CSetCamera::new(self.entity_id().into()));
    }

    /// 获取玩家摄像机当前附着的实体的实体 ID，
    /// 未被覆盖时则为玩家自身的实体 ID。
    pub fn get_camera_entity_id(&self) -> i32 {
        self.camera_target_id
            .load()
            .unwrap_or_else(|| self.entity_id())
    }

    pub fn world(&self) -> Arc<World> {
        self.living_entity.entity.world.load_full()
    }

    pub fn position(&self) -> Vector3<f64> {
        self.living_entity.entity.pos.load()
    }

    pub fn eye_position(&self) -> Vector3<f64> {
        let eye_height = self.living_entity.entity.get_eye_height();
        Vector3::new(
            self.living_entity.entity.pos.load().x,
            self.living_entity.entity.pos.load().y + eye_height,
            self.living_entity.entity.pos.load().z,
        )
    }

    /// 返回玩家的旋转。
    /// 先偏航（Yaw）后俯仰（Pitch）
    pub fn rotation(&self) -> (f32, f32) {
        (
            self.living_entity.entity.yaw.load(),
            self.living_entity.entity.pitch.load(),
        )
    }

    /// 更新玩家当前拥有的能力。
    pub fn send_abilities_update(&self) {
        let abilities = *self
            .abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut b = 0;

        if abilities.invulnerable {
            b |= 1;
        }
        if abilities.flying {
            b |= 2;
        }
        if abilities.allow_flying {
            b |= 4;
        }
        if abilities.creative {
            b |= 8;
        }
        let packet = CPlayerAbilities::new(b, abilities.fly_speed, abilities.walk_speed);
        self.client.try_send_packet(&packet);
    }

    pub fn send_stats(&self) {
        let java = &self.client;
        let packet_stats: Vec<Statistic> = {
            let stats_guard = self
                .stats
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            stats_guard
                .stats
                .iter()
                .map(|((category, stat), value)| Statistic {
                    category_id: VarInt(*category),
                    statistic_id: VarInt(*stat),
                    value: VarInt(*value),
                })
                .collect()
        };

        let packet = CAwardStats {
            stats: &packet_stats,
        };
        if let Ok(data) = java.serialize_packet(&packet) {
            java.try_enqueue_packet(data);
        }
    }

    pub fn increment_stat(&self, category: statistics::StatisticCategory, stat: i32, amount: i32) {
        let final_amount = if let Some(player_arc) =
            self.world().get_player_by_uuid(self.gameprofile.id)
            && let Some(server) = self.world().server.upgrade()
        {
            let mut event = crate::plugin::api::events::player::player_statistic_increment::PlayerStatisticIncrementEvent {
                player: player_arc,
                statistic_id: format!("{category:?}:{stat}"),
                amount,
                cancelled: false,
            };
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
            event.amount
        } else {
            amount
        };
        if let Ok(mut stats) = self.stats.try_lock() {
            stats.increment(category, stat, final_amount);
        }
    }

    pub fn set_stat(&self, category: statistics::StatisticCategory, stat: i32, value: i32) {
        if let Ok(mut stats) = self.stats.try_lock() {
            stats.set(category, stat, value);
        }
    }

    pub fn get_stat(&self, category: statistics::StatisticCategory, stat: i32) -> i32 {
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(category, stat)
    }

    pub fn get_custom_stat(&self, stat: statistics::CustomStatistic) -> i32 {
        self.get_stat(statistics::StatisticCategory::Custom, stat as i32)
    }

    pub fn set_custom_stat(&self, stat: statistics::CustomStatistic, value: i32) {
        self.set_stat(statistics::StatisticCategory::Custom, stat as i32, value);
    }

    pub fn increment_custom_stat(&self, stat: statistics::CustomStatistic, amount: i32) {
        self.increment_stat(statistics::StatisticCategory::Custom, stat as i32, amount);
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn get_movement_statistic(&self) -> statistics::CustomStatistic {
        let entity = self.get_entity();
        if entity.has_vehicle() {
            let vehicle = entity
                .vehicle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(vehicle) = vehicle.as_ref() {
                let entity_type = vehicle.get_entity().entity_type;
                if entity_type.has_tag(&papokin_data::tag::EntityType::MINECRAFT_BOAT)
                    || entity_type.has_tag(&papokin_data::tag::EntityType::C_BOATS)
                    || entity_type == &EntityType::OAK_BOAT
                    || entity_type == &EntityType::SPRUCE_BOAT
                    || entity_type == &EntityType::BIRCH_BOAT
                    || entity_type == &EntityType::JUNGLE_BOAT
                    || entity_type == &EntityType::ACACIA_BOAT
                    || entity_type == &EntityType::DARK_OAK_BOAT
                    || entity_type == &EntityType::MANGROVE_BOAT
                    || entity_type == &EntityType::CHERRY_BOAT
                    || entity_type == &EntityType::PALE_OAK_BOAT
                    || entity_type == &EntityType::BAMBOO_RAFT
                    || entity_type == &EntityType::OAK_CHEST_BOAT
                    || entity_type == &EntityType::SPRUCE_CHEST_BOAT
                    || entity_type == &EntityType::BIRCH_CHEST_BOAT
                    || entity_type == &EntityType::JUNGLE_CHEST_BOAT
                    || entity_type == &EntityType::ACACIA_CHEST_BOAT
                    || entity_type == &EntityType::DARK_OAK_CHEST_BOAT
                    || entity_type == &EntityType::MANGROVE_CHEST_BOAT
                    || entity_type == &EntityType::CHERRY_CHEST_BOAT
                    || entity_type == &EntityType::PALE_OAK_CHEST_BOAT
                    || entity_type == &EntityType::BAMBOO_CHEST_RAFT
                {
                    return statistics::CustomStatistic::BoatOneCm;
                }
                if entity_type.has_tag(&papokin_data::tag::EntityType::C_MINECARTS)
                    || entity_type == &EntityType::MINECART
                    || entity_type == &EntityType::CHEST_MINECART
                    || entity_type == &EntityType::FURNACE_MINECART
                    || entity_type == &EntityType::TNT_MINECART
                    || entity_type == &EntityType::HOPPER_MINECART
                    || entity_type == &EntityType::COMMAND_BLOCK_MINECART
                    || entity_type == &EntityType::SPAWNER_MINECART
                {
                    return statistics::CustomStatistic::MinecartOneCm;
                }
                if entity_type == &EntityType::HORSE
                    || entity_type == &EntityType::DONKEY
                    || entity_type == &EntityType::MULE
                    || entity_type == &EntityType::SKELETON_HORSE
                    || entity_type == &EntityType::ZOMBIE_HORSE
                    || entity_type == &EntityType::CAMEL
                    || entity_type == &EntityType::LLAMA
                    || entity_type == &EntityType::TRADER_LLAMA
                {
                    return statistics::CustomStatistic::HorseOneCm;
                }
                if entity_type == &EntityType::PIG {
                    return statistics::CustomStatistic::PigOneCm;
                }
                if entity_type == &EntityType::STRIDER {
                    return statistics::CustomStatistic::StriderOneCm;
                }
                if entity_type == &EntityType::HAPPY_GHAST {
                    return statistics::CustomStatistic::HappyGhastOneCm;
                }
                if entity_type == &EntityType::NAUTILUS
                    || entity_type == &EntityType::ZOMBIE_NAUTILUS
                {
                    return statistics::CustomStatistic::NautilusOneCm;
                }
            }
        }

        if self.is_flying() {
            return statistics::CustomStatistic::FlyOneCm;
        }

        if entity.fall_flying.load(Ordering::Relaxed) {
            return statistics::CustomStatistic::AviateOneCm;
        }

        if entity.swimming.load(Ordering::Relaxed) {
            return statistics::CustomStatistic::SwimOneCm;
        }

        let pos = entity.block_pos.load();
        let world = entity.world.load_full();
        let block = world.get_block(&pos);
        if block.has_tag(&papokin_data::tag::Block::MINECRAFT_CLIMBABLE) {
            return statistics::CustomStatistic::ClimbOneCm;
        }

        if entity.touching_water.load(Ordering::Relaxed) {
            if entity.is_submerged_in_water() {
                return statistics::CustomStatistic::WalkUnderWaterOneCm;
            }
            return statistics::CustomStatistic::WalkOnWaterOneCm;
        }

        if entity.sneaking.load(Ordering::Relaxed) {
            return statistics::CustomStatistic::CrouchOneCm;
        }

        if entity.sprinting.load(Ordering::Relaxed) {
            return statistics::CustomStatistic::SprintOneCm;
        }

        if !entity.on_ground.load(Ordering::Relaxed) && entity.velocity.load().y < -0.005 {
            return statistics::CustomStatistic::FallOneCm;
        }

        statistics::CustomStatistic::WalkOneCm
    }

    /// 向客户端更新玩家当前的权限等级。
    pub fn send_permission_lvl_update(&self) {
        let status = match self.permission_lvl.load() {
            PermissionLvl::Zero => EntityStatus::PermissionLevelAll,
            PermissionLvl::One => EntityStatus::PermissionLevelModerators,
            PermissionLvl::Two => EntityStatus::PermissionLevelGamemasters,
            PermissionLvl::Three => EntityStatus::PermissionLevelAdmins,
            PermissionLvl::Four => EntityStatus::PermissionLevelOwners,
        };
        // 玩家在切换维度之后可能还没有追踪条目。
        // 权限等级属于此连接，而不属于跟踪客户端。
        let packet = papokin_protocol::java::client::play::CEntityStatus::new(
            self.living_entity.entity.entity_id,
            status as i8,
        );
        self.client.try_send_packet(&packet);
    }

    /// 设置玩家的难度等级。
    pub fn send_difficulty_update(&self) {
        let world = self.world();
        let level_info = world.level_info.load();
        self.client.try_send_packet(&CChangeDifficulty::new(
            level_info.difficulty as u8,
            level_info.difficulty_locked,
        ));
    }

    /// 设置玩家的权限等级并通知客户端。
    pub fn set_permission_lvl(
        self: &Arc<Self>,
        server: &Arc<Server>,
        lvl: PermissionLvl,
        command_dispatcher: &CommandDispatcher,
    ) {
        self.permission_lvl.store(lvl);
        self.send_permission_lvl_update();

        client_suggestions::send_c_commands_packet(self, server, command_dispatcher);
    }

    pub fn can_use_game_master_blocks(&self) -> bool {
        self.gamemode.load() == GameMode::Creative
            && self.permission_lvl.load() >= PermissionLvl::Two
    }

    /// 仅向该玩家发送世界时间。
    pub fn send_time(&self, world: &World) {
        let advance_time = {
            let lock = world.level_info.load();
            lock.game_rules.advance_time
        };

        let clock_packet = {
            let l_world = world
                .level_time
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some((custom_time, relative)) = self.per_player_time.load() {
                let time_of_day = if relative {
                    (l_world.time_of_day as u64 + custom_time) as i64
                } else {
                    custom_time as i64
                };
                let paused = l_world.paused || !advance_time;
                let rate = if paused { 0.0 } else { l_world.rate };
                CUpdateTime::new_clock(
                    l_world.world_age,
                    0,
                    time_of_day,
                    l_world.partial_tick,
                    rate,
                )
            } else {
                let (total_ticks, partial_tick, rate) = l_world.pack_network_state(advance_time);
                CUpdateTime::new_clock(l_world.world_age, 0, total_ticks, partial_tick, rate)
            }
        };

        self.client.try_send_packet(&clock_packet);
    }

    pub fn set_player_time(&self, time: u64, relative: bool) {
        let world = self.world();
        self.per_player_time.store(Some((time, relative)));
        self.send_time(&world);
    }

    pub fn reset_player_time(&self) {
        let world = self.world();
        self.per_player_time.store(None);
        self.send_time(&world);
    }

    pub fn get_player_time(&self) -> Option<u64> {
        self.per_player_time.load().map(|(t, _)| t)
    }

    pub fn is_player_time_relative(&self) -> bool {
        self.per_player_time.load().is_none_or(|(_, r)| r)
    }

    pub fn set_player_weather(&self, weather: PlayerWeather) {
        self.per_player_weather.store(Some(weather));
    }

    pub fn reset_player_weather(&self) {
        self.per_player_weather.store(None);
    }

    pub fn get_player_weather(&self) -> Option<PlayerWeather> {
        self.per_player_weather.load()
    }

    pub fn try_send_client_packet<C: papokin_protocol::ClientPacket + Sync>(&self, packet: &C) {
        self.client.try_send_packet(packet);
    }

    pub async fn send_client_packet<C: papokin_protocol::ClientPacket + Sync>(&self, packet: &C) {
        self.client.enqueue_client_packet(packet).await;
    }

    #[must_use]
    pub const fn as_java(&self) -> Option<JavaPlayer<'_>> {
        Some(JavaPlayer(self))
    }

    pub fn reset_scoreboard(&self) {
        *self
            .custom_scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        self.send_scoreboard();
    }

    pub fn send_scoreboard(&self) {
        let guard = self
            .custom_scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(CustomScoreboard::Java(custom)) = guard.as_ref() {
            custom.send_to_player(self);
        } else {
            drop(guard);
            self.world()
                .scoreboard
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .send_to_player(self);
        }
    }

    pub fn get_team(&self) -> Option<crate::world::scoreboard::Team> {
        let guard = self
            .custom_scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(CustomScoreboard::Java(sb)) = guard.as_ref()
            && let Some(team) = sb.get_entity_team(&self.gameprofile.name)
        {
            return Some(team.clone());
        }
        let world = self.world();
        let sb = world
            .scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sb.get_entity_team(&self.gameprofile.name).cloned()
    }

    pub fn get_team_name(&self) -> Option<String> {
        let guard = self
            .custom_scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(CustomScoreboard::Java(sb)) = guard.as_ref()
            && let Some(team) = sb.get_entity_team(&self.gameprofile.name)
        {
            return Some(team.name.clone());
        }
        let world = self.world();
        let sb = world
            .scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sb.get_entity_team(&self.gameprofile.name)
            .map(|team| team.name.clone())
    }

    pub fn set_compass_target(&self, pos: papokin_util::math::position::BlockPos) {
        use papokin_protocol::java::client::play::CPlayerSpawnPosition;
        self.compass_target.store(Some(pos));
        self.try_send_client_packet(&CPlayerSpawnPosition::new(pos, 0.0, 0.0, String::new()));
    }

    pub fn get_compass_target(&self) -> Option<papokin_util::math::position::BlockPos> {
        self.compass_target.load()
    }

    pub fn set_respawn_location(&self, pos: papokin_util::math::position::BlockPos) {
        self.respawn_location.store(Some(pos));
    }

    pub fn get_respawn_location(&self) -> Option<papokin_util::math::position::BlockPos> {
        self.respawn_location.load()
    }

    pub fn hide_player(&self, other_id: uuid::Uuid) {
        self.hidden_players
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(other_id);
    }

    pub fn show_player(&self, other_id: uuid::Uuid) {
        self.hidden_players
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&other_id);
    }

    pub fn can_see(&self, other_id: &uuid::Uuid) -> bool {
        !self
            .hidden_players
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(other_id)
    }

    pub fn can_see_player(&self, other_id: &uuid::Uuid) -> bool {
        self.can_see(other_id)
    }

    // --- 经验与等级 API ---
    pub fn add_experience(self: &Arc<Self>, points: i32) {
        self.add_experience_points(points);
    }

    pub fn add_levels(&self, levels: i32) {
        self.add_experience_levels(levels);
    }

    pub fn get_experience_level(&self) -> i32 {
        self.experience_level.load(Ordering::Relaxed)
    }

    pub fn get_experience_progress(&self) -> f32 {
        self.experience_progress.load()
    }

    pub fn get_total_experience(&self) -> i32 {
        self.experience_points.load(Ordering::Relaxed)
    }

    pub fn set_experience_progress(&self, progress: f32) {
        let level = self.get_experience_level();
        let max_points = experience::points_in_level(level);
        let points = (progress.clamp(0.0, 1.0) * max_points as f32) as i32;
        self.set_experience(level, progress, points);
    }

    pub fn set_total_experience(&self, points: i32) {
        self.set_experience_points(points);
    }

    // --- 物品冷却系统 ---
    pub fn set_item_cooldown(&self, item_id: &str, ticks: i32) {
        self.start_cooldown(item_id.to_string(), ticks);
    }

    pub fn get_item_cooldown(&self, item_id: &str) -> Option<i32> {
        let cooldowns = self
            .item_cooldowns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cooldown) = cooldowns.get(item_id) {
            let current_tick = self.tick_counter.load(Ordering::Relaxed);
            let elapsed = current_tick - cooldown.start_tick;
            if elapsed < cooldown.duration {
                return Some(cooldown.duration - elapsed);
            }
        }
        None
    }

    pub fn has_item_cooldown(&self, item_id: &str) -> bool {
        self.is_on_cooldown(item_id)
    }

    // --- Tab 列表与显示名称 ---
    pub fn set_tab_list_ping(&self, latency_ms: i32) {
        self.set_tab_list_latency(latency_ms);
    }

    // --- 饥饿与饱食度别名 ---
    pub fn get_food_saturation(&self) -> f32 {
        self.get_saturation()
    }

    pub fn set_food_saturation(&self, saturation: f32) {
        self.set_saturation(saturation);
    }

    pub fn get_food_exhaustion(&self) -> f32 {
        self.get_exhaustion()
    }

    pub fn set_food_exhaustion(&self, exhaustion: f32) {
        self.set_exhaustion(exhaustion);
    }

    pub fn get_target_block(
        &self,
        world: &Arc<World>,
        max_distance: f64,
    ) -> Option<papokin_util::math::position::BlockPos> {
        let (yaw, pitch) = (
            self.living_entity.entity.yaw.load(),
            self.living_entity.entity.pitch.load(),
        );
        let eye_pos = self.living_entity.entity.get_eye_pos();
        let yaw_rad = f64::from(yaw + 90.0).to_radians();
        let pitch_rad = f64::from(-pitch).to_radians();
        let dir = papokin_util::math::vector3::Vector3::new(
            pitch_rad.cos() * yaw_rad.cos(),
            pitch_rad.sin(),
            pitch_rad.cos() * yaw_rad.sin(),
        );
        let end_pos = eye_pos + dir * max_distance;
        let res = world.raycast(eye_pos, end_pos, |pos, w| !w.get_block_state(pos).is_air());
        res.map(|(pos, _)| pos)
    }

    pub async fn unload_watched_chunks(&self, world: &World) {
        let radial_chunks = self.watched_section.load().all_chunks_within();
        let level = &world.level;
        let chunks_to_clean = level.mark_chunks_as_not_watched(radial_chunks).await;
        if !chunks_to_clean.is_empty() {
            world.remove_entities_in_chunks(&chunks_to_clean).await;
            level.clean_entity_chunks(&chunks_to_clean);
        }
        for chunk in &chunks_to_clean {
            self.send_client_packet(&CUnloadChunk::new(chunk.x, chunk.y))
                .await;
        }

        self.watched_section.store(Cylindrical::new(
            Vector2::new(0, 0),
            NonZero::new(1).unwrap_or(NonZero::<u8>::MIN),
        ));
    }

    /// 将玩家传送到另一个世界或维度，可指定位置、偏航角与俯仰角。
    pub async fn teleport_world(
        self: &Arc<Self>,
        new_world: Arc<World>,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
    ) {
        self.teleport_world_with_relatives(new_world, position, yaw, pitch, Vec::new())
            .await;
    }

    /// `teleport_world` 的变体，会单独标记……的各个组件
    /// clientbound 位置数据包作为相对坐标（Papo `TeleportFlags`
    /// 等价）。`none` 的 yaw/pitch 会回退为世界的出生点
    /// 旋转（如 `teleport_world`），并忽略对应的旋转标志。
    ///
    /// 与原版一致，位置数据包（网络）以原始值发送，并且
    /// 标志，让客户端自行解析相对分量，而
    /// 服务器会追踪解析出的绝对目标。
    #[expect(clippy::too_many_lines)]
    pub async fn teleport_world_with_relatives(
        self: &Arc<Self>,
        new_world: Arc<World>,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        relatives: Vec<PositionFlag>,
    ) {
        let current_world = self.living_entity.entity.world.load_full();
        // 缺失的偏航/俯仰回退到出生点朝向；相应的
        // 旋转标志此时并不适用。
        let mut relative_bits = PositionFlag::get_bitfield(&relatives);
        if yaw.is_none() {
            relative_bits &=
                !PositionFlag::get_bitfield(&[PositionFlag::YRot, PositionFlag::RotateDelta]);
        }
        if pitch.is_none() {
            relative_bits &=
                !PositionFlag::get_bitfield(&[PositionFlag::XRot, PositionFlag::RotateDelta]);
        }
        let relatives = PositionFlag::from_bitfield(relative_bits);
        let yaw = yaw.unwrap_or(new_world.level_info.load().spawn_yaw);
        let pitch = pitch.unwrap_or(new_world.level_info.load().spawn_pitch);
        // 在服务器端解析绝对目标；Java 客户端接收
        // 原始值和标志，并自行计算出相同的目标。
        let (abs_position, abs_yaw, abs_pitch) = Self::resolve_relative_teleport(
            &self.living_entity.entity,
            position,
            yaw,
            pitch,
            &relatives,
        );

        let Some(server) = new_world.server.upgrade() else {
            return;
        };

        send_cancellable! {{
            server;
            PlayerChangeWorldEvent {
                player: self.clone(),
                previous_world: current_world.clone(),
                new_world: new_world.clone(),
                position: abs_position,
                yaw: abs_yaw,
                pitch: abs_pitch,
                cancelled: false,
            };

            'after: {
                // 如果事件重写了目标，则使用原始相对分量
                // 已无法描述它：回退为绝对传送。
                let target_rewritten = event.position != abs_position
                    || event.yaw != abs_yaw
                    || event.pitch != abs_pitch;
                // TODO: 这是与 world 重复的代码
                let abs_position = event.position;
                let abs_yaw = event.yaw;
                let abs_pitch = event.pitch;
                let new_world = event.new_world;

                // 目标坐标可能来自插件 API 或被事件处理器改写：
                // 非有限坐标一旦落库会沿实体位置污染全部下游算术，
                // 超界坐标会把区块加载中心推到世界边缘，二者都直接拒绝。
                if !abs_position.x.is_finite()
                    || !abs_position.y.is_finite()
                    || !abs_position.z.is_finite()
                    || abs_position.x.abs() > 2.999_99E7
                    || abs_position.z.abs() > 2.999_99E7
                {
                    warn!(
                        "拒绝玩家 {} 的跨世界传送：目标坐标非法（{:?}）",
                        self.gameprofile.name, abs_position
                    );
                    return;
                }

                let (packet_position, packet_yaw, packet_pitch, packet_relatives) =
                    if target_rewritten {
                        (abs_position, abs_yaw, abs_pitch, Vec::new())
                    } else {
                        (position, yaw, pitch, relatives)
                    };

                self.set_client_loaded(false);
                let Some(player) = current_world.remove_player(self, false).await else {
                    return;
                };
               new_world.players.rcu(|current_list| {
                    let mut new_list = (**current_list).clone();
                    new_list.push(player.clone());
                    new_list
                });
                self.unload_watched_chunks(&current_world).await;

                self.change_world_chunks(&current_world.level, &new_world);
                self.living_entity.entity.set_world(new_world.clone());

                if new_world.dimension == papokin_data::dimension::Dimension::THE_NETHER {
                    self.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::EnterDimension {
                        dimension: "the_nether".to_string(),
                    });
                } else if new_world.dimension == papokin_data::dimension::Dimension::THE_END {
                    self.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::EnterDimension {
                        dimension: "the_end".to_string(),
                    });
                }

                let last_pos = self.living_entity.entity.last_pos.load();
                let death_dimension = ResourceLocation::from(self.world().dimension.minecraft_name);
                let death_location = BlockPos(Vector3::new(
                    last_pos.x.round() as i32,
                    last_pos.y.round() as i32,
                    last_pos.z.round() as i32,
                ));
                let packet = CRespawn::new(
                    PlayerSpawnData::new(
                        new_world.dimension.clone(),
                        biome::hash_seed(new_world.level.seed.0), // 种子
                        self.gamemode.load() as u8,
                        self.previous_gamemode.load().unwrap_or(self.gamemode.load()) as i8,
                        false,
                        false,
                        Some((death_dimension, death_location)),
                        VarInt(self.get_entity().portal_cooldown.load(Ordering::Relaxed) as i32),
                        new_world.sea_level.into(),
                    ),
                    CRespawn::KEEP_ALL_DATA,
                );
                if let Ok(data) = self.client.serialize_packet(&packet) {
                    self.client.send_packet_now(data).await;
                }

                self.send_permission_lvl_update();

                player.get_entity().set_pos(abs_position);
                player.get_entity().set_rotation(abs_yaw, abs_pitch);
                player.get_entity().last_pos.store(abs_position);

                self.send_abilities_update();

                self.enqueue_set_held_item_packet(&CSetSelectedSlot::new(
                    self.get_inventory().get_selected_slot() as i8,
                ));

                self.on_screen_handler_opened(&self.player_screen_handler);

                self.send_health();

                new_world.send_world_info(&player, abs_position, abs_yaw, abs_pitch);

                let java_client = &player.client;
                let center_chunk = player.get_entity().chunk_pos.load();
                let chunk = new_world
                    .level
                    .get_or_fetch_chunk(center_chunk, std::clone::Clone::clone)
                    .await;
                java_client.send_chunks(&[chunk]).await;

                if player.fire_teleport_event(abs_position) {
                    player.send_teleport_packet(
                        packet_position,
                        packet_yaw,
                        packet_pitch,
                        abs_position,
                        abs_yaw,
                        abs_pitch,
                        &packet_relatives,
                    );
                }

                let mut changed_world_event = crate::plugin::api::events::player::player_changed_world::PlayerChangedWorldEvent {
                    player: player.clone(),
                    from_world: current_world,
                    to_world: new_world,
                    cancelled: false,
                };
                server.plugin_manager.fire(&server, &mut changed_world_event).await;
            }
        }}
    }

    /// `yaw` 和 `pitch` 以度为单位。
    /// 很少使用，例如将玩家从床上唤醒或玩家首次生成时。其他情况下应使用 `teleport` 方法。
    /// 玩家应以 `SConfirmTeleport` 数据包作为响应。
    pub fn request_teleport(&self, position: Vector3<f64>, yaw: f32, pitch: f32) {
        self.request_teleport_with_relatives(position, yaw, pitch, &[]);
    }

    /// `yaw` 和 `pitch` 以度为单位。
    /// `request_teleport` 的变体，会单独标记……的各个组件
    /// clientbound 位置数据包作为相对坐标（Papo `TeleportFlags`
    /// 等价）：`relatives` 中的一个标志标记对应的数据包组件
    /// 视为相对于玩家当前状态的值，而非绝对值。
    ///
    /// 服务端状态会更新为解析后的绝对目标；
    /// 客户端会连同标志一起收到原始值。
    pub fn request_teleport_with_relatives(
        &self,
        position: Vector3<f64>,
        yaw: f32,
        pitch: f32,
        relatives: &[PositionFlag],
    ) {
        let _ = self.request_teleport_resolved(position, yaw, pitch, relatives);
    }

    /// 解析传送目标，其各分量可能是相对于
    /// 实体当前状态转换为绝对值。`RotateDelta` 会应用
    /// 旋转量作为叠加在当前旋转之上的增量，与
    /// 原版客户端行为。
    fn resolve_relative_teleport(
        entity: &Entity,
        position: Vector3<f64>,
        yaw: f32,
        pitch: f32,
        relatives: &[PositionFlag],
    ) -> (Vector3<f64>, f32, f32) {
        let current = entity.pos.load();
        let abs_position = Vector3::new(
            if relatives.contains(&PositionFlag::X) {
                current.x + position.x
            } else {
                position.x
            },
            if relatives.contains(&PositionFlag::Y) {
                current.y + position.y
            } else {
                position.y
            },
            if relatives.contains(&PositionFlag::Z) {
                current.z + position.z
            } else {
                position.z
            },
        );
        let rotate_delta = relatives.contains(&PositionFlag::RotateDelta);
        let abs_yaw = if rotate_delta || relatives.contains(&PositionFlag::YRot) {
            entity.yaw.load() + yaw
        } else {
            yaw
        };
        let abs_pitch = if rotate_delta || relatives.contains(&PositionFlag::XRot) {
            entity.pitch.load() + pitch
        } else {
            pitch
        };
        (abs_position, abs_yaw, abs_pitch)
    }

    /// 触发传送事件并发送客户端方向的位置数据包，用于
    /// 一个各分量可为相对值的传送。返回解析后的
    /// 绝对目标；若传送被取消则返回 `None`
    /// (或没有可用的服务器)。
    fn request_teleport_resolved(
        &self,
        position: Vector3<f64>,
        yaw: f32,
        pitch: f32,
        relatives: &[PositionFlag],
    ) -> Option<(Vector3<f64>, f32, f32)> {
        // 保留 `request_teleport` 的提前返回：没有服务器就没有传送。
        self.world().server.upgrade()?;
        let (abs_position, abs_yaw, abs_pitch) = Self::resolve_relative_teleport(
            &self.living_entity.entity,
            position,
            yaw,
            pitch,
            relatives,
        );
        if !self.fire_teleport_event(abs_position) {
            return None;
        }
        self.send_teleport_packet(
            position,
            yaw,
            pitch,
            abs_position,
            abs_yaw,
            abs_pitch,
            relatives,
        );
        Some((abs_position, abs_yaw, abs_pitch))
    }

    /// 为到 `to` 的传送触发 `PlayerTeleportEvent`。若事件被取消则返回 `false`
    /// 当事件已被取消或没有可用的服务器时。
    fn fire_teleport_event(&self, to: Vector3<f64>) -> bool {
        let Some(server) = self.world().server.upgrade() else {
            return false;
        };
        if let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id) {
            let mut event = PlayerTeleportEvent {
                player: player_arc,
                from: self.living_entity.entity.pos.load(),
                to,
                cancelled: false,
            };
            server.plugin_manager.fire_blocking(&server, &mut event);
            return !event.cancelled;
        }
        true
    }

    /// 分配一个传送 ID，并将服务器端状态更新为解析出的结果，
    /// 绝对目标，并使用原始值发送发往客户端的位置数据包
    /// `position`/`yaw`/`pitch` 值以及 `relatives`。
    #[expect(clippy::too_many_arguments)]
    fn send_teleport_packet(
        &self,
        position: Vector3<f64>,
        yaw: f32,
        pitch: f32,
        abs_position: Vector3<f64>,
        abs_yaw: f32,
        abs_pitch: f32,
        relatives: &[PositionFlag],
    ) {
        // 这是用于创建传送 id 的超级特殊魔法代码
        // 这会返回旧值
        // 此操作在溢出时会回绕。
        let i = self.teleport_id_count.fetch_add(1, Ordering::Relaxed);
        self.chunk_send_epoch.fetch_add(1, Ordering::Relaxed);
        let teleport_id = i + 1;
        let entity = &self.living_entity.entity;
        entity.set_pos(abs_position);
        entity.set_rotation(abs_yaw, abs_pitch);
        *self
            .awaiting_teleport
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((teleport_id.into(), abs_position));
        let packet = CPlayerPosition::new(
            teleport_id.into(),
            position,
            Vector3::new(0.0, 0.0, 0.0),
            yaw,
            pitch,
            // `PositionFlag` 不是 `Clone`；需要从其重建该向量
            // 其位域（无损往返转换）。
            PositionFlag::from_bitfield(PositionFlag::get_bitfield(relatives)),
        );
        self.client.try_send_packet(&packet);
    }

    pub fn block_interaction_range(&self) -> f64 {
        if self.gamemode.load() == GameMode::Creative {
            5.0
        } else {
            4.5
        }
    }

    pub fn can_interact_with_block_at(&self, position: &BlockPos, additional_range: f64) -> bool {
        let d = self.block_interaction_range() + additional_range;
        let box_pos = BoundingBox::from_block(position);
        let entity_pos = self.living_entity.entity.pos.load();
        let eye_height = self.living_entity.entity.get_eye_height();
        box_pos.squared_magnitude(Vector3 {
            x: entity_pos.x,
            y: entity_pos.y + eye_height,
            z: entity_pos.z,
        }) < d * d
    }

    /// 实体交互范围（对应原版 `entity_interaction_range` 属性的默认值）。
    pub const fn entity_interaction_range(&self) -> f64 {
        3.0
    }

    /// 玩家是否能触及目标实体：眼睛位置到实体包围盒最近点的距离
    /// 须在实体交互范围内（对应原版 canInteractWithEntity）。
    /// 攻击与右键实体共用此检查，防止改过的客户端超距打击实体。
    pub fn can_interact_with_entity(&self, target: &dyn EntityBase) -> bool {
        let d = self.entity_interaction_range();
        let eye = self.eye_position();
        let target_box = target.get_entity().bounding_box.load();
        target_box.squared_magnitude(eye) <= d * d
    }

    #[must_use]
    pub fn may_build(&self) -> bool {
        self.abilities.lock().is_ok_and(|a| a.allow_modify_world)
    }

    pub fn kick(&self, message: &TextComponent) {
        if let Some(server) = self.world().server.upgrade()
            && let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
        {
            let mut event = crate::plugin::api::events::player::player_kick::PlayerKickEvent::new(
                player_arc,
                message.clone().to_pretty_console(),
            );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        self.client.try_kick(message);
    }

    /// 将最近操作时间更新为当前时刻。请在玩家移动、聊天等操作时调用。
    pub fn update_last_action_time(&self) {
        self.last_action_time.store(std::time::Instant::now());
    }

    /// 检查发送聊天消息或命令是否构成刷屏。
    pub fn check_chat_spam(&self, server: &Server, spam_type: SpamType) -> bool {
        let anti_spam = &server.advanced_config.chat.anti_spam;
        if !anti_spam.enabled {
            return false;
        }

        if anti_spam.ops_bypass && self.permission_lvl.load() > PermissionLvl::Zero {
            return false;
        }

        let threshold = match spam_type {
            SpamType::Chat => anti_spam.chat_threshold_ticks(),
            SpamType::Command => anti_spam.command_threshold_ticks(),
        };

        let new_count = self
            .chat_spam_tick_count
            .fetch_add(anti_spam.message_cost, Ordering::SeqCst)
            + anti_spam.message_cost;

        if new_count > threshold {
            warn!(
                "玩家 {} 因刷屏被踢出（刷屏分数：{}/{}）",
                self.gameprofile.name, new_count, threshold
            );
            self.kick(&TextComponent::translate(
                translation::java::DISCONNECT_SPAM,
                [],
            ));
            return true;
        }

        false
    }

    #[must_use]
    pub fn can_eat(&self, can_always_eat: bool) -> bool {
        self.abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invulnerable
            || can_always_eat
            || self.hunger_manager.level.load() < 20
    }

    pub fn can_food_heal(&self) -> bool {
        let health = self.living_entity.health.load();
        let max_health = self.living_entity.get_max_health();
        health > 0.0 && health < max_health
    }

    pub fn add_exhaustion(&self, exhaustion: f32) {
        if self
            .abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invulnerable
        {
            return;
        }
        let mut exhaustion_event =
            crate::plugin::api::events::entity::entity_exhaustion::EntityExhaustionEvent::new(
                self.entity_id(),
                exhaustion,
            );
        if let Some(server) = self.world().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut exhaustion_event);
        }
        if exhaustion_event.cancelled {
            return;
        }
        self.hunger_manager
            .add_exhaustion(exhaustion_event.exhaustion);
    }

    pub fn heal(&self, additional_health: f32) {
        self.living_entity.heal(additional_health);
        self.send_health();
    }

    pub fn damage(
        &self,
        caller: &dyn crate::entity::EntityBase,
        amount: f32,
        damage_type: papokin_data::damage::DamageType,
    ) -> bool {
        self.damage_with_context(caller, amount, damage_type, None, None, None)
    }

    pub fn damage_with_context(
        &self,
        caller: &dyn crate::entity::EntityBase,
        amount: f32,
        damage_type: papokin_data::damage::DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn crate::entity::EntityBase>,
        cause: Option<&dyn crate::entity::EntityBase>,
    ) -> bool {
        self.damage_with_resolved_context(
            caller,
            amount,
            &papokin_data::damage_ext::ResolvedDamageType::Vanilla(damage_type),
            position,
            source,
            cause,
        )
    }

    /// 使用已解析的（原版或插件注册的自定义）伤害类型造成伤害
    /// 伤害类型。
    pub fn damage_resolved(
        &self,
        caller: &dyn crate::entity::EntityBase,
        amount: f32,
        damage_type: &papokin_data::damage_ext::ResolvedDamageType,
    ) -> bool {
        self.damage_with_resolved_context(caller, amount, damage_type, None, None, None)
    }

    pub fn damage_with_resolved_context(
        &self,
        caller: &dyn crate::entity::EntityBase,
        amount: f32,
        damage_type: &papokin_data::damage_ext::ResolvedDamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn crate::entity::EntityBase>,
        cause: Option<&dyn crate::entity::EntityBase>,
    ) -> bool {
        if self
            .abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invulnerable
            && !damage_type.is(papokin_data::damage::DamageType::GENERIC_KILL)
            && !damage_type.is(papokin_data::damage::DamageType::OUT_OF_WORLD)
        {
            return false;
        }
        self.living_entity.damage_with_resolved_context(
            caller,
            amount,
            damage_type,
            position,
            source,
            cause,
        )
    }

    pub fn damage_generic(&self, amount: f32) -> bool {
        use papokin_data::damage::DamageType;
        self.living_entity.damage(self, amount, DamageType::GENERIC)
    }

    pub fn kill(&self) {
        use papokin_data::damage::DamageType;
        let health = self.living_entity.health.load();
        self.living_entity
            .damage(self, health + 10.0, DamageType::OUT_OF_WORLD);
    }

    pub fn send_health(&self) {
        if !self.has_client_loaded() {
            return;
        }

        self.client.try_send_packet(&CSetHealth::new(
            self.living_entity.health.load(),
            self.hunger_manager.level.load().into(),
            self.hunger_manager.saturation.load(),
        ));
    }

    pub fn tick_health(&self) {
        if !self.has_client_loaded() {
            return;
        }

        let health = self.living_entity.health.load() as i32;
        let food = self.hunger_manager.level.load();
        let saturation = self.hunger_manager.saturation.load();

        let last_health = self.last_sent_health.load(Ordering::Relaxed);
        let last_food = self.last_sent_food.load(Ordering::Relaxed);
        let last_saturation = self.last_food_saturation.load(Ordering::Relaxed);

        if health != last_health || food != last_food || (saturation == 0.0) != last_saturation {
            self.last_sent_health.store(health, Ordering::Relaxed);
            self.last_sent_food.store(food, Ordering::Relaxed);
            self.last_food_saturation
                .store(saturation == 0.0, Ordering::Relaxed);
            self.send_health();
        }
    }

    pub fn tick_raid_omen(&self) {
        if self.is_spectator() {
            return;
        }

        if let Some(bad_omen) = self.get_effect(&StatusEffect::BAD_OMEN)
            && !self.has_effect(&StatusEffect::RAID_OMEN)
        {
            let world = self.world();
            if !world.dimension.can_start_raid {
                return;
            }
            let player_pos = self.living_entity.entity.block_pos.load();
            let pos_f64 = self.living_entity.entity.pos.load();

            let village_pos = world
                .villager_poi
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get_nearest_job_site(player_pos, 64)
                .or_else(|| {
                    world.raids.try_lock().ok().and_then(|raids| {
                        raids
                            .get_nearby_raid(&player_pos, 64.0 * 64.0)
                            .map(|r| r.center)
                    })
                });

            if let Some(pos) = village_pos {
                if let Some(p) = self.world().get_player_by_uuid(self.gameprofile.id) {
                    let bad_omen_amplifier = bad_omen.amplifier;
                    p.living_entity.remove_effect(&StatusEffect::BAD_OMEN);
                    p.set_raid_omen_position(pos);
                    let effect = Effect {
                        effect_type: &StatusEffect::RAID_OMEN,
                        duration: 600,
                        amplifier: bad_omen_amplifier,
                        ambient: false,
                        show_particles: true,
                        show_icon: true,
                        blend: true,
                    };
                    p.add_effect(effect);
                }
                world.play_sound(Sound::BlockBellResonate, SoundCategory::Neutral, &pos_f64);
            }
        }
    }

    pub fn set_health(&self, health: f32) {
        self.living_entity.set_health(health);
        self.send_health();
    }

    pub fn set_max_health(&self, max_health: f32) {
        self.living_entity.set_max_health(max_health);
        self.send_health();
    }

    pub fn get_food_level(&self) -> u8 {
        self.hunger_manager.level.load()
    }

    pub fn set_food_level(&self, food_level: u8) {
        let mut food_event =
            crate::plugin::api::events::entity::food_level_change::FoodLevelChangeEvent::new(
                self.living_entity.entity.entity_id,
                food_level,
            );
        if let Some(server) = self.world().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut food_event);
        }
        if food_event.cancelled {
            return;
        }
        self.hunger_manager.set_level(food_event.food_level);
        self.send_health();
    }

    pub fn get_saturation(&self) -> f32 {
        self.hunger_manager.saturation.load()
    }

    pub fn set_saturation(&self, saturation: f32) {
        self.hunger_manager.set_saturation(saturation);
        self.send_health();
    }

    pub fn set_allow_flight(&self, allow: bool) {
        self.abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .allow_flying = allow;
        self.send_abilities_update();
    }

    pub fn set_flying(&self, flying: bool) {
        if flying {
            self.living_entity.fall_distance.store(0.0);
        }
        self.abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .flying = flying;
        self.send_abilities_update();
    }

    pub fn set_fly_speed(&self, speed: f32) {
        self.abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fly_speed = speed;
        self.send_abilities_update();
    }

    pub fn set_walk_speed(&self, speed: f32) {
        self.abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .walk_speed = speed;
        self.send_abilities_update();
    }

    pub fn set_invulnerable(&self, invulnerable: bool) {
        self.abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invulnerable = invulnerable;
        self.send_abilities_update();
    }

    pub fn get_exhaustion(&self) -> f32 {
        self.hunger_manager.get_exhaustion()
    }

    pub fn set_exhaustion(&self, exhaustion: f32) {
        self.hunger_manager.set_exhaustion(exhaustion);
        self.send_health();
    }

    pub fn get_absorption(&self) -> f32 {
        self.living_entity.get_absorption()
    }

    pub fn set_absorption(&self, absorption: f32) {
        self.living_entity.set_absorption(absorption);
    }

    pub fn get_ip(&self) -> String {
        self.client.address.to_string()
    }

    pub async fn respawn(self: &Arc<Self>) {
        let is_bed_spawn = self
            .respawn_point
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        self.world().respawn_player(self, false).await;
        // 重生后通知，在玩家被移动到
        // 重生位置。
        let respawn_location = self.position();
        if let Some(server) = self.world().server.upgrade() {
            let mut post_respawn =
                crate::plugin::api::events::player::player_post_respawn::PlayerPostRespawnEvent::new(
                    self.clone(),
                    respawn_location,
                    is_bed_spawn,
                );
            server.plugin_manager.fire(&server, &mut post_respawn).await;
        }
        // 客户端在重生时重建了属性状态，因此要发送当前持有的
        // 武器修饰符。
        crate::entity::attributes::send_attribute_updates_for_living(
            &self.living_entity,
            vec![Attributes::ATTACK_SPEED, Attributes::ATTACK_DAMAGE],
        );
    }

    pub fn ban(&self, server: &Server, reason: Option<TextComponent>) {
        self.ban_explicit(server, reason, None, None, true, true);
    }

    pub fn ban_explicit(
        &self,
        server: &Server,
        reason: Option<TextComponent>,
        source: Option<String>,
        expires: Option<time::OffsetDateTime>,
        kick_if_online: bool,
        log_to_console: bool,
    ) {
        let string_reason = reason.clone().map_or_else(
            || "已被管理员封禁。".to_string(),
            papokin_util::text::TextComponent::get_text,
        );
        let source_str = source.unwrap_or_else(|| "Plugin".to_string());

        if log_to_console {
            tracing::info!(
                "封禁玩家 {}（{}），来源 {}，原因：{}",
                self.gameprofile.name,
                self.gameprofile.id,
                source_str,
                string_reason
            );
        }

        {
            let mut banned_players = server
                .data
                .banned_player_list
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            banned_players
                .banned_players
                .retain(|entry| entry.uuid != self.gameprofile.id);

            banned_players.banned_players.push(
                crate::data::banlist_serializer::BannedPlayerEntry::new(
                    &self.gameprofile,
                    source_str,
                    expires,
                    string_reason,
                ),
            );

            banned_players.save();
        };

        if kick_if_online {
            let kick_reason = reason.unwrap_or_else(|| {
                TextComponent::translate(translation::java::MULTIPLAYER_DISCONNECT_BANNED, [])
            });

            self.kick(&kick_reason);
        }
    }

    pub fn ban_ip(&self, server: &Server, reason: Option<TextComponent>) {
        self.ban_ip_explicit(server, reason, None, None, true, true);
    }

    pub fn ban_ip_explicit(
        &self,
        server: &Server,
        reason: Option<TextComponent>,
        source: Option<String>,
        expires: Option<time::OffsetDateTime>,
        kick_matching_players: bool,
        log_to_console: bool,
    ) {
        let string_reason = reason.clone().map_or_else(
            || "已被管理员封禁。".to_string(),
            papokin_util::text::TextComponent::get_text,
        );
        let source_str = source.unwrap_or_else(|| "Plugin".to_string());
        let target_ip = self.client.address.ip();

        if log_to_console {
            tracing::info!(
                "封禁 IP {}，来源 {}，原因：{}",
                target_ip,
                source_str,
                string_reason
            );
        }

        {
            let mut banned_ips = server
                .data
                .banned_ip_list
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            banned_ips.banned_ips.retain(|entry| entry.ip != target_ip);

            banned_ips
                .banned_ips
                .push(crate::data::banlist_serializer::BannedIpEntry::new(
                    target_ip,
                    source_str,
                    expires,
                    string_reason,
                ));

            banned_ips.save();
        };

        if kick_matching_players {
            let kick_reason = reason.unwrap_or_else(|| {
                TextComponent::translate(translation::java::MULTIPLAYER_DISCONNECT_IP_BANNED, [])
            });

            let affected = server.get_players_by_ip(target_ip);
            for target in affected {
                target.kick(&kick_reason);
            }
        }
    }

    pub fn tick_client_load_timeout(&self) {
        if !self.supports_player_loaded() {
            return;
        }
        if !self.client_loaded.load(Ordering::Relaxed) {
            let timeout = self.client_loaded_timeout.load(Ordering::Relaxed);
            self.client_loaded_timeout
                .store(timeout.saturating_sub(1), Ordering::Relaxed);
        }
    }

    pub fn send_combat_death(&self, death_msg: &TextComponent) {
        self.client
            .try_send_packet(&CCombatDeath::new(self.entity_id().into(), death_msg));
    }

    pub fn handle_killed(&self, death_msg: &TextComponent) {
        self.trigger_advancement(
            crate::entity::player::advancement::trigger::AdvancementTrigger::PlayerKilled,
        );
        let block_pos = self.position().to_block_pos();

        let keep_inventory = { self.world().level_info.load().game_rules.keep_inventory };

        if !keep_inventory {
            let mut main_inv = self
                .inventory()
                .main_inventory
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for item in main_inv.iter_mut() {
                if !item.is_empty() {
                    let stack = std::mem::replace(item, ItemStack::EMPTY.clone());
                    self.increment_stat(
                        statistics::StatisticCategory::Dropped,
                        stack.item.id as i32,
                        stack.item_count as i32,
                    );
                    self.increment_custom_stat(
                        statistics::CustomStatistic::Drop,
                        stack.item_count as i32,
                    );
                    self.world().drop_stack(&block_pos, stack);
                }
            }
        }

        // 死亡时重置氧气供应与溺水刻
        self.breath_manager.reset(self);

        self.set_client_loaded(false);
        self.send_combat_death(death_msg);
        self.send_health();
    }

    pub fn set_gamemode(self: &Arc<Self>, gamemode: GameMode) -> bool {
        // 我们可以毫无问题地发送相同的游戏模式，但为什么要浪费带宽呢？
        // assert_ne!(
        //    self.gamemode.load(),
        //    gamemode,
        //    "尝试将游戏模式设置为当前已生效的游戏模式"
        // );
        // 游戏模式相同时我们为什么要 panic？原版只是提前退出。
        if self.gamemode.load() == gamemode {
            return false;
        }
        let Some(server) = self.world().server.upgrade() else {
            return false;
        };

        let mut event = PlayerGamemodeChangeEvent {
            player: self.clone(),
            new_gamemode: gamemode,
            previous_gamemode: self.gamemode.load(),
            cancelled: false,
        };
        server.plugin_manager.fire_blocking(&server, &mut event);
        if event.cancelled {
            return false;
        }

        let gamemode = event.new_gamemode;
        self.gamemode.store(gamemode);
        // TODO: 等 Mojang 修复后再修复此处
        // 这是为了保持纯粹的 Mojang 原版体验而有意为之
        // self.previous_gamemode.store(self.previous_gamemode.load());
        {
            // 使用另一个作用域，以便立即解锁 `abilities`。
            let mut abilities = self
                .abilities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            abilities.set_for_gamemode(gamemode);
        };
        self.send_abilities_update();

        if gamemode == GameMode::Creative {
            self.get_entity().extinguish();
            self.get_entity().set_on_fire(false);
        }

        // 切换到旁观模式时停止鞘翅飞行并重置潜行
        if gamemode == GameMode::Spectator {
            let entity = self.get_entity();
            if entity.is_fall_flying() {
                entity.set_fall_flying(false);
            }
            if entity.is_sneaking() {
                entity.set_sneaking(false);
            }
            entity.on_ground.store(false, Ordering::Relaxed);
            self.living_entity.fall_distance.store(0.0);
        }

        if gamemode != GameMode::Spectator && self.camera_target_id.load().is_some() {
            self.camera_target_id.store(None);
            self.try_send_client_packet(&CSetCamera::new(self.entity_id().into()));
        }

        self.living_entity.entity.invulnerable.store(
            matches!(gamemode, GameMode::Creative | GameMode::Spectator),
            Ordering::Relaxed,
        );
        self.living_entity
            .entity
            .no_physics
            .store(gamemode == GameMode::Spectator, Ordering::Relaxed);
        self.living_entity
            .entity
            .world
            .load()
            .broadcast_packet_all(&CPlayerInfoUpdate::new(
                PlayerInfoFlags::UPDATE_GAME_MODE.bits(),
                &[papokin_protocol::java::client::play::Player {
                    uuid: self.gameprofile.id,
                    actions: &[PlayerAction::UpdateGameMode((gamemode as i32).into())],
                }],
            ));

        self.client.try_send_packet(&CGameEvent::new(
            GameEvent::ChangeGameMode,
            gamemode as i32 as f32,
        ));

        true
    }

    /// 将玩家的皮肤层和所使用的手发送给所有玩家。
    pub fn send_client_information(&self) {
        let config = self.config.load();
        self.living_entity.entity.set_synced_data(
            papokin_data::tracked_data::player::PLAYER_MODE_CUSTOMISATION,
            config.skin_parts,
        );
        self.living_entity.entity.set_synced_data(
            papokin_data::tracked_data::player::PLAYER_MAIN_HAND,
            config.main_hand as u8,
        );
        self.living_entity.entity.set_synced_data(
            papokin_data::tracked_data::player::PLAYER_MODE_CUSTOMIZATION_ID,
            config.skin_parts,
        );
        self.living_entity.entity.set_synced_data(
            papokin_data::tracked_data::player::MAIN_ARM_ID,
            config.main_hand as u8,
        );
    }

    pub fn can_harvest(&self, state: &BlockState, block: &'static Block) -> bool {
        !state.tool_required() || self.inventory().held_item().is_correct_for_drops(block)
    }

    /// Pumpkin 存储 `mining_efficiency` 修正值所使用的 ID
    const EFFICIENCY_ATTRIBUTE_MODIFIER_ID: &'static str = "minecraft:enchantment.efficiency";

    fn sync_mining_efficiency(&self) {
        let level = self
            .inventory()
            .held_item()
            .get_enchantment_level(&Enchantment::EFFICIENCY);
        if self
            .synced_mining_efficiency_level
            .swap(level, Ordering::Relaxed)
            == level
        {
            return;
        }
        self.living_entity
            .update_attribute(&Attributes::MINING_EFFICIENCY, |inst| {
                if level > 0 {
                    inst.add_or_replace_modifier(Modifier {
                        id: Self::EFFICIENCY_ATTRIBUTE_MODIFIER_ID.to_string(),
                        amount: f64::from(level * level + 1),
                        operation: ModifierOperation::Add,
                    });
                } else {
                    inst.remove_modifier(Self::EFFICIENCY_ATTRIBUTE_MODIFIER_ID);
                }
            });
        crate::entity::attributes::send_attribute_updates_for_living(
            &self.living_entity,
            vec![Attributes::MINING_EFFICIENCY],
        );
    }

    pub fn get_mining_speed(&self, block: &'static Block) -> f32 {
        self.sync_mining_efficiency();
        let held_item = self.inventory.held_item();
        let mut speed = held_item.get_speed(block);
        // Effi 仅在工具对方块的破坏速度已高于 1 时才生效（意味着
        // 正确的工具）
        if speed > 1.0 {
            speed += self
                .living_entity
                .get_attribute_value(&Attributes::MINING_EFFICIENCY) as f32;
        }
        // 急迫
        if self.living_entity.has_effect(&StatusEffect::HASTE)
            || self.living_entity.has_effect(&StatusEffect::CONDUIT_POWER)
        {
            speed *= ((self.get_haste_amplifier() + 1) as f32).mul_add(0.2, 1.0);
        }
        // 疲劳
        if let Some(fatigue) = self.living_entity.get_effect(&StatusEffect::MINING_FATIGUE) {
            let fatigue_speed = match fatigue.amplifier {
                0 => 0.3,
                1 => 0.09,
                2 => 0.0027,
                _ => 8.1E-4,
            };
            speed *= fatigue_speed;
        }
        // TODO: 处理在水中的情况
        if !self.living_entity.entity.on_ground.load(Ordering::Relaxed) {
            speed /= 5.0;
        }
        speed
    }

    fn get_haste_amplifier(&self) -> u32 {
        let mut i = 0;
        let mut j = 0;
        if let Some(effect) = self.living_entity.get_effect(&StatusEffect::HASTE) {
            i = effect.amplifier;
        }
        if let Some(effect) = self.living_entity.get_effect(&StatusEffect::CONDUIT_POWER) {
            j = effect.amplifier;
        }
        u32::from(i.max(j))
    }

    pub fn send_message(
        &self,
        message: &TextComponent,
        chat_type: u8,
        sender_name: &TextComponent,
        target_name: Option<&TextComponent>,
    ) {
        self.try_send_client_packet(&CDisguisedChatMessage::new(
            message,
            (chat_type + 1).into(),
            sender_name,
            target_name,
        ));
    }

    pub fn drop_item(&self, item_stack: ItemStack) {
        self.increment_stat(
            statistics::StatisticCategory::Dropped,
            item_stack.item.id as i32,
            item_stack.item_count as i32,
        );
        self.increment_custom_stat(
            statistics::CustomStatistic::Drop,
            item_stack.item_count as i32,
        );
        let item_pos = self.living_entity.entity.pos.load()
            + Vector3::new(0.0, self.living_entity.entity.get_eye_height() - 0.3, 0.0);
        let entity = Entity::new(self.world(), item_pos, &EntityType::ITEM);

        let pitch = f64::from(self.living_entity.entity.pitch.load()).to_radians();
        let yaw = f64::from(self.living_entity.entity.yaw.load()).to_radians();
        let pitch_sin = pitch.sin();
        let pitch_cos = pitch.cos();
        let yaw_sin = yaw.sin();
        let yaw_cos = yaw.cos();
        let horizontal_offset = rand::random::<f64>() * TAU;
        let l = 0.02 * rand::random::<f64>();

        let velocity = Vector3::new(
            (-yaw_sin * pitch_cos).mul_add(0.3, horizontal_offset.cos() * l),
            (rand::random::<f64>() - rand::random::<f64>())
                .mul_add(0.1, (-pitch_sin).mul_add(0.3, 0.1)),
            (yaw_cos * pitch_cos).mul_add(0.3, horizontal_offset.sin() * l),
        );

        // TODO: 将多个堆叠合并
        let item_entity = Arc::new(ItemEntity::new_with_velocity(
            entity, item_stack, velocity, 40,
        ));
        self.world().spawn_entity(item_entity);
    }

    pub fn drop_held_item(&self, drop_stack: bool) {
        let mut item_stack = self.inventory().held_item();

        if item_stack.is_empty() {
            return;
        }

        let drop_amount = if drop_stack { item_stack.item_count } else { 1 };
        let dropped_stack = item_stack.copy_with_count(drop_amount);

        if let Some(server) = self.world().server.upgrade()
            && let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
        {
            let mut event =
                crate::plugin::api::events::player::player_drop_item::PlayerDropItemEvent::new(
                    player_arc,
                    dropped_stack.item.registry_key.to_string(),
                    dropped_stack.item_count,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }

        item_stack.decrement(drop_amount);
        let updated_stack = item_stack.clone();
        self.inventory().set_held_item(updated_stack.clone());

        self.drop_item(dropped_stack);

        let inv: Arc<dyn Inventory> = self.inventory.clone();
        let screen_binding = self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut screen_handler = screen_binding
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let selected_slot = self.inventory.get_selected_slot();
        if let Some(slot_index) = screen_handler.get_slot_index(&inv, selected_slot as usize) {
            screen_handler.set_received_stack(slot_index, updated_stack);
            screen_handler.send_content_updates();
        }
    }

    pub fn swap_item(&self) {
        if let Some(server) = self.world().server.upgrade()
            && let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
        {
            let mut event = crate::plugin::api::events::player::player_swap_hands::PlayerSwapHandItemsEvent::new(player_arc);
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        let (main_hand_item, off_hand_item) = self.inventory.swap_item();
        let equipment = &[
            (EquipmentSlot::MAIN_HAND, main_hand_item),
            (EquipmentSlot::OFF_HAND, off_hand_item),
        ];
        self.living_entity.send_equipment_changes(equipment);
        // todo this.player.stopUsingItem();
    }

    #[must_use]
    pub fn is_text_filtering_enabled(&self) -> bool {
        self.config.load().text_filtering
    }

    pub fn send_chat_message(
        self: &Arc<Self>,
        tracked: &crate::net::chat::OutgoingChatMessage,
        filtered: bool,
        chat_type: papokin_protocol::codec::var_int::VarInt,
        sender_name: &TextComponent,
        target_name: Option<&TextComponent>,
    ) {
        tracked.send_to_player(self, filtered, chat_type, sender_name, target_name);
    }

    pub fn send_system_message(&self, text: &TextComponent) {
        self.send_system_message_raw(text, false);
    }

    pub fn send_system_message_raw(&self, text: &TextComponent, overlay: bool) {
        let je_packet = CSystemChatMessage::new(text, overlay);
        self.client.try_send_packet(&je_packet);
    }

    pub fn tick_experience(&self) {
        if !self.has_client_loaded() {
            return;
        }

        let level = self.experience_level.load(Ordering::Relaxed);
        if self.last_sent_xp.load(Ordering::Relaxed) != level {
            let progress = self.experience_progress.load();
            let points = self.experience_points.load(Ordering::Relaxed);

            self.last_sent_xp.store(level, Ordering::Relaxed);

            self.client.try_send_packet(&CSetExperience::new(
                progress.clamp(0.0, 1.0),
                level.into(),
                points.into(),
            ));
        }
    }

    pub fn tick_maps(&self, server: &Server) {
        use papokin_data::data_component_impl::MapIdImpl;
        use papokin_data::item::Item;

        for hand in Hand::all() {
            let stack = self.inventory().get_stack_in_hand(hand);

            if stack.item.id == Item::FILLED_MAP.id
                && let Some(map_id_comp) = stack.get_data_component::<MapIdImpl>()
            {
                let map_id = map_id_comp.id;
                if let Some(map_data_arc) = server.map_manager.get_map(map_id)
                    && let Ok(mut map_data) = map_data_arc.try_lock()
                {
                    map_data.update(self);

                    let tick_count = self.tick_counter.load(Ordering::Relaxed);
                    if map_data.dirty || tick_count % 10 == 0 {
                        let scale = 1 << map_data.scale;
                        let pos = self.position();
                        let dx = pos.x - map_data.center_x as f64;
                        let dz = pos.z - map_data.center_z as f64;

                        let raw_x = dx / scale as f64 * 2.0;
                        let raw_z = dz / scale as f64 * 2.0;
                        let is_off_map = !(-127.0..=127.0).contains(&raw_x)
                            || !(-127.0..=127.0).contains(&raw_z);

                        let icon_x = raw_x.clamp(-128.0, 127.0) as i8;
                        let icon_z = raw_z.clamp(-128.0, 127.0) as i8;

                        let yaw = self.living_entity.entity.yaw.load();
                        let icon_direction =
                            ((((yaw * 16.0 / 360.0).round() as i32 + 8) % 16 + 16) % 16) as i8;

                        let decoration_type = if is_off_map {
                            &papokin_data::map_decoration::MapDecorationType::PLAYER_OFF_MAP
                        } else {
                            &papokin_data::map_decoration::MapDecorationType::PLAYER
                        };

                        let mut icons = vec![MapIcon {
                            icon_type: VarInt(decoration_type.id as i32),
                            x: icon_x,
                            z: icon_z,
                            direction: icon_direction,
                            display_name: None,
                        }];
                        icons.extend(map_data.decorations.iter().map(|decoration| {
                            MapIcon {
                                icon_type: VarInt(decoration.icon_type),
                                x: decoration.x,
                                z: decoration.z,
                                direction: decoration.direction,
                                display_name: decoration
                                    .display_name
                                    .as_ref()
                                    .map(|name| TextComponent::text(name.clone())),
                            }
                        }));

                        let data = map_data.dirty.then(|| MapPatch {
                            columns: 128,
                            rows: 128,
                            x: 0,
                            z: 0,
                            data: &*map_data.colors,
                        });

                        self.client.try_send_packet(&CMapItemData {
                            map_id: VarInt(map_id),
                            scale: map_data.scale,
                            tracking_position: true,
                            locked: map_data.locked,
                            icons: Some(&icons),
                            data,
                        });
                        map_data.dirty = false;
                    }
                }
            }
        }
    }

    /// 设置玩家的经验等级并通知客户端。
    pub fn set_experience(&self, level: i32, progress: f32, points: i32) {
        let old_level = self.experience_level.load(Ordering::Relaxed);
        if old_level != level
            && let Some(server) = self.world().server.upgrade()
            && let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
        {
            let mut event = crate::plugin::api::events::player::player_level_change::PlayerLevelChangeEvent::new(
                player_arc,
                old_level,
                level,
            );
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        // TODO: 这些应作为整体保持原子性，而非各自独立；做一个包含它们的结构体。否则可能引发 ABA 问题
        self.experience_level.store(level, Ordering::Relaxed);
        self.experience_progress.store(progress.clamp(0.0, 1.0));
        self.experience_points.store(points, Ordering::Relaxed);
        self.last_sent_xp.store(-1, Ordering::Relaxed);
        self.tick_experience();

        if self.has_client_loaded() {
            self.try_send_client_packet(&CSetExperience::new(
                progress.clamp(0.0, 1.0),
                level.into(),
                points.into(),
            ));
        }
    }

    /// 直接设置玩家的经验等级。
    pub fn set_experience_level(&self, new_level: i32, keep_progress: bool) {
        let progress = self.experience_progress.load();
        let mut points = self.experience_points.load(Ordering::Relaxed);

        // 如果 `keep_progress` 为 `true`，则按比例计算保持相同进度所需的点数。
        if keep_progress {
            // 获取我们当前的等级
            let current_level = self.experience_level.load(Ordering::Relaxed);
            let current_max_points = experience::points_in_level(current_level);
            // 计算新等级的最大值
            let new_max_points = experience::points_in_level(new_level);
            // 计算缩放因子
            let scale = new_max_points as f32 / current_max_points as f32;
            // 缩放分数（原版似乎不重新计算进度，因此我们也不）
            points = (points as f32 * scale) as i32;
        }

        self.set_experience(new_level, progress, points);
    }

    pub fn add_effect(&self, effect: Effect) {
        self.living_entity.add_effect(effect);
    }

    pub fn has_effect(&self, effect_type: &'static StatusEffect) -> bool {
        self.living_entity.has_effect(effect_type)
    }

    pub fn get_effect(&self, effect_type: &'static StatusEffect) -> Option<Effect> {
        self.living_entity.get_effect(effect_type)
    }

    pub fn get_active_effects(&self) -> Vec<Effect> {
        let effects = self
            .living_entity
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        effects.values().cloned().collect()
    }

    #[must_use]
    pub fn get_raid_omen_position(&self) -> Option<BlockPos> {
        self.raid_omen_position.load()
    }

    pub fn set_raid_omen_position(&self, pos: BlockPos) {
        self.raid_omen_position.store(Some(pos));
    }

    pub fn clear_raid_omen_position(&self) {
        self.raid_omen_position.store(None);
    }

    pub fn send_active_effects(&self) {
        let effects: Vec<_> = self
            .living_entity
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        for effect in &effects {
            self.send_effect(effect);
        }
    }

    /**
     * 向玩家发送仅客户端可见的效果。
     * 它不会在服务器上被追踪。
     */
    pub fn send_effect(&self, effect: &Effect) {
        let mut flag: i8 = 0;

        if effect.ambient {
            flag |= 1;
        }
        if effect.show_particles {
            flag |= 2;
        }
        if effect.show_icon {
            flag |= 4;
        }
        if effect.blend {
            flag |= 8;
        }

        let effect_id = VarInt(i32::from(effect.effect_type.id));
        self.try_send_client_packet(&CUpdateMobEffect::new(
            self.entity_id().into(),
            effect_id,
            effect.amplifier.into(),
            effect.duration.into(),
            flag,
        ));
    }

    pub fn remove_effect(&self, effect_type: &'static StatusEffect) -> bool {
        let effect_id = VarInt(i32::from(effect_type.id));
        self.try_send_client_packet(
            &papokin_protocol::java::client::play::CRemoveMobEffect::new(
                self.entity_id().into(),
                effect_id,
            ),
        );

        self.living_entity.remove_effect(effect_type)

        // TODO 广播元数据
    }

    pub fn remove_all_effects(&self) -> bool {
        let mut succeeded = false;
        let mut effect_list = vec![];
        let effects: Vec<_> = self
            .living_entity
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .copied()
            .collect();
        for effect in effects {
            effect_list.push(effect);
            let effect_id = VarInt(i32::from(effect.id));
            self.try_send_client_packet(
                &papokin_protocol::java::client::play::CRemoveMobEffect::new(
                    self.entity_id().into(),
                    effect_id,
                ),
            );
            succeeded = true;
        }

        // 需要在此之后移除效果，否则在 for 循环中执行会造成死锁。
        for effect in effect_list {
            self.living_entity.remove_effect(effect);
        }

        succeeded
    }

    /// 为玩家增加经验等级。
    pub fn add_experience_levels(&self, added_levels: i32) {
        let current_level = self.experience_level.load(Ordering::Relaxed);
        let new_level = current_level + added_levels;
        self.set_experience_level(new_level, true);
    }

    /// 直接设置玩家的经验点数。成功时返回 `true`。
    pub fn set_experience_points(&self, new_points: i32) -> bool {
        let current_points = self.experience_points.load(Ordering::Relaxed);

        if new_points == current_points {
            return true;
        }

        let current_level = self.experience_level.load(Ordering::Relaxed);
        let max_points = experience::points_in_level(current_level);

        if new_points < 0 || new_points > max_points {
            return false;
        }

        let progress = new_points as f32 / max_points as f32;
        self.set_experience(current_level, progress, new_points);
        true
    }

    /// 为玩家增加经验点数。
    pub fn add_experience_points(self: &Arc<Self>, mut added_points: i32) {
        let server = self.world().server.upgrade();
        if let Some(server) = server {
            let mut event = PlayerExpChangeEvent::new(self.clone(), added_points);
            server.plugin_manager.fire_blocking(&server, &mut event);
            added_points = event.amount;
        }

        let current_level = self.experience_level.load(Ordering::Relaxed);
        let current_points = self.experience_points.load(Ordering::Relaxed);

        let total_exp = experience::points_to_level(current_level) as i64 + current_points as i64;
        let new_total_exp = total_exp + added_points as i64;
        let safe_new_total = new_total_exp.clamp(0, i32::MAX as i64) as i32;

        let (new_level, new_points) = experience::total_to_level_and_points(safe_new_total);
        let progress = experience::progress_in_level(new_points, new_level);

        self.set_experience(new_level, progress, new_points);
    }

    pub fn apply_mending_from_xp(&self, mut xp: i32) -> i32 {
        if xp <= 0 {
            return xp;
        }

        let mut candidates: Vec<(usize, EquipmentSlot, ItemStack)> = Vec::new();

        let selected_slot = self.inventory.get_selected_slot() as usize;
        let mut slot_pairs: Vec<(usize, EquipmentSlot)> = vec![
            (selected_slot, EquipmentSlot::MAIN_HAND),
            (PlayerInventory::OFF_HAND_SLOT, EquipmentSlot::OFF_HAND),
        ];
        for (slot_index, slot) in self.inventory.equipment_slots.iter() {
            if slot.is_armor_slot() {
                slot_pairs.push((*slot_index, slot.clone()));
            }
        }

        for (slot_index, equipment_slot) in slot_pairs {
            let stack = self.inventory.get_slot(slot_index);
            if stack.get_enchantment_level(&Enchantment::MENDING) > 0 && stack.get_damage() > 0 {
                candidates.push((slot_index, equipment_slot, stack));
            }
        }

        if candidates.is_empty() {
            return xp;
        }

        let idx = rand::random::<u32>() as usize % candidates.len();
        let (slot_index, equipment_slot, mut stack) = candidates.swap_remove(idx);

        let repaired = stack.repair_item(xp.saturating_mul(2));
        if repaired <= 0 {
            return xp;
        }

        let xp_used = (repaired + 1) / 2;

        if let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
            && let Some(server) = self.world().server.upgrade()
        {
            let mut event =
                crate::plugin::api::events::player::player_item_mend::PlayerItemMendEvent {
                    player: player_arc,
                    item_name: stack.item.registry_key.to_string(),
                    repair_amount: repaired,
                    exp_consumed: xp_used,
                    cancelled: false,
                };
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return xp;
            }
        }

        let updated_stack = stack.clone();
        self.inventory.set_slot(slot_index, updated_stack.clone());

        xp = xp.saturating_sub(xp_used);

        self.try_send_slot_set_packet(&CSetPlayerInventory::new(
            (slot_index as i32).into(),
            &ItemStackSerializer::from(updated_stack.clone()),
        ));
        self.sync_inventory_to_client();

        self.living_entity
            .send_equipment_changes(&[(equipment_slot, updated_stack)]);

        xp
    }

    pub fn increment_screen_handler_sync_id(&self) {
        let current_id = self.screen_handler_sync_id.load(Ordering::Relaxed);
        self.screen_handler_sync_id
            .store(current_id % 100 + 1, Ordering::Relaxed);
    }

    pub fn close_handled_screen(&self) {
        let sync_id = {
            let current_handler_guard = self
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let handler = current_handler_guard
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            handler.sync_id()
        };

        self.client
            .try_send_packet(&CCloseContainer::new(sync_id.into()));
        self.on_handled_screen_closed();
    }

    pub fn on_handled_screen_closed(&self) {
        let current_screen_handler: Arc<std::sync::Mutex<dyn ScreenHandler>> = self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();

        let window_type = {
            let mut handler = current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let wt = handler.window_type();
            handler.on_closed(self);
            wt
        };

        let world = self.living_entity.entity.world.load();
        let server = world.server.upgrade();
        if let Some(server) = server
            && let Some(player_arc) = world.get_player_by_uuid(self.gameprofile.id)
        {
            let mut event =
                crate::plugin::api::events::player::inventory_close::InventoryCloseEvent::new(
                    &player_arc,
                    window_type,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
        }

        let player_screen_handler: Arc<std::sync::Mutex<dyn ScreenHandler>> =
            self.player_screen_handler.clone();

        if !Arc::ptr_eq(&player_screen_handler, &current_screen_handler) {
            player_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .copy_shared_slots(current_screen_handler);
        }

        *self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            self.player_screen_handler.clone();
        self.open_container_pos.store(None);
    }

    pub fn on_screen_handler_opened<T: ScreenHandler + ?Sized>(
        &self,
        screen_handler: &std::sync::Mutex<T>,
    ) {
        let mut screen_handler = screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // 商人交易钩子：安装购买/交易闸门，使完成
        // 一次交易会先触发可取消的插件事件。
        if let Some(merchant) = screen_handler
            .as_any_mut()
            .downcast_mut::<MerchantScreenHandler>()
        {
            let world_weak = Arc::downgrade(&self.world());
            let player_uuid = self.gameprofile.id;
            merchant.trade_check = Some(Box::new(
                move |inventory_player, offer, merchant_entity_id| {
                    let _ = inventory_player;
                    let world = world_weak.upgrade()?;
                    let server = world.server.upgrade()?;
                    let player = world.get_player_by_uuid(player_uuid)?;

                    let mut ingredients = vec![offer.base_cost_a.0.as_ref().clone()];
                    if let Some(cost_b) = &offer.cost_b {
                        ingredients.push(cost_b.0.as_ref().clone());
                    }
                    let result = offer.output.0.as_ref().clone();

                    // 先购买（通用村民商人钩子）……
                    let mut purchase_event = crate::plugin::api::events::player::player_purchase::PlayerPurchaseEvent::new(
                        player.clone(),
                        merchant_entity_id,
                        ingredients.clone(),
                        result.clone(),
                    );
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut purchase_event);
                    if purchase_event.cancelled {
                        return None;
                    }

                    // ……然后是村民特有的交易钩子。村民
                    // 经验可被修改并返回给处理函数。
                    let mut trade_event =
                        crate::plugin::api::events::player::player_trade::PlayerTradeEvent::new(
                            player,
                            merchant_entity_id.unwrap_or(-1),
                            ingredients,
                            result,
                            offer.xp,
                        );
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut trade_event);
                    if trade_event.cancelled {
                        return None;
                    }
                    Some(trade_event.villager_experience)
                },
            ));
        }

        screen_handler.add_listener(self.screen_handler_listener.clone());
        screen_handler.update_sync_handler(self.screen_handler_sync_handler.clone());
    }

    pub fn on_rename_item(self: &Arc<Self>, packet: &SRenameItem<'_>) {
        self.update_last_action_time();

        let mut prepare_event =
            crate::plugin::api::events::inventory::prepare_anvil::PrepareAnvilEvent::new(
                self.clone(),
                packet.item_name.to_string(),
                1,
            );
        if let Some(server) = self.world().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut prepare_event);
        }

        let screen_handler_arc = self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut screen_handler = screen_handler_arc
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(anvil_handler) = screen_handler
            .as_any_mut()
            .downcast_mut::<papokin_inventory::anvil::AnvilScreenHandler>()
        {
            anvil_handler.set_item_name(packet.item_name, self.has_infinite_materials());
        }
    }

    pub fn open_handled_screen(
        &self,
        screen_handler_factory: &dyn ScreenHandlerFactory,
        block_pos: Option<BlockPos>,
    ) -> Option<u8> {
        if !self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_any()
            .is::<PlayerScreenHandler>()
        {
            self.close_handled_screen();
        }

        let server = self.world().server.upgrade();
        if let Some(server) = server
            && let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id)
        {
            let mut event =
                crate::plugin::api::events::inventory::inventory_open::InventoryOpenEvent::new(
                    player_arc,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return None;
            }
        }

        self.increment_screen_handler_sync_id();

        if let Some(screen_handler) = screen_handler_factory.create_screen_handler(
            self.screen_handler_sync_id.load(Ordering::Relaxed),
            &self.inventory,
            self,
        ) {
            let screen_handler_temp = screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let sync_id = screen_handler_temp.sync_id();
            let window_type = screen_handler_temp.window_type()?;

            let display_name = screen_handler_factory.get_display_name();
            let java_packet =
                COpenScreen::new(sync_id.into(), (window_type as i32).into(), &display_name);

            self.client.try_send_packet(&java_packet);

            drop(screen_handler_temp);
            self.on_screen_handler_opened(&screen_handler);
            *self
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = screen_handler;
            self.open_container_pos.store(block_pos);
            Some(self.screen_handler_sync_id.load(Ordering::Relaxed))
        } else {
            //TODO: 若为旁观者则发送消息

            None
        }
    }

    pub fn open_handled_screen_direct(
        &self,
        screen_handler: Arc<std::sync::Mutex<dyn ScreenHandler>>,
        title: &TextComponent,
    ) {
        if !self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_any()
            .is::<PlayerScreenHandler>()
        {
            self.close_handled_screen();
        }

        let screen_handler_temp = screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sync_id = screen_handler_temp.sync_id();
        let Some(window_type) = screen_handler_temp.window_type() else {
            return;
        };

        let java_packet = COpenScreen::new(sync_id.into(), (window_type as i32).into(), title);

        self.client.try_send_packet(&java_packet);

        drop(screen_handler_temp);
        self.on_screen_handler_opened(&screen_handler);
        *self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = screen_handler;
        self.open_container_pos.store(None);
    }

    #[allow(clippy::too_many_lines)]
    pub fn on_slot_click(self: &Arc<Self>, packet: SClickSlot, server: &Arc<Server>) {
        self.update_last_action_time();

        let (
            sync_id,
            container_slots,
            allow_grab_items,
            allow_put_items,
            can_use,
            is_slot_valid,
            available_slots,
            clicked_item,
            cursor_item,
            window_type,
        ) = {
            let screen_handler_arc = self
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let screen_handler = screen_handler_arc
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            let b = screen_handler.get_behaviour();
            let sync_id = b.sync_id;
            let container_slots = b.container_slots;
            let allow_grab_items = b.allow_grab_items;
            let allow_put_items = b.allow_put_items;
            let can_use = screen_handler.can_use(self.as_ref());
            let is_slot_valid = screen_handler.is_slot_valid(i32::from(packet.slot));
            let available_slots = b.slots.len();

            let clicked_item = (packet.slot >= 0 && (packet.slot as usize) < b.slots.len())
                .then(|| b.slots[packet.slot as usize].get_cloned_stack());

            let cursor_item = Some(
                b.cursor_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
            );

            let window_type = screen_handler.window_type();

            (
                sync_id,
                container_slots,
                allow_grab_items,
                allow_put_items,
                can_use,
                is_slot_valid,
                available_slots,
                clicked_item,
                cursor_item,
                window_type,
            )
        };

        if i32::from(sync_id) != packet.sync_id.0 {
            return;
        }

        if self.gamemode.load() == GameMode::Spectator {
            let screen_handler_arc = self
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            screen_handler_arc
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .sync_state();
            return;
        }

        if !can_use {
            warn!(
                "玩家 {} 与无效的菜单 {:?} 交互",
                self.gameprofile.name, window_type
            );
            return;
        }

        let slot = packet.slot;

        if !is_slot_valid {
            warn!(
                "玩家 {} 点击了无效的槽位索引：{}，可用槽位：{}",
                self.gameprofile.name, slot, available_slots
            );
            return;
        }

        let cancel_screen = || {
            let screen_handler_arc = self
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            screen_handler_arc
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .cancel();
        };

        let raw_slot = slot; // 目前 raw_slot == slot，因为我们还没有独立的视图/物品栏索引
        let hotbar_button = if matches!(packet.mode, SlotActionType::Swap) {
            packet.button
        } else {
            -1
        };

        let click_type = match packet.mode {
            SlotActionType::Pickup => {
                if packet.button == SClickSlot::BUTTON_LEFT {
                    ClickType::Left
                } else {
                    ClickType::Right
                }
            }
            SlotActionType::QuickMove => {
                if packet.button == SClickSlot::BUTTON_LEFT {
                    ClickType::ShiftLeft
                } else {
                    ClickType::ShiftRight
                }
            }
            SlotActionType::Swap => ClickType::NumberKey(packet.button as u8),
            SlotActionType::Clone => ClickType::Middle,
            SlotActionType::Throw => {
                if packet.button == SClickSlot::BUTTON_DROP_SINGLE {
                    ClickType::Drop
                } else {
                    ClickType::ControlDrop
                }
            }
            SlotActionType::QuickCraft => {
                if [0, 4, 8].contains(&packet.button) {
                    ClickType::Left
                } else if [1, 5, 9].contains(&packet.button) {
                    ClickType::Right
                } else {
                    ClickType::Middle
                }
            }
            SlotActionType::PickupAll => ClickType::DoubleClick,
        };

        send_cancellable_blocking! {{
            server;
            InventoryClickEvent::new(
                self,
                window_type,
                click_type,
                slot,
                raw_slot,
                clicked_item.clone(),
                cursor_item.clone(),
                i32::from(hotbar_button),
            );
            'after: {}
            'cancelled: {
                cancel_screen();
                return;
            }
        }}

        let mut interact_event =
            crate::plugin::api::events::inventory::inventory_interact::InventoryInteractEvent::new(
                self.clone(),
            );
        if let Some(server) = self.world().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut interact_event);
        }
        if interact_event.cancelled {
            cancel_screen();
            return;
        }

        // 与装备槽交换钩子：仅限玩家物品栏界面，盔甲
        // 槽位 5..=8（头/胸/腿/脚）。触发方式是点击盔甲
        // 槽位（此时光标上持有可装备物品），或通过快捷栏交换
        // 到盔甲槽位上。取消即否决此次交换。
        if window_type.is_none()
            && (5..=8).contains(&slot)
            && (matches!(packet.mode, SlotActionType::Swap)
                || (matches!(packet.mode, SlotActionType::Pickup)
                    && cursor_item.as_ref().is_some_and(|cursor| {
                        !cursor.is_empty()
                            && cursor.get_data_component::<EquippableImpl>().is_some()
                    })))
        {
            let slot_name = match slot {
                5 => "head",
                6 => "chest",
                7 => "legs",
                _ => "feet",
            };
            let equipped_item = clicked_item.clone().filter(|stack| !stack.is_empty());
            let source_item = if matches!(packet.mode, SlotActionType::Swap) {
                let hotbar_stack = self.inventory().get_slot(packet.button as usize);
                if hotbar_stack.is_empty() {
                    None
                } else {
                    Some(hotbar_stack)
                }
            } else {
                cursor_item.filter(|stack| !stack.is_empty())
            };
            let mut swap_event = crate::plugin::api::events::player::player_swap_with_equipment_slot::PlayerSwapWithEquipmentSlotEvent::new(
                self.clone(),
                slot_name,
                equipped_item,
                source_item,
            );
            server.plugin_manager.fire_blocking(server, &mut swap_event);
            if swap_event.cancelled {
                cancel_screen();
                return;
            }
        }

        if slot == 0
            && let Some(ref stack) = clicked_item
            && !stack.is_empty()
        {
            let mut craft_event =
                crate::plugin::api::events::inventory::craft_item::CraftItemEvent::new(
                    self.clone(),
                    stack.item.registry_key.to_string(),
                );
            let mut prep_craft =
                crate::plugin::api::events::inventory::prepare_item_craft::PrepareItemCraftEvent::new(
                    self.clone(),
                    stack.item.registry_key.to_string(),
                );
            if let Some(server) = self.world().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut craft_event);
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut prep_craft);
            }
            if craft_event.cancelled || prep_craft.cancelled {
                cancel_screen();
                return;
            }
        }

        if window_type == Some(WindowType::Smithing)
            && slot == 3
            && let Some(ref stack) = clicked_item
            && !stack.is_empty()
        {
            let mut smith_event =
                crate::plugin::api::events::inventory::smith_item::SmithItemEvent::new(
                    self.clone(),
                    stack.item.registry_key.to_string(),
                );
            let mut prep_smith =
                crate::plugin::api::events::inventory::prepare_smithing::PrepareSmithingEvent::new(
                    self.clone(),
                    Some(stack.item.registry_key.to_string()),
                );
            if let Some(server) = self.world().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut smith_event);
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut prep_smith);
            }
            if smith_event.cancelled {
                cancel_screen();
                return;
            }
        }

        if (window_type == Some(WindowType::Furnace)
            || window_type == Some(WindowType::BlastFurnace)
            || window_type == Some(WindowType::Smoker))
            && slot == 2
            && let Some(ref stack) = clicked_item
            && !stack.is_empty()
        {
            let mut extract_event =
                crate::plugin::api::events::inventory::furnace_extract::FurnaceExtractEvent::new(
                    self.clone(),
                    papokin_util::math::position::BlockPos::new(0, 0, 0),
                    stack.item.registry_key.to_string(),
                    stack.item_count as u32,
                    0.0,
                );
            if let Some(server) = self.world().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut extract_event);
            }
        }

        if window_type == Some(WindowType::Grindstone)
            && let Some(ref stack) = clicked_item
        {
            let mut prep_grindstone =
                crate::plugin::api::events::inventory::prepare_grindstone::PrepareGrindstoneEvent::new(
                    self.clone(),
                    if stack.is_empty() { None } else { Some(stack.item.registry_key.to_string()) },
                );
            if let Some(server) = self.world().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut prep_grindstone);
            }
        }

        if let Some(ref stack) = clicked_item {
            let mut prep_result =
                crate::plugin::api::events::inventory::prepare_inventory_result::PrepareInventoryResultEvent::new(
                    self.clone(),
                    if stack.is_empty() { None } else { Some(stack.item.registry_key.to_string()) },
                );
            if let Some(server) = self.world().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut prep_result);
            }
        }

        if packet.mode == SlotActionType::QuickCraft {
            let mut drag_event =
                crate::plugin::api::events::inventory::inventory_drag::InventoryDragEvent::new(
                    self.clone(),
                );
            if let Some(server) = self.world().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut drag_event);
            }
            if drag_event.cancelled {
                cancel_screen();
                return;
            }
        }

        let screen_handler_arc = self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut screen_handler = screen_handler_arc
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // 强制标志
        let is_container_slot = slot >= 0 && i32::from(slot) < container_slots as i32;

        match packet.mode {
            SlotActionType::Pickup => {
                let cursor_stack = screen_handler
                    .get_behaviour()
                    .cursor_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if is_container_slot {
                    if !cursor_stack.is_empty() && !allow_put_items {
                        drop(cursor_stack);
                        screen_handler.cancel();
                        return;
                    }
                    if cursor_stack.is_empty() && !allow_grab_items {
                        drop(cursor_stack);
                        screen_handler.cancel();
                        return;
                    }
                }
            }
            SlotActionType::QuickMove => {
                if is_container_slot && !allow_grab_items {
                    screen_handler.cancel();
                    return;
                }
                if !is_container_slot && !allow_put_items {
                    screen_handler.cancel();
                    return;
                }
            }
            SlotActionType::Swap => {
                if is_container_slot && (!allow_grab_items || !allow_put_items) {
                    screen_handler.cancel();
                    return;
                }
            }
            SlotActionType::Throw => {
                if is_container_slot && !allow_grab_items {
                    screen_handler.cancel();
                    return;
                }
            }
            SlotActionType::QuickCraft => {
                if !allow_put_items {
                    // 将物品拖入槽位
                    screen_handler.cancel();
                    return;
                }
            }
            SlotActionType::PickupAll => {
                if !allow_grab_items {
                    screen_handler.cancel();
                    return;
                }
            }
            SlotActionType::Clone => {}
        }

        let not_in_sync = packet.revision.0
            != (screen_handler
                .get_behaviour()
                .revision
                .load(Ordering::Relaxed) as i32);

        screen_handler.disable_sync();
        screen_handler.on_slot_click(
            i32::from(slot),
            i32::from(packet.button),
            packet.mode.clone(),
            self.as_ref(),
        );

        for (key, value) in packet.array_of_changed_slots {
            screen_handler.set_received_hash(key as usize, value);
        }

        screen_handler.set_received_cursor_hash(packet.carried_item);
        screen_handler.enable_sync();

        if not_in_sync {
            screen_handler.update_to_client();
        } else {
            screen_handler.send_content_updates();
        }
    }

    /// 处理玩家点击容器（例如附魔台）中按钮的情况
    pub fn on_container_button_click(&self, packet: &SContainerButtonClick) {
        let screen_handler = self
            .current_screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut screen_handler = screen_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if i32::from(screen_handler.sync_id()) != packet.window_id.0 {
            return;
        }

        // 织布机图案 / 切石机配方选择钩子：取消即否决
        // 所选内容（按钮点击被丢弃）。修改后的织布机图案
        // 会被映射回可选图案列表中的索引。
        let mut button_id = packet.button_id.0;
        {
            let world = self.world();
            let server = world.server.upgrade();
            let player = world.get_player_by_uuid(self.gameprofile.id);
            if let (Some(server), Some(player)) = (server, player) {
                if let Some(loom) = screen_handler
                    .as_any()
                    .downcast_ref::<papokin_inventory::loom_screen_handler::LoomScreenHandler>(
                ) {
                    let pattern = (button_id >= 0).then(|| {
                        loom.selectable_patterns.get(button_id as usize).cloned()
                    });
                    if let Some(Some(pattern)) = pattern {
                        let mut loom_event = crate::plugin::api::events::player::player_loom_pattern_select::PlayerLoomPatternSelectEvent::new(
                            player,
                            pattern,
                        );
                        server
                            .plugin_manager
                            .fire_blocking(&server, &mut loom_event);
                        if loom_event.cancelled {
                            return;
                        }
                        if let Some(new_index) = loom
                            .selectable_patterns
                            .iter()
                            .position(|p| p == &loom_event.pattern)
                        {
                            button_id = new_index as i32;
                        }
                    }
                } else if let Some(stonecutter) = screen_handler
                    .as_any()
                    .downcast_ref::<papokin_inventory::stonecutter_screen_handler::StonecutterScreenHandler>()
                    && let Some(recipe_id) = stonecutter.recipe_id_for_button(button_id)
                {
                    let mut stonecutter_event = crate::plugin::api::events::player::player_stonecutter_recipe_select::PlayerStonecutterRecipeSelectEvent::new(
                        player,
                        recipe_id,
                    );
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut stonecutter_event);
                    if stonecutter_event.cancelled {
                        return;
                    }
                }
            }
        }

        screen_handler.on_button_click(self, button_id);
    }

    pub fn has_permission(self: &Arc<Self>, server: &Server, node: &str) -> bool {
        let result = server.permission_manager.has_permission(
            &self.gameprofile.id,
            node,
            self.permission_lvl.load(),
        );

        let mut event = PlayerPermissionCheckEvent::new(self.clone(), node.to_string(), result);
        let server_arc = self.world().server.upgrade();
        if let Some(server_arc) = server_arc {
            server_arc
                .plugin_manager
                .fire_blocking(&server_arc, &mut event);
        }
        event.result
    }

    pub fn is_creative(&self) -> bool {
        self.gamemode.load() == GameMode::Creative
    }

    /// 挥动玩家的手臂
    pub fn swing_hand(&self, hand: Hand, all: bool) {
        let world = self.world();
        let entity_id = self.entity_id();

        let je_packet = papokin_protocol::java::client::play::CSwingArm::new(
            VarInt(entity_id),
            hand == Hand::Left,
        );

        if all {
            world.broadcast_packet_all(&je_packet);
        } else {
            world.broadcast_packet_except(&[self.gameprofile.id], &je_packet);
        }
    }

    /// 开始使用物品（例如拉弓）
    pub fn start_using_item(&self, hand: Hand) {
        self.using_item.store(true, Ordering::Relaxed);
        self.item_use_start_time
            .store(self.tick_counter.load(Ordering::Relaxed), Ordering::Relaxed);
        self.using_hand.store(Some(hand));
    }

    /// 停止使用物品
    pub fn stop_using_item(&self) {
        self.using_item.store(false, Ordering::Relaxed);
        self.using_hand.store(None);
    }

    /// 获取物品已被使用的刻数
    pub fn get_item_use_ticks(&self) -> i32 {
        if !self.using_item.load(Ordering::Relaxed) {
            return 0;
        }
        self.tick_counter.load(Ordering::Relaxed) - self.item_use_start_time.load(Ordering::Relaxed)
    }

    /// 在物品栏中寻找箭（主手、副手或物品栏槽位）
    pub fn find_arrow(&self) -> Option<usize> {
        use papokin_data::item::Item;
        let inventory = &self.inventory;

        // 先检查副手
        let stack = inventory.get_slot(PlayerInventory::OFF_HAND_SLOT);
        if matches!(
            stack.item.id,
            id if id == Item::ARROW.id
                || id == Item::TIPPED_ARROW.id
                || id == Item::SPECTRAL_ARROW.id
        ) && stack.item_count > 0
        {
            return Some(PlayerInventory::OFF_HAND_SLOT);
        }

        // 检查快捷栏与主物品栏
        for slot in 0..PlayerInventory::MAIN_SIZE {
            let stack = inventory.get_slot(slot);
            if matches!(
                stack.item.id,
                id if id == Item::ARROW.id
                    || id == Item::TIPPED_ARROW.id
                    || id == Item::SPECTRAL_ARROW.id
            ) && stack.item_count > 0
            {
                return Some(slot);
            }
        }

        None
    }

    /// 从指定槽位消耗一支箭
    pub fn consume_arrow(&self, slot: usize) -> bool {
        let gamemode = self.gamemode.load();
        if gamemode == GameMode::Creative {
            return true; // 创造模式下不消耗
        }

        let inventory = &self.inventory;
        let mut stack = inventory.get_slot(slot);
        match stack.item_count {
            2.. => {
                stack.item_count -= 1;
                inventory.set_slot(slot, stack);
                true
            }
            1 => {
                inventory.set_slot(slot, ItemStack::EMPTY.clone());
                true
            }
            _ => false,
        }
    }

    /// 返回玩家下方的主要非空气 `BlockPos`。
    pub fn get_supporting_block_pos(&self) -> Option<BlockPos> {
        let entity = self.get_entity();
        let entity_pos = entity.pos.load();
        let aabb = entity.bounding_box.load();
        let world = self.world();

        // 在实体脚下正下方创建薄薄的包围盒
        let footprint = BoundingBox::new(
            Vector3::new(aabb.min.x, aabb.min.y - 1.0e-6, aabb.min.z),
            Vector3::new(aabb.max.x, aabb.min.y, aabb.max.z),
        );

        let min_pos = footprint.min_block_pos();
        let max_pos = footprint.max_block_pos();

        let mut closest_candidate = None;
        let mut min_dist_sq = f64::MAX;

        // 遍历候选
        for pos in BlockPos::iterate(min_pos, max_pos) {
            let (_, state) = world.get_block_and_state(&pos);

            // 只考虑有物理碰撞的方块
            if state.is_air() {
                continue;
            }

            // 计算方块中心到实体位置的距离平方
            let block_center_x = f64::from(pos.0.x) + 0.5;
            let block_center_y = f64::from(pos.0.y) + 0.5;
            let block_center_z = f64::from(pos.0.z) + 0.5;

            let dx = block_center_x - entity_pos.x;
            let dy = block_center_y - entity_pos.y;
            let dz = block_center_z - entity_pos.z;
            let dist_sq = dx * dx + dy * dy + dz * dz;

            // 选择距离最小的方块
            if dist_sq < min_dist_sq {
                min_dist_sq = dist_sq;
                closest_candidate = Some(pos);
            } else if (dist_sq - min_dist_sq).abs() < f64::EPSILON {
                // 如果距离相同，选取 y 最小，其次 z，再次 x 的方块
                if let Some(best_pos) = closest_candidate {
                    let is_smaller = pos.0.y < best_pos.0.y
                        || (pos.0.y == best_pos.0.y && pos.0.z < best_pos.0.z)
                        || (pos.0.y == best_pos.0.y
                            && pos.0.z == best_pos.0.z
                            && pos.0.x < best_pos.0.x);

                    if is_smaller {
                        closest_candidate = Some(pos);
                    }
                }
            }
        }

        // 如果找到方块，返回最近的一个
        if closest_candidate.is_some() {
            return closest_candidate;
        }

        // 若未找到候选，则回退到玩家正下方的方块
        let fallback_pos = BlockPos::new(
            entity_pos.x.floor() as i32,
            (entity_pos.y - 0.2).floor() as i32,
            entity_pos.z.floor() as i32,
        );

        let state = world.get_block_state(&fallback_pos);
        (!state.is_air()).then_some(fallback_pos)
    }

    pub fn get_command_source(self: &Arc<Self>, server: &Arc<Server>) -> CommandSource {
        CommandSender::Player(self.clone()).into_source(server)
    }

    pub fn has_advancement(
        &self,
        advancement: &'static papokin_data::advancement::Advancement,
    ) -> bool {
        self.advancements.try_lock().is_ok_and(|advancements| {
            advancements
                .progress
                .map
                .get(advancement)
                .is_some_and(crate::entity::player::advancement::AdvancementProgress::is_done)
        })
    }

    pub fn has_item_in_inventory(&self, item: &papokin_data::item::Item) -> bool {
        let main_inv = self
            .inventory
            .main_inventory
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for stack in main_inv.iter() {
            if !stack.is_empty() && stack.item.id == item.id {
                return true;
            }
        }
        let equipment = self
            .inventory
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for stack in equipment.equipment.values() {
            if !stack.is_empty() && stack.item.id == item.id {
                return true;
            }
        }
        false
    }

    pub fn trigger_advancement_criterion(
        &self,
        advancement: &'static papokin_data::advancement::Advancement,
        criterion: &str,
    ) {
        let Some((player, result)) =
            self.advancements
                .try_lock()
                .ok()
                .and_then(|mut advancements| {
                    let player = advancements.player.upgrade()?;
                    let result = advancements.award(advancement, criterion);
                    Some((player, result))
                })
        else {
            return;
        };

        PlayerAdvancement::finish_award(&player, advancement, result);
    }

    pub fn check_inventory_advancements(&self) {
        if self.inventory_changed.swap(false, Ordering::Relaxed)
            && let Some(p) = self.world().get_player_by_uuid(self.gameprofile.id)
        {
            p.trigger_advancement(
                crate::entity::player::advancement::trigger::AdvancementTrigger::InventoryChanged,
            );
        }
    }
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        self.gameprofile.id == other.gameprofile.id
    }
}

impl NBTStorage for PlayerInventory {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        // 保存选中的槽位（快捷栏）
        nbt.put_int("SelectedItemSlot", i32::from(self.get_selected_slot()));

        // 以正确的容量（物品栏大小）创建物品栏列表
        let mut items: Vec<NbtTag> = Vec::with_capacity(41);
        {
            let main_inv = self
                .main_inventory
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (i, stack) in main_inv.iter().enumerate() {
                if !stack.is_empty() {
                    let mut item_compound = NbtCompound::new();
                    item_compound.put_byte("Slot", i as i8);
                    stack.write_item_stack(&mut item_compound);
                    items.push(NbtTag::Compound(item_compound));
                }
            }
        }

        let mut equipment_compound = NbtCompound::new();
        {
            let equipment_guard = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (slot, stack) in &equipment_guard.equipment {
                if !stack.is_empty() {
                    let mut item_compound = NbtCompound::new();
                    stack.write_item_stack(&mut item_compound);
                    let vanilla_slot = match slot {
                        EquipmentSlot::Feet(_) => {
                            equipment_compound.put_compound("feet", item_compound.clone());
                            Some(100i8)
                        }
                        EquipmentSlot::Legs(_) => {
                            equipment_compound.put_compound("legs", item_compound.clone());
                            Some(101i8)
                        }
                        EquipmentSlot::Chest(_) => {
                            equipment_compound.put_compound("chest", item_compound.clone());
                            Some(102i8)
                        }
                        EquipmentSlot::Head(_) => {
                            equipment_compound.put_compound("head", item_compound.clone());
                            Some(103i8)
                        }
                        EquipmentSlot::OffHand(_) => {
                            equipment_compound.put_compound("offhand", item_compound.clone());
                            Some(-106i8)
                        }
                        _ => None,
                    };
                    if let Some(slot_byte) = vanilla_slot {
                        let mut inv_item_compound = NbtCompound::new();
                        inv_item_compound.put_byte("Slot", slot_byte);
                        stack.write_item_stack(&mut inv_item_compound);
                        items.push(NbtTag::Compound(inv_item_compound));
                    }
                }
            }
        }
        nbt.put_compound("equipment", equipment_compound);
        nbt.put("Inventory", NbtTag::List(items));
    }

    fn read_nbt_non_mut(&self, nbt: &NbtCompound) {
        // 读取选中的快捷栏槽位
        self.set_selected_slot(nbt.get_int("SelectedItemSlot").unwrap_or(0) as u8);
        // 处理物品栏列表
        let set_stack_sync = |slot: usize, stack: ItemStack| {
            if slot < Self::MAIN_SIZE {
                let mut inv = self
                    .main_inventory
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                inv[slot] = stack;
            } else if let Some(slot) = self.equipment_slots.get(&slot) {
                self.entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .put(slot, stack);
            }
        };

        if let Some(inventory_list) = nbt.get_list("Inventory") {
            for tag in inventory_list {
                if let Some(item_compound) = tag.extract_compound()
                    && let Some(slot_byte) = item_compound.get_byte("Slot")
                {
                    let slot = match slot_byte {
                        100 => 36,  // 脚部
                        101 => 37,  // 腿部
                        102 => 38,  // 箱子（chest）
                        103 => 39,  // 头部
                        -106 => 40, // 副手
                        s if (0..=40).contains(&s) => s as usize,
                        _ => continue,
                    };
                    if let Some(item_stack) = ItemStack::read_item_stack(item_compound) {
                        set_stack_sync(slot, item_stack);
                    }
                }
            }
        }

        if let Some(equipment) = nbt.get_compound("equipment") {
            if let Some(offhand) = equipment.get_compound("offhand")
                && let Some(item_stack) = ItemStack::read_item_stack(offhand)
            {
                set_stack_sync(40, item_stack);
            }

            if let Some(head) = equipment.get_compound("head")
                && let Some(item_stack) = ItemStack::read_item_stack(head)
            {
                set_stack_sync(39, item_stack);
            }

            if let Some(chest) = equipment.get_compound("chest")
                && let Some(item_stack) = ItemStack::read_item_stack(chest)
            {
                set_stack_sync(38, item_stack);
            }

            if let Some(legs) = equipment.get_compound("legs")
                && let Some(item_stack) = ItemStack::read_item_stack(legs)
            {
                set_stack_sync(37, item_stack);
            }

            if let Some(feet) = equipment.get_compound("feet")
                && let Some(item_stack) = ItemStack::read_item_stack(feet)
            {
                set_stack_sync(36, item_stack);
            }
        }
    }
}

impl NBTStorageInit for PlayerInventory {}

impl NBTStorage for EnderChestInventory {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        // 以正确的容量（物品栏大小）创建物品列表
        let mut items: Vec<NbtTag> = Vec::with_capacity(Self::INVENTORY_SIZE);
        let ec_items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (i, stack) in ec_items.iter().enumerate() {
            if !stack.is_empty() {
                let mut item_compound = NbtCompound::new();
                item_compound.put_byte("Slot", i as i8);
                stack.write_item_stack(&mut item_compound);
                items.push(NbtTag::Compound(item_compound));
            }
        }

        nbt.put("EnderItems", NbtTag::List(items));
    }

    fn read_nbt_non_mut(&self, nbt: &NbtCompound) {
        // 处理物品列表
        if let Some(item_list) = nbt.get_list("EnderItems") {
            let mut items = self
                .items
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for tag in item_list {
                if let Some(item_compound) = tag.extract_compound()
                    && let Some(slot_byte) = item_compound.get_byte("Slot")
                    && (0..Self::INVENTORY_SIZE as i8).contains(&slot_byte)
                {
                    let slot = slot_byte as usize;
                    if let Some(item_stack) = ItemStack::read_item_stack(item_compound) {
                        items[slot] = item_stack;
                    }
                }
            }
        }
    }
}

impl NBTStorageInit for EnderChestInventory {}

impl EntityBase for Player {
    fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.damage_with_context(caller, amount, damage_type, position, source, cause)
    }

    fn damage_with_resolved_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: &papokin_data::damage_ext::ResolvedDamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.damage_with_resolved_context(caller, amount, damage_type, position, source, cause)
    }

    fn teleport(
        &self,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        world: Arc<World>,
    ) {
        if Arc::ptr_eq(&world, &self.world()) {
            // 同一世界
            let yaw = yaw.unwrap_or_else(|| self.living_entity.entity.yaw.load());
            let pitch = pitch.unwrap_or_else(|| self.living_entity.entity.pitch.load());
            self.request_teleport(position, yaw, pitch);
            let entity = self.get_entity();
            let chunk_pos = entity.chunk_pos.load();
            entity.world.load().broadcast_to_chunk_except(
                chunk_pos,
                &[self.living_entity.entity.entity_uuid],
                &CEntityPositionSync::new(
                    self.living_entity.entity.entity_id.into(),
                    position,
                    Vector3::new(0.0, 0.0, 0.0),
                    yaw,
                    pitch,
                    entity.on_ground.load(Ordering::SeqCst),
                ),
            );
        } else if let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id) {
            self.spawn_task(async move {
                player_arc.teleport_world(world, position, yaw, pitch).await;
            });
        }
    }

    fn teleport_with_relatives(
        &self,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        relatives: &[PositionFlag],
        world: Arc<World>,
    ) {
        if Arc::ptr_eq(&world, &self.world()) {
            // 同一世界
            let entity = &self.living_entity.entity;
            // 缺失的偏航/俯仰保持当前朝向；相应的
            // 旋转标志此时并不适用。
            let mut relative_bits = PositionFlag::get_bitfield(relatives);
            if yaw.is_none() {
                relative_bits &=
                    !PositionFlag::get_bitfield(&[PositionFlag::YRot, PositionFlag::RotateDelta]);
            }
            if pitch.is_none() {
                relative_bits &=
                    !PositionFlag::get_bitfield(&[PositionFlag::XRot, PositionFlag::RotateDelta]);
            }
            let relatives = PositionFlag::from_bitfield(relative_bits);
            let yaw = yaw.unwrap_or_else(|| entity.yaw.load());
            let pitch = pitch.unwrap_or_else(|| entity.pitch.load());
            if let Some((abs_position, abs_yaw, abs_pitch)) =
                self.request_teleport_resolved(position, yaw, pitch, &relatives)
            {
                let chunk_pos = entity.chunk_pos.load();
                entity.world.load().broadcast_to_chunk_except(
                    chunk_pos,
                    &[entity.entity_uuid],
                    // 观察者总是接收解析后的绝对位置。
                    &CEntityPositionSync::new(
                        entity.entity_id.into(),
                        abs_position,
                        Vector3::new(0.0, 0.0, 0.0),
                        abs_yaw,
                        abs_pitch,
                        entity.on_ground.load(Ordering::SeqCst),
                    ),
                );
            }
        } else if let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id) {
            // `PositionFlag` 不是 `Clone`；向任务传入一份持有所有权的副本。
            let relatives = PositionFlag::from_bitfield(PositionFlag::get_bitfield(relatives));
            self.spawn_task(async move {
                player_arc
                    .teleport_world_with_relatives(world, position, yaw, pitch, relatives)
                    .await;
            });
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.living_entity.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        Some(&self.living_entity)
    }

    fn get_player(&self) -> Option<&Player> {
        Some(self)
    }

    fn is_spectator(&self) -> bool {
        self.gamemode.load() == GameMode::Spectator
    }

    fn set_on_fire_for_ticks(&self, ticks: u32) {
        let entity = self.get_entity();
        let ticks = if entity.invulnerable.load(Ordering::Relaxed) {
            1
        } else {
            ticks
        };
        if entity.fire_ticks.load(Ordering::Relaxed) < ticks as i32 {
            entity.fire_ticks.store(ticks as i32, Ordering::Relaxed);
        }
    }

    fn is_pushable(&self) -> bool {
        self.gamemode.load() != GameMode::Spectator && self.gamemode.load() != GameMode::Creative
    }

    fn get_name(&self) -> TextComponent {
        //TODO: 队伍颜色
        TextComponent::text(self.gameprofile.name.clone())
    }

    fn get_display_name(&self) -> TextComponent {
        if let Some(display_name) = self
            .display_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            return display_name.clone();
        }
        let name = self.get_name();
        let name_clone = name.clone();
        let mut name = name.click_event(ClickEvent::SuggestCommand {
            command: format!("/tell {} ", self.gameprofile.name.clone()).into(),
        });
        name = name.hover_event(HoverEvent::show_entity(
            self.living_entity.entity.entity_uuid.to_string(),
            self.living_entity.entity.entity_type.resource_name.into(),
            Some(name_clone),
        ));
        name.insertion(self.gameprofile.name.clone())
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("DataVersion", DATA_VERSION);
        self.inventory.write_nbt(nbt);
        self.ender_chest_inventory.write_nbt(nbt);

        self.abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write_nbt(nbt);

        let total_exp = experience::points_to_level(self.experience_level.load(Ordering::Relaxed))
            + self.experience_points.load(Ordering::Relaxed);
        nbt.put_float("XpP", self.experience_progress.load());
        nbt.put_int("XpLevel", self.experience_level.load(Ordering::Relaxed));
        nbt.put_int("XpTotal", total_exp);
        nbt.put_int("XpSeed", self.enchantment_seed.load(Ordering::Relaxed));
        nbt.put_int("Score", self.score.load(Ordering::Relaxed));
        nbt.put_short("SleepTimer", self.sleeping_since.load().unwrap_or(0) as i16);

        nbt.put_int("playerGameType", self.gamemode.load() as i32);
        if let Some(previous_gamemode) = self.previous_gamemode.load() {
            nbt.put_int("previousPlayerGameType", previous_gamemode as i32);
        }

        nbt.put_bool("seenCredits", self.seen_credits.load(Ordering::Relaxed));
        nbt.put_bool(
            "spawn_extra_particles_on_fall",
            self.spawn_extra_particles_on_fall.load(Ordering::Relaxed),
        );
        nbt.put_bool(
            "HasPlayedBefore",
            self.has_played_before.load(Ordering::Relaxed),
        );

        // 存储饥饿值、饱和度、消耗度与刻计时器
        self.hunger_manager.write_nbt(nbt);

        let air_supply = self
            .breath_manager
            .air_supply
            .load(Ordering::Relaxed)
            .clamp(0, super::breath::MAX_AIR);
        nbt.put_short("Air", air_supply as i16);
        nbt.put_int("AirSupply", air_supply);
        nbt.put_int(
            "DrowningTick",
            self.breath_manager
                .drowning_tick
                .load(Ordering::Relaxed)
                .clamp(0, super::breath::DROWNING_INTERVAL - 1),
        );

        nbt.put_string(
            "Dimension",
            self.world().dimension.minecraft_name.to_string(),
        );

        if let Some(respawn) = self
            .respawn_point
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            nbt.put_int("SpawnX", respawn.position.0.x);
            nbt.put_int("SpawnY", respawn.position.0.y);
            nbt.put_int("SpawnZ", respawn.position.0.z);
            nbt.put_string(
                "SpawnDimension",
                respawn.dimension.minecraft_name.to_owned(),
            );
            nbt.put_bool("SpawnForced", respawn.force);

            let mut respawn_compound = NbtCompound::new();
            respawn_compound.put_string("dimension", respawn.dimension.minecraft_name.to_string());
            respawn_compound.put(
                "pos",
                NbtTag::IntArray(vec![
                    respawn.position.0.x,
                    respawn.position.0.y,
                    respawn.position.0.z,
                ]),
            );
            respawn_compound.put_float("angle", respawn.yaw);
            respawn_compound.put_bool("forced", respawn.force);
            nbt.put_compound("respawn", respawn_compound);
        }

        let vehicle_uuid = self
            .living_entity
            .entity
            .vehicle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|vehicle| vehicle.get_entity().entity_uuid)
            .or_else(|| self.root_vehicle_uuid.load());
        if let Some(vehicle_uuid) = vehicle_uuid {
            write_root_vehicle(nbt, vehicle_uuid);
        }
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write_nbt(nbt);
    }

    #[expect(clippy::too_many_lines)]
    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        self.inventory.read_nbt_non_mut(nbt);
        self.ender_chest_inventory.read_nbt_non_mut(nbt);
        self.living_entity
            .apply_current_equipment_attribute_modifiers();

        let xp_p = nbt.get_float("XpP").unwrap_or(0.0);
        let xp_level = nbt.get_int("XpLevel");
        let total_exp = nbt.get_int("XpTotal").unwrap_or(0);

        if let Some(level) = xp_level {
            self.experience_level.store(level, Ordering::Relaxed);
            self.experience_progress.store(xp_p);
            let points = (xp_p * experience::points_in_level(level) as f32).round() as i32;
            self.experience_points.store(points, Ordering::Relaxed);
        } else {
            let (level, points) = experience::total_to_level_and_points(total_exp);
            let progress = experience::progress_in_level(level, points);
            self.experience_level.store(level, Ordering::Relaxed);
            self.experience_progress.store(progress);
            self.experience_points.store(points, Ordering::Relaxed);
        }

        self.enchantment_seed.store(
            nbt.get_int("XpSeed").unwrap_or_else(rand::random),
            Ordering::Relaxed,
        );

        self.score
            .store(nbt.get_int("Score").unwrap_or(0), Ordering::Relaxed);
        if let Some(sleep_timer) = nbt.get_short("SleepTimer")
            && sleep_timer > 0
        {
            self.sleeping_since.store(Some(sleep_timer as u8));
        }

        self.seen_credits.store(
            nbt.get_bool("seenCredits").unwrap_or(false),
            Ordering::Relaxed,
        );
        self.spawn_extra_particles_on_fall.store(
            nbt.get_bool("spawn_extra_particles_on_fall")
                .unwrap_or(false),
            Ordering::Relaxed,
        );

        let gamemode = nbt
            .get_int("playerGameType")
            .or_else(|| nbt.get_byte("playerGameType").map(i32::from))
            .and_then(|val| GameMode::try_from(val).ok())
            .unwrap_or_else(|| self.gamemode.load());

        self.gamemode.store(gamemode);

        self.previous_gamemode.store(
            nbt.get_int("previousPlayerGameType")
                .or_else(|| nbt.get_byte("previousPlayerGameType").map(i32::from))
                .and_then(|val| GameMode::try_from(val).ok()),
        );

        {
            let mut abilities = self
                .abilities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            abilities.set_for_gamemode(gamemode);
            abilities.read_nbt(nbt);
            if gamemode == GameMode::Creative {
                abilities.allow_flying = true;
                abilities.creative = true;
                abilities.invulnerable = true;
            } else if gamemode == GameMode::Spectator {
                abilities.allow_flying = true;
                abilities.creative = false;
                abilities.invulnerable = true;
            }
        }

        self.living_entity.entity.invulnerable.store(
            matches!(gamemode, GameMode::Creative | GameMode::Spectator),
            Ordering::Relaxed,
        );
        self.living_entity
            .entity
            .no_physics
            .store(gamemode == GameMode::Spectator, Ordering::Relaxed);
        if gamemode == GameMode::Spectator {
            self.living_entity
                .entity
                .on_ground
                .store(false, Ordering::Relaxed);
        }

        self.has_played_before.store(
            nbt.get_bool("HasPlayedBefore").unwrap_or(false),
            Ordering::Relaxed,
        );

        self.hunger_manager.read_nbt_non_mut(nbt);

        if let Some(air) = nbt
            .get_short("Air")
            .map(i32::from)
            .or_else(|| nbt.get_int("AirSupply"))
        {
            self.breath_manager
                .air_supply
                .store(air.clamp(0, super::breath::MAX_AIR), Ordering::Relaxed);
        }
        if let Some(tick) = nbt.get_int("DrowningTick") {
            self.breath_manager.drowning_tick.store(
                tick.clamp(0, super::breath::DROWNING_INTERVAL - 1),
                Ordering::Relaxed,
            );
        }

        // 加载所有已保存的重生点数据（包括原版 "respawn" 复合标签和旧版 SpawnX/SpawnY/SpawnZ）
        if let Some(respawn_compound) = nbt.get_compound("respawn") {
            let dim = respawn_compound
                .get_string("dimension")
                .and_then(|s| Dimension::from_name(s).cloned())
                .unwrap_or_else(|| self.world().dimension.clone());
            let pos = if let Some(pos_array) = respawn_compound.get_int_array("pos")
                && pos_array.len() >= 3
            {
                BlockPos(Vector3::new(pos_array[0], pos_array[1], pos_array[2]))
            } else {
                BlockPos(Vector3::new(0, 0, 0))
            };
            let yaw = respawn_compound.get_float("angle").unwrap_or(0.0);
            let force = respawn_compound.get_bool("forced").unwrap_or(false);
            *self
                .respawn_point
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(RespawnPoint {
                dimension: dim,
                position: pos,
                yaw,
                force,
            });
        } else if let (Some(x), Some(y), Some(z)) = (
            nbt.get_int("SpawnX"),
            nbt.get_int("SpawnY"),
            nbt.get_int("SpawnZ"),
        ) {
            let dim = nbt
                .get_string("SpawnDimension")
                .and_then(|s| Dimension::from_name(s).cloned())
                .unwrap_or_else(|| self.world().dimension.clone());
            let force = nbt.get_bool("SpawnForced").unwrap_or(false);
            *self
                .respawn_point
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(RespawnPoint {
                dimension: dim,
                position: BlockPos(Vector3::new(x, y, z)),
                yaw: 0.0,
                force,
            });
        }
        self.root_vehicle_uuid.store(read_root_vehicle(nbt));
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .read_nbt(nbt);
    }

    fn get_experience_reward(&self, _killer: Option<&dyn EntityBase>) -> u32 {
        // 原版：min(level * 7, 100)
        let level = self.experience_level.load(Ordering::Relaxed);
        (level * 7).min(100) as u32
    }

    fn tick_in_void(&self, dyn_self: &dyn EntityBase) {
        self.living_entity.tick_in_void(dyn_self);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TitleMode {
    Title,
    SubTitle,
    ActionBar,
}

/// 表示玩家的能力与特殊力量。
///
/// 此结构体包含玩家当前能力的相关信息，例如飞行、无敌和创造模式。
#[derive(Clone, Copy, Debug)]
pub struct Abilities {
    /// 表示玩家是否对伤害免疫。
    pub invulnerable: bool,
    /// 表示玩家当前是否正在飞行。
    pub flying: bool,
    /// 表示玩家是否被允许飞行（如果已启用）。
    pub allow_flying: bool,
    /// 表示玩家是否处于创造模式。
    pub creative: bool,
    /// 表示玩家是否被允许修改世界。
    pub allow_modify_world: bool,
    /// 玩家的飞行速度。
    pub fly_speed: f32,
    /// 玩家行走或疾跑时的视野调整。
    pub walk_speed: f32,
}

impl NBTStorage for Abilities {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        let mut component = NbtCompound::new();
        component.put_bool("invulnerable", self.invulnerable);
        component.put_bool("flying", self.flying);
        component.put_bool("mayfly", self.allow_flying);
        component.put_bool("instabuild", self.creative);
        component.put_bool("mayBuild", self.allow_modify_world);
        component.put_float("flySpeed", self.fly_speed);
        component.put_float("walkSpeed", self.walk_speed);
        nbt.put_compound("abilities", component);
    }

    fn read_nbt(&mut self, nbt: &mut NbtCompound) {
        Self::read_nbt(self, nbt);
    }
}

impl NBTStorageInit for Abilities {}

impl Default for Abilities {
    fn default() -> Self {
        Self {
            invulnerable: false,
            flying: false,
            allow_flying: false,
            creative: false,
            allow_modify_world: true,
            fly_speed: 0.05,
            walk_speed: 0.1,
        }
    }
}

impl Abilities {
    pub fn read_nbt(&mut self, nbt: &NbtCompound) {
        if let Some(component) = nbt.get_compound("abilities") {
            self.invulnerable = component.get_bool("invulnerable").unwrap_or(false);
            self.flying = component.get_bool("flying").unwrap_or(false);
            self.allow_flying = component.get_bool("mayfly").unwrap_or(false);
            self.creative = component.get_bool("instabuild").unwrap_or(false);
            self.allow_modify_world = component.get_bool("mayBuild").unwrap_or(true);
            // 速度系数拒绝 NaN/Inf 与负数，防止经移动算术把 NaN
            // 传播进玩家坐标。
            self.fly_speed =
                finite_non_negative_f32_or(component.get_float("flySpeed").unwrap_or(0.05), 0.05);
            self.walk_speed =
                finite_non_negative_f32_or(component.get_float("walkSpeed").unwrap_or(0.1), 0.1);
        }
    }

    pub const fn set_for_gamemode(&mut self, gamemode: GameMode) {
        match gamemode {
            GameMode::Creative => {
                // self.flying = false; // Start not flying
                self.allow_flying = true;
                self.creative = true;
                self.invulnerable = true;
                self.allow_modify_world = true;
            }
            GameMode::Spectator => {
                self.flying = true;
                self.allow_flying = true;
                self.creative = false;
                self.invulnerable = true;
                self.allow_modify_world = false;
            }
            GameMode::Adventure => {
                self.flying = false;
                self.allow_flying = false;
                self.creative = false;
                self.invulnerable = false;
                self.allow_modify_world = false;
            }
            GameMode::Survival => {
                self.flying = false;
                self.allow_flying = false;
                self.creative = false;
                self.invulnerable = false;
                self.allow_modify_world = true;
            }
        }
    }
}

/// 表示玩家存储的重生点（床/重生锚/强制）。
#[derive(Debug, Clone, PartialEq)]
pub struct RespawnPoint {
    pub dimension: Dimension,
    pub position: BlockPos,
    pub yaw: f32,
    pub force: bool,
}

pub struct CalculatedRespawnPoint {
    /// 生成所在的精确位置（以方块中心为准）。
    pub position: Vector3<f64>,
    /// 偏航角旋转。
    pub yaw: f32,
    /// 俯仰角旋转。
    pub pitch: f32,
    /// 生成所在的维度。
    pub dimension: Dimension,
}

/// 表示玩家的聊天模式设置。
#[derive(Debug, Clone)]
pub enum ChatMode {
    /// 该玩家已启用聊天。
    Enabled,
    /// 玩家应只能看到来自命令的聊天消息。
    CommandsOnly,
    /// 所有消息都应隐藏。
    Hidden,
}

pub struct InvalidChatMode;

impl TryFrom<i32> for ChatMode {
    type Error = InvalidChatMode;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Enabled),
            1 => Ok(Self::CommandsOnly),
            2 => Ok(Self::Hidden),
            _ => Err(InvalidChatMode),
        }
    }
}

/// 玩家当前的聊天会话
pub struct ChatSession {
    pub session_id: uuid::Uuid,
    pub expires_at: i64,
    pub public_key: Box<[u8]>,
    pub signature: Box<[u8]>,
    pub messages_sent: i32,
    pub messages_received: i32,
    pub signature_cache: Vec<Box<[u8]>>,
}

impl Default for ChatSession {
    fn default() -> Self {
        Self::new(Uuid::nil(), 0, Box::new([]), Box::new([]))
    }
}

impl ChatSession {
    #[must_use]
    pub const fn new(
        session_id: Uuid,
        expires_at: i64,
        public_key: Box<[u8]>,
        key_signature: Box<[u8]>,
    ) -> Self {
        Self {
            session_id,
            expires_at,
            public_key,
            signature: key_signature,
            messages_sent: 0,
            messages_received: 0,
            signature_cache: Vec::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct LastSeen(Vec<Box<[u8]>>);

impl From<LastSeen> for Vec<Box<[u8]>> {
    fn from(seen: LastSeen) -> Self {
        seen.0
    }
}

impl From<Vec<Box<[u8]>>> for LastSeen {
    fn from(entries: Vec<Box<[u8]>>) -> Self {
        Self(entries)
    }
}

impl AsRef<[Box<[u8]>]> for LastSeen {
    fn as_ref(&self) -> &[Box<[u8]>] {
        &self.0
    }
}

impl LastSeen {
    /// 如果接收者的缓存中存有发送者的 `last_seen` 签名，则会以 ID 的形式发送。
    /// 否则发送完整签名。（ID:0 表示正在发送完整签名）
    pub fn indexed_for(&self, recipient: &Arc<Player>) -> Box<[PreviousMessage]> {
        let mut indexed = Vec::new();
        let cache = recipient
            .signature_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for signature in &self.0 {
            let index = cache.full_cache.iter().position(|s| s == signature);
            if let Some(index) = index {
                indexed.push(PreviousMessage {
                    // 将 ID 引用发送到接收者的缓存（索引 + 1，因为 0 保留给完整签名）
                    id: VarInt(1 + index as i32),
                    signature: None,
                });
            } else {
                indexed.push(PreviousMessage {
                    // 完整签名时将 ID 发送为 0
                    id: VarInt(0),
                    signature: Some(signature.clone()),
                });
            }
        }
        indexed.into_boxed_slice()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastSeenTrackedEntry {
    pub signature: Box<[u8]>,
    pub pending: bool,
}

impl LastSeenTrackedEntry {
    #[must_use]
    pub fn acknowledge(&self) -> Self {
        Self {
            signature: self.signature.clone(),
            pending: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LastSeenMessagesValidator {
    pub last_seen_count: usize,
    pub tracked_messages: VecDeque<Option<LastSeenTrackedEntry>>,
    pub last_pending_message: Option<Box<[u8]>>,
}

impl Default for LastSeenMessagesValidator {
    fn default() -> Self {
        Self::new(MAX_PREVIOUS_MESSAGES as usize)
    }
}

impl LastSeenMessagesValidator {
    #[must_use]
    pub fn new(last_seen_count: usize) -> Self {
        let mut tracked = VecDeque::with_capacity(last_seen_count);
        for _ in 0..last_seen_count {
            tracked.push_back(None);
        }
        Self {
            last_seen_count,
            tracked_messages: tracked,
            last_pending_message: None,
        }
    }

    pub fn add_pending(&mut self, signature: &[u8]) {
        if self.last_pending_message.as_deref() != Some(signature) {
            let sig_box: Box<[u8]> = signature.into();
            self.tracked_messages.push_back(Some(LastSeenTrackedEntry {
                signature: sig_box.clone(),
                pending: true,
            }));
            self.last_pending_message = Some(sig_box);
        }
    }

    #[must_use]
    pub fn tracked_messages_count(&self) -> usize {
        self.tracked_messages.len()
    }

    pub fn apply_offset(&mut self, offset: usize) -> Result<(), &'static str> {
        let max_offset = self
            .tracked_messages
            .len()
            .saturating_sub(self.last_seen_count);
        if offset <= max_offset {
            self.tracked_messages.drain(0..offset);
            Ok(())
        } else {
            Err("Advanced last seen window by more messages than expected")
        }
    }

    pub fn apply_update(
        &mut self,
        offset: usize,
        acknowledged: &[u8],
    ) -> Result<Vec<Box<[u8]>>, &'static str> {
        self.apply_offset(offset)?;
        let mut last_seen_entries = Vec::new();

        for i in 0..self.last_seen_count {
            let is_acknowledged = if i / 8 < acknowledged.len() {
                (acknowledged[i / 8] & (1 << (i % 8))) != 0
            } else {
                false
            };

            let message = self.tracked_messages.get(i).cloned().flatten();
            if is_acknowledged {
                let Some(entry) = message else {
                    return Err(
                        "Last seen update acknowledged unknown or previously ignored message",
                    );
                };
                self.tracked_messages[i] = Some(entry.acknowledge());
                last_seen_entries.push(entry.signature);
            } else {
                if let Some(entry) = message
                    && !entry.pending
                {
                    return Err("Last seen update ignored previously acknowledged message");
                }
                self.tracked_messages[i] = None;
            }
        }

        Ok(last_seen_entries)
    }
}

pub struct MessageCache {
    /// 最多缓存 128 个消息签名。最新的排在最前。
    /// 服务器应（在可能时）引用此（接收方的）缓存中的索引，而不是在 last seen 中发送完整签名。
    /// 必须与客户端的签名缓存一一对应。
    full_cache: VecDeque<Box<[u8]>>,
    /// 每个发送者最多 20 条最近见过的消息。最新的排在最后
    pub last_seen: LastSeen,
    pub last_seen_validator: LastSeenMessagesValidator,
}

impl Default for MessageCache {
    fn default() -> Self {
        Self {
            full_cache: VecDeque::with_capacity(MAX_CACHED_SIGNATURES as usize),
            last_seen: LastSeen::default(),
            last_seen_validator: LastSeenMessagesValidator::default(),
        }
    }
}

impl MessageCache {
    /// 不用于缓存已见消息。仅用于发送者的未索引签名。
    pub fn cache_signatures(&mut self, signatures: &[Box<[u8]>]) {
        for sig in signatures.iter().rev() {
            if self.full_cache.contains(sig) {
                continue;
            }
            // 如果缓存已满，且有人发送比缓存中最旧签名更旧的签名，则忽略它
            if self.full_cache.len() < MAX_CACHED_SIGNATURES as usize {
                self.full_cache.push_back(sig.clone()); // 接收者从未见过此消息，因此它一定比缓存中最旧的还要早
            }
        }
    }

    /// 向 `last_seen` 和 `full_cache` 添加一条已见签名。
    pub fn add_seen_signature(&mut self, signature: &[u8]) {
        if self.last_seen.0.len() >= MAX_PREVIOUS_MESSAGES as usize {
            self.last_seen.0.remove(0);
        }
        self.last_seen.0.push(signature.into());
        // 这里可能并不需要循环，但宁可稳妥不可后悔
        while self.full_cache.len() >= MAX_CACHED_SIGNATURES as usize {
            self.full_cache.pop_back();
        }
        self.full_cache.push_front(signature.into()); // 由于接收者已看到此消息，它将是缓存中最新的一条
    }
}

impl InventoryPlayer for Player {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn drop_item(&self, item: ItemStack, _retain_ownership: bool) {
        self.drop_item(item);
    }

    fn has_infinite_materials(&self) -> bool {
        self.gamemode.load() == GameMode::Creative
    }

    fn is_creative(&self) -> bool {
        self.gamemode.load() == GameMode::Creative
    }

    fn is_spectator(&self) -> bool {
        self.gamemode.load() == GameMode::Spectator
    }

    fn experience_level(&self) -> i32 {
        self.experience_level
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn add_experience_levels(&self, levels: i32) {
        self.add_experience_levels(levels);
    }

    fn enchantment_seed(&self) -> i32 {
        self.enchantment_seed.load(Ordering::Relaxed)
    }

    fn set_enchantment_seed(&self, seed: i32) {
        self.enchantment_seed.store(seed, Ordering::Relaxed);
    }

    fn get_inventory(&self) -> Arc<PlayerInventory> {
        self.inventory.clone()
    }

    fn enqueue_inventory_packet(
        &self,
        packet: &CSetContainerContent,
        _window_type: Option<WindowType>,
    ) {
        self.client.try_send_packet(packet);
    }

    fn enqueue_slot_packet(
        &self,
        packet: &CSetContainerSlot,
        _window_type: Option<WindowType>,
        _total_slots: usize,
    ) {
        self.client.try_send_packet(packet);
    }

    fn enqueue_cursor_packet(&self, packet: &CSetCursorItem) {
        self.client.try_send_packet(packet);
    }

    fn enqueue_property_packet(&self, packet: &CSetContainerProperty) {
        self.try_send_client_packet(packet);
    }

    fn enqueue_slot_set_packet(&self, packet: &CSetPlayerInventory) {
        self.try_send_slot_set_packet(packet);
        self.sync_inventory_to_client();
    }

    fn enqueue_set_held_item_packet(&self, packet: &CSetSelectedSlot) {
        self.client.try_send_packet(packet);
    }

    fn enqueue_equipment_change(&self, slot: &EquipmentSlot, stack: &ItemStack) {
        self.living_entity
            .send_equipment_changes(&[(slot.clone(), stack.clone())]);

        // 盔甲变更钩子：仅限人形生物盔甲槽。纯通知；
        // 此刻无法观察到先前的内容，因此 `old_item`
        // 始终为 `None`。
        if slot.slot_type() == papokin_data::data_component_impl::EquipmentType::HumanoidArmor {
            let world = self.world();
            if let Some(server) = world.server.upgrade()
                && server.plugin_manager.has_handlers::<crate::plugin::api::events::player::player_armor_change::PlayerArmorChangeEvent>()
                && let Some(player) = world.get_player_by_uuid(self.gameprofile.id)
            {
                let new_item = if stack.is_empty() {
                    None
                } else {
                    Some(stack.clone())
                };
                let mut armor_event = crate::plugin::api::events::player::player_armor_change::PlayerArmorChangeEvent::new(
                    player,
                    slot.to_name().to_string(),
                    None,
                    new_item,
                );
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut armor_event);
            }
        }

        if let Some(equippable) = stack.get_data_component::<EquippableImpl>() {
            self.world().play_sound_event(
                &equippable.equip_sound,
                SoundCategory::Players,
                &self.position(),
            );
        }
    }

    fn award_experience(&self, amount: i32) {
        debug!("Player::award_experience 被调用，amount={amount}");
        if amount > 0 {
            debug!("玩家：正在增加 {amount} 点经验值");
            let player = self.world().get_player_by_uuid(self.gameprofile.id);
            if let Some(player) = player {
                player.add_experience_points(amount);
            }
        }
    }

    fn increment_stat(&self, category: StatisticCategory, stat_id: i32, amount: i32) {
        self.increment_stat(category, stat_id, amount);
    }

    fn play_block_sound(&self, sound: Sound, pitch: f32) {
        if let Some(pos) = self.open_container_pos.load() {
            self.world().play_sound_fine(
                sound,
                SoundCategory::Blocks,
                &pos.to_centered_f64(),
                1.0,
                pitch,
            );
        }
    }

    fn fire_prepare_item_enchant_event(
        &self,
        item: &ItemStack,
        level_requirements: &mut [i32; 3],
        enchantment_id: &mut [i32; 3],
        enchantment_level: &mut [i32; 3],
        bookshelf_count: i32,
    ) -> bool {
        let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id) else {
            return false;
        };
        let Some(server) = self.world().server.upgrade() else {
            return false;
        };
        let mut event = PrepareItemEnchantEvent::new(
            player_arc,
            item.clone(),
            *level_requirements,
            *enchantment_id,
            *enchantment_level,
            bookshelf_count,
        );
        server.plugin_manager.fire_blocking(&server, &mut event);
        if event.cancelled {
            return true;
        }
        *level_requirements = event.level_requirements;
        *enchantment_id = event.enchantment_id;
        *enchantment_level = event.enchantment_level;
        false
    }

    fn fire_enchant_item_event(
        &self,
        item: &ItemStack,
        option: i32,
        exp_level_cost: i32,
        enchantments_to_add: &mut Vec<(&'static papokin_data::Enchantment, i32)>,
    ) -> bool {
        let Some(player_arc) = self.world().get_player_by_uuid(self.gameprofile.id) else {
            return false;
        };
        let Some(server) = self.world().server.upgrade() else {
            return false;
        };
        let mut event = EnchantItemEvent::new(
            player_arc,
            item.clone(),
            option,
            exp_level_cost,
            enchantments_to_add.clone(),
        );
        server.plugin_manager.fire_blocking(&server, &mut event);
        if event.cancelled {
            return true;
        }
        *enchantments_to_add = event.enchantments_to_add;
        false
    }

    fn close_screen_handler(&self) {
        self.close_handled_screen();
    }

    fn use_anvil(&self) {
        if let Some(pos) = self.open_container_pos.load() {
            let world = self.world();
            let state = world.get_block_state(&pos);
            let block = papokin_data::Block::from_state_id(state.id);
            if block.has_tag(&papokin_data::tag::Block::MINECRAFT_ANVIL) {
                if !self.has_infinite_materials() && rand::random::<f32>() < 0.12 {
                    if let Some(new_state) =
                        crate::block::blocks::anvil::AnvilBlock::damage(state.id)
                    {
                        world.set_block_state(
                            &pos,
                            new_state,
                            papokin_world::world::BlockFlags::NOTIFY_ALL,
                        );
                        world.sync_world_event(
                            papokin_data::world::WorldEvent::SoundAnvilUsed,
                            pos,
                            0,
                        );
                    } else {
                        world.set_block_state(
                            &pos,
                            papokin_data::BlockStateId::AIR,
                            papokin_world::world::BlockFlags::NOTIFY_ALL,
                        );
                        world.sync_world_event(
                            papokin_data::world::WorldEvent::SoundAnvilBroken,
                            pos,
                            0,
                        );
                    }
                } else {
                    world.sync_world_event(papokin_data::world::WorldEvent::SoundAnvilUsed, pos, 0);
                }
            } else {
                world.sync_world_event(papokin_data::world::WorldEvent::SoundAnvilUsed, pos, 0);
            }
        }
    }

    fn use_grindstone(&self, xp_amount: i32) {
        if let Some(pos) = self.open_container_pos.load() {
            let world = self.world();
            if xp_amount > 0 {
                crate::entity::experience_orb::ExperienceOrbEntity::spawn(
                    &world,
                    pos.to_centered_f64(),
                    xp_amount as u32,
                );
            }
            world.sync_world_event(papokin_data::world::WorldEvent::SoundGrindstoneUsed, pos, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{read_root_vehicle, write_root_vehicle};
    use papokin_nbt::{compound::NbtCompound, tag::NbtTag};
    use uuid::Uuid;

    #[test]
    fn root_vehicle_uuid_round_trips_with_vanilla_shape() {
        let expected = Uuid::from_u128(0xFEDC_BA98_7654_3210_89AB_CDEF_0123_4567);
        let mut nbt = NbtCompound::new();

        write_root_vehicle(&mut nbt, expected);

        assert_eq!(read_root_vehicle(&nbt), Some(expected));
        assert!(
            nbt.get_compound("RootVehicle")
                .and_then(|root| root.get_int_array("Attach"))
                .is_some()
        );
    }

    #[test]
    fn root_vehicle_uuid_accepts_integer_lists() {
        let expected = Uuid::from_u128(0xFEDC_BA98_7654_3210_89AB_CDEF_0123_4567);
        let value = expected.as_u128();
        let mut root_vehicle = NbtCompound::new();
        root_vehicle.put(
            "Attach",
            NbtTag::List(vec![
                NbtTag::Int((value >> 96) as i32),
                NbtTag::Int((value >> 64) as i32),
                NbtTag::Int((value >> 32) as i32),
                NbtTag::Int(value as i32),
            ]),
        );
        let mut nbt = NbtCompound::new();
        nbt.put("RootVehicle", NbtTag::Compound(root_vehicle));

        assert_eq!(read_root_vehicle(&nbt), Some(expected));
    }

    #[test]
    fn anti_spam_counter_decay_and_threshold() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let counter = AtomicU32::new(0);
        let message_cost = 20u32;
        let spam_threshold = 200u32;
        let decay_per_tick = 1u32;

        // 10 条消息 -> count = 200（在阈值内）
        for _ in 0..10 {
            counter.fetch_add(message_cost, Ordering::SeqCst);
        }
        assert_eq!(counter.load(Ordering::SeqCst), 200);
        assert!(counter.load(Ordering::SeqCst) <= spam_threshold);

        // 第 11 条消息 -> count = 220（超出阈值）
        counter.fetch_add(message_cost, Ordering::SeqCst);
        assert_eq!(counter.load(Ordering::SeqCst), 220);
        assert!(counter.load(Ordering::SeqCst) > spam_threshold);

        // 模拟 25 刻的衰减
        for _ in 0..25 {
            let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                Some(count.saturating_sub(decay_per_tick))
            });
        }
        assert_eq!(counter.load(Ordering::SeqCst), 195);
        assert!(counter.load(Ordering::SeqCst) <= spam_threshold);

        // 模拟衰减降到 0 以下（在 0 处饱和）
        for _ in 0..250 {
            let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                Some(count.saturating_sub(decay_per_tick))
            });
        }
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
