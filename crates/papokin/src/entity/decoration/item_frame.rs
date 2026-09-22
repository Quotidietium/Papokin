use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use crate::entity::player::Player;
use crate::entity::{Entity, EntityBase, living::LivingEntity};
use crossbeam::atomic::AtomicCell;
use papokin_data::BlockDirection;
use papokin_data::damage::DamageType;
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::Sound;
use papokin_nbt::compound::NbtCompound;
use papokin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use papokin_protocol::java::client::play::{CSetEntityMetadata, Metadata};
use papokin_util::math::vector3::Vector3;
use papokin_util::version::JavaMinecraftVersion;

/// 物品展示框或荧光物品展示框。
///
/// 保存展示的物品及其旋转角度，以便比较器读取
/// 帧的模拟输出，因此来自原版世界的展示框会保留其数据
/// 跨保存周期存在。
pub struct ItemFrameEntity {
    entity: Entity,
    item_stack: Mutex<ItemStack>,
    /// 展示物品的旋转，取值始终在 `0..8` 内。
    rotation: AtomicU8,
    /// 展示框朝向的方向，即背离
    /// 其所依附的方块。以原版 3D 方向索引存储
    /// (0 = 下, 1 = 上, 2 = 北, 3 = 南, 4 = 西, 5 = 东)。
    facing: AtomicU8,
    item_drop_chance: AtomicCell<f32>,
    invisible: AtomicBool,
    fixed: AtomicBool,
}

impl ItemFrameEntity {
    /// 未带 NBT 创建物品展示框时使用的朝向，与原版一致。
    const DEFAULT_FACING: BlockDirection = BlockDirection::South;

    pub fn new(entity: Entity) -> Self {
        let facing = Self::DEFAULT_FACING.to_index();
        // 生成数据包从实体数据字段中读取方向，因此
        // 它必须与 `facing` 一致，否则框架生成时会朝向其他方向。
        entity.data.store(i32::from(facing), Ordering::Relaxed);
        Self {
            entity,
            item_stack: Mutex::new(ItemStack::EMPTY.clone()),
            rotation: AtomicU8::new(0),
            facing: AtomicU8::new(facing),
            item_drop_chance: AtomicCell::new(1.0),
            invisible: AtomicBool::new(false),
            fixed: AtomicBool::new(false),
        }
    }

    pub const fn is_glow(&self) -> bool {
        self.entity.entity_type.id == EntityType::GLOW_ITEM_FRAME.id
    }

    pub const fn get_add_item_sound(&self) -> Sound {
        if self.is_glow() {
            Sound::EntityGlowItemFrameAddItem
        } else {
            Sound::EntityItemFrameAddItem
        }
    }

    pub const fn get_remove_item_sound(&self) -> Sound {
        if self.is_glow() {
            Sound::EntityGlowItemFrameRemoveItem
        } else {
            Sound::EntityItemFrameRemoveItem
        }
    }

    pub const fn get_rotate_item_sound(&self) -> Sound {
        if self.is_glow() {
            Sound::EntityGlowItemFrameRotateItem
        } else {
            Sound::EntityItemFrameRotateItem
        }
    }

    pub const fn get_break_sound(&self) -> Sound {
        if self.is_glow() {
            Sound::EntityGlowItemFrameBreak
        } else {
            Sound::EntityItemFrameBreak
        }
    }

    pub const fn get_place_sound(&self) -> Sound {
        if self.is_glow() {
            Sound::EntityGlowItemFramePlace
        } else {
            Sound::EntityItemFramePlace
        }
    }

    pub fn get_facing(&self) -> BlockDirection {
        BlockDirection::from_index(self.facing.load(Ordering::Relaxed))
            .unwrap_or(Self::DEFAULT_FACING)
    }

    pub fn set_facing(&self, facing: BlockDirection) {
        let index = facing.to_index();
        self.facing.store(index, Ordering::Relaxed);
        self.entity.data.store(i32::from(index), Ordering::Relaxed);
    }

    pub fn get_item(&self) -> ItemStack {
        self.item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_item(&self, mut item_stack: ItemStack, update_neighbours: bool) {
        if !item_stack.is_empty() {
            item_stack.item_count = 1;
        }

        let play_sound = !item_stack.is_empty();
        let item_serializer = ItemStackSerializer::from(item_stack.clone());
        *self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = item_stack;

        self.entity.set_synced_data(
            papokin_data::tracked_data::item_frame::ITEM,
            item_serializer,
        );

        if play_sound {
            self.entity.play_sound(self.get_add_item_sound());
        }

        if update_neighbours {
            let world = self.entity.world.load();
            let pos = self.entity.block_pos.load();
            world.update_neighbors(&pos, None);
        }
    }

    pub fn get_rotation(&self) -> u8 {
        self.rotation.load(Ordering::Relaxed) % 8
    }

    pub fn set_rotation(&self, rotation: u8, update_neighbours: bool) {
        let rot = rotation % 8;
        self.rotation.store(rot, Ordering::Relaxed);

        self.entity
            .set_synced_data(papokin_data::tracked_data::item_frame::ROTATION, rot as i32);

        if update_neighbours {
            let world = self.entity.world.load();
            let pos = self.entity.block_pos.load();
            world.update_neighbors(&pos, None);
        }
    }

    pub fn get_drop_chance(&self) -> f32 {
        self.item_drop_chance.load()
    }

    pub fn set_drop_chance(&self, chance: f32) {
        self.item_drop_chance.store(chance);
    }

    pub fn is_fixed(&self) -> bool {
        self.fixed.load(Ordering::Relaxed)
    }

    pub fn set_fixed(&self, fixed: bool) {
        self.fixed.store(fixed, Ordering::Relaxed);
    }

    pub fn is_invisible(&self) -> bool {
        self.invisible.load(Ordering::Relaxed)
    }

    pub fn set_invisible(&self, invisible: bool) {
        self.invisible.store(invisible, Ordering::Relaxed);
    }

    pub fn get_frame_item_stack(&self) -> ItemStack {
        if self.is_glow() {
            ItemStack::new(1, &Item::GLOW_ITEM_FRAME)
        } else {
            ItemStack::new(1, &Item::ITEM_FRAME)
        }
    }

    pub fn get_frame_item_stack_with_data(&self) -> ItemStack {
        let mut stack = self.get_frame_item_stack();
        if let Some(custom_name) = self.entity.custom_name.load().as_ref().clone() {
            stack.set_custom_name(custom_name.to_pretty_console());
        }
        stack
    }

    pub fn get_pick_result(&self) -> ItemStack {
        let framed_stack = self.get_item();
        if framed_stack.is_empty() {
            self.get_frame_item_stack_with_data()
        } else {
            framed_stack
        }
    }

    /// 该物品展示框产生的比较器信号。
    ///
    /// 原版：`getItem().isEmpty() ? 0 : getRotation() % 8 + 1`。
    pub fn get_analog_output(&self) -> u8 {
        if self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
        {
            0
        } else {
            self.rotation.load(Ordering::Relaxed) % 8 + 1
        }
    }

    pub fn drop_item(&self, caused_by: Option<&dyn EntityBase>, with_frame: bool) {
        if self.is_fixed() {
            return;
        }

        let item_stack = self.get_item();
        *self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ItemStack::EMPTY.clone();
        let item_serializer = ItemStackSerializer::from(ItemStack::EMPTY.clone());
        self.entity.set_synced_data(
            papokin_data::tracked_data::item_frame::ITEM,
            item_serializer,
        );

        let is_creative_player = caused_by.is_some_and(|s| {
            s.cast_any()
                .downcast_ref::<Player>()
                .is_some_and(Player::is_creative)
        });

        if is_creative_player {
            return;
        }

        let world = self.entity.world.load();
        let pos = self.entity.block_pos.load();

        if with_frame {
            world.drop_stack(&pos, self.get_frame_item_stack_with_data());
        }

        if !item_stack.is_empty() {
            let drop_chance = self.item_drop_chance.load();
            if rand::random::<f32>() < drop_chance {
                world.drop_stack(&pos, item_stack);
            }
        }
    }

    /// 触发 `HangingBreakEvent`（当移除者是实体时还会触发 `HangingBreakByEntityEvent`）
    /// 移除者已知）并返回该画框是否可能破碎。
    fn fire_break_events(&self, caused_by: Option<&dyn EntityBase>) -> bool {
        let world = self.entity.world.load();
        let Some(entity_arc) = world.get_entity_by_id(self.entity.entity_id) else {
            return true;
        };
        let remover = caused_by.and_then(|c| world.get_entity_by_id(c.get_entity().entity_id));

        let mut event = crate::plugin::api::events::hanging::hanging_break::HangingBreakEvent::new(
            entity_arc.clone(),
            remover.clone(),
        );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return false;
        }

        if let Some(remover) = remover {
            let mut by_entity_event = crate::plugin::api::events::hanging::hanging_break_by_entity::HangingBreakByEntityEvent::new(
                entity_arc, remover,
            );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut by_entity_event);
            }
            if by_entity_event.cancelled {
                return false;
            }
        }
        true
    }
}

impl EntityBase for ItemFrameEntity {
    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        let item = self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !item.is_empty() {
            let mut item_compound = NbtCompound::new();
            item.write_item_stack(&mut item_compound);
            nbt.put_compound("Item", item_compound);
        }
        nbt.put_float("ItemDropChance", self.item_drop_chance.load());
        nbt.put_byte("ItemRotation", self.rotation.load(Ordering::Relaxed) as i8);
        nbt.put_byte("Facing", self.facing.load(Ordering::Relaxed) as i8);
        nbt.put_bool("Invisible", self.invisible.load(Ordering::Relaxed));
        nbt.put_bool("Fixed", self.fixed.load(Ordering::Relaxed));
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        if let Some(item_compound) = nbt.get_compound("Item")
            && let Some(stack) = ItemStack::read_item_stack(item_compound)
        {
            *self
                .item_stack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = stack;
        }
        self.rotation.store(
            (nbt.get_byte("ItemRotation").unwrap_or(0) as u8) % 8,
            Ordering::Relaxed,
        );
        let facing = nbt.get_byte("Facing").unwrap_or(0) as u8 % 6;
        self.facing.store(facing, Ordering::Relaxed);
        // 生成数据包的 data 字段携带展示框的朝向。
        self.entity.data.store(i32::from(facing), Ordering::Relaxed);
        self.item_drop_chance
            .store(nbt.get_float("ItemDropChance").unwrap_or(1.0));
        self.invisible.store(
            nbt.get_bool("Invisible").unwrap_or(false),
            Ordering::Relaxed,
        );
        self.fixed
            .store(nbt.get_bool("Fixed").unwrap_or(false), Ordering::Relaxed);
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn init_data_tracker(&self) {
        let item_serializer = ItemStackSerializer::from(
            self.item_stack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        );
        let rotation = self.get_rotation() as i32;

        self.entity.set_synced_data(
            papokin_data::tracked_data::item_frame::ITEM,
            item_serializer,
        );
        self.entity
            .set_synced_data(papokin_data::tracked_data::item_frame::ROTATION, rotation);
    }

    fn send_java_spawn_packet(&self, client: &crate::net::java::JavaClient) {
        let spawn_packet = self.entity.create_spawn_packet();
        if let Ok(data) = client.serialize_packet(&spawn_packet) {
            client.try_enqueue_packet(data);
        }

        let ver = client.version.load();
        if ver >= JavaMinecraftVersion::V_1_21 {
            let item_serializer = ItemStackSerializer::from(
                self.item_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
            );
            let rotation = self.get_rotation() as i32;

            let mut data = Vec::new();
            let meta_item = Metadata::new(
                papokin_data::tracked_data::item_frame::ITEM,
                item_serializer,
            );
            let meta_rot =
                Metadata::new(papokin_data::tracked_data::item_frame::ROTATION, rotation);

            if meta_item.write(&mut data, &ver).is_ok() && meta_rot.write(&mut data, &ver).is_ok() {
                data.push(255);
                let meta_packet =
                    CSetEntityMetadata::new(self.entity.entity_id.into(), data.into());
                if let Ok(meta_data) = client.serialize_packet(&meta_packet) {
                    client.try_enqueue_packet(meta_data);
                }
            }
        }
    }

    fn interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if self.is_fixed() {
            return false;
        }

        let frame_has_item = !self.get_item().is_empty();
        let has_held_item = !item_stack.is_empty();

        if frame_has_item {
            // 旋转钩子。
            let mut change_event = crate::plugin::api::events::player::player_item_frame_change::PlayerItemFrameChangeEvent::new(
                player.clone(),
                self.entity.entity_id,
                self.get_item(),
                crate::plugin::api::events::player::player_item_frame_change::ItemFrameAction::Rotate,
            );
            let world = self.entity.world.load();
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut change_event);
            }
            if change_event.cancelled {
                return false;
            }
            let new_rot = self.get_rotation() + 1;
            self.set_rotation(new_rot, true);
            self.entity.play_sound(self.get_rotate_item_sound());
            true
        } else if has_held_item && !self.entity.removed.load(Ordering::Relaxed) {
            let mut new_stack = item_stack.clone();
            new_stack.item_count = 1;
            // 放置钩子。
            let mut change_event = crate::plugin::api::events::player::player_item_frame_change::PlayerItemFrameChangeEvent::new(
                player.clone(),
                self.entity.entity_id,
                new_stack.clone(),
                crate::plugin::api::events::player::player_item_frame_change::ItemFrameAction::Place,
            );
            let world = self.entity.world.load();
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut change_event);
            }
            if change_event.cancelled {
                return false;
            }
            self.set_item(new_stack, true);

            if !player.is_creative() {
                item_stack.decrement(1);
            }
            true
        } else {
            false
        }
    }

    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        _amount: f32,
        damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        let fixed = self.is_fixed();
        let is_creative_player = source.is_some_and(|s| {
            s.cast_any()
                .downcast_ref::<Player>()
                .is_some_and(Player::is_creative)
        });
        let bypasses_invuln =
            damage_type == DamageType::OUT_OF_WORLD || damage_type == DamageType::GENERIC_KILL;

        if fixed {
            if !bypasses_invuln && !is_creative_player {
                return false;
            }
            if !self.fire_break_events(source) {
                return true;
            }
            self.drop_item(source, true);
            self.entity.remove();
            return true;
        }

        let has_item = !self.get_item().is_empty();
        let is_explosion =
            damage_type == DamageType::EXPLOSION || damage_type == DamageType::PLAYER_EXPLOSION;

        if !is_explosion && has_item {
            // 移除钩子（玩家击打已装填的展示框）。
            if let Some(player) = source.and_then(|s| s.cast_any().downcast_ref::<Player>()) {
                // 源 `&Player` 并不携带 `Arc`；需要另行解析它
                // 从世界玩家列表中移除。
                let Some(player_arc) = self
                    .entity
                    .world
                    .load()
                    .get_player_by_uuid(player.gameprofile.id)
                else {
                    return false;
                };
                let mut change_event = crate::plugin::api::events::player::player_item_frame_change::PlayerItemFrameChangeEvent::new(
                    player_arc,
                    self.entity.entity_id,
                    self.get_item(),
                    crate::plugin::api::events::player::player_item_frame_change::ItemFrameAction::Remove,
                );
                let world = self.entity.world.load();
                if let Some(server) = world.server.upgrade() {
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut change_event);
                }
                if change_event.cancelled {
                    return false;
                }
            }
            self.drop_item(source, false);
            self.entity.play_sound(self.get_remove_item_sound());
        } else {
            if !self.fire_break_events(source) {
                return true;
            }
            self.drop_item(source, true);
            self.entity.play_sound(self.get_break_sound());
            self.entity.remove();
        }
        true
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
