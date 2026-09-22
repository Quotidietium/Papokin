use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use crossbeam::atomic::AtomicCell;
use papokin_data::Block;
use papokin_data::effect::StatusEffect;
use papokin_data::entity::{EntityStatus, EntityType};
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::potion::Effect;
use papokin_data::tag::{self, Taggable};
use papokin_data::tracked_data;
use papokin_data::world::WorldEvent;
use papokin_nbt::compound::NbtCompound;
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::Metadata;
use papokin_util::math::position::BlockPos;
use papokin_util::version::JavaMinecraftVersion;
use uuid::Uuid;

use crate::entity::mob::zombie::ZombieEntityBase;
use crate::entity::mob::{Mob, MobEntity};
use crate::entity::passive::villager::VillagerEntity;
use crate::entity::passive::villager::data::{GossipType, VillagerData};
use crate::entity::player::Player;
use crate::entity::player::advancement::trigger::AdvancementTrigger;
use crate::entity::{Entity, EntityBase};

/// 可加快僵尸村民治愈倒计时的方块：铁栏杆和床。
fn speeds_up_conversion(block: &Block) -> bool {
    block == &Block::IRON_BARS || block.has_tag(&tag::Block::MINECRAFT_BEDS)
}

pub struct ZombieVillagerEntity {
    pub mob_entity: Arc<ZombieEntityBase>,
    villager_data: std::sync::Mutex<VillagerData>,
    /// `Offers` 与 `Gossips` 按原版 NBT 的存储方式保留，因此治愈后可以
    /// 将它们直接交回它要转化成的村民。
    villager_nbt: std::sync::Mutex<NbtCompound>,
    villager_xp: AtomicI32,
    /// 距离治愈完成剩余的刻数；`0` 表示该生物未在转化中。
    conversion_time: AtomicI32,
    conversion_starter: AtomicCell<Option<Uuid>>,
}

impl ZombieVillagerEntity {
    const VILLAGER_CONVERSION_WAIT_MIN: i32 = 3600;
    const VILLAGER_CONVERSION_WAIT_MAX: i32 = 6000;
    const MAX_SPECIAL_BLOCKS_COUNT: i32 = 14;
    const SPECIAL_BLOCK_RADIUS_X: i32 = 4;
    const SPECIAL_BLOCK_RADIUS_Y: i32 = 4;
    const SPECIAL_BLOCK_RADIUS_Z: i32 = 4;
    /// 被治愈的村民醒来时带有的反胃效果。
    const NAUSEA_DURATION: i32 = 200;

    pub fn new(entity: Entity) -> Arc<Self> {
        Self::with_can_break_doors(entity, false)
    }

    #[must_use]
    pub fn with_can_break_doors(entity: Entity, can_break_doors: bool) -> Arc<Self> {
        let mob_entity = ZombieEntityBase::with_can_break_doors(entity, can_break_doors);
        Arc::new(Self {
            mob_entity,
            villager_data: std::sync::Mutex::new(VillagerData::new(
                papokin_data::villager::VillagerType::Plains,
                // 原版的 `initializeZombieVillagerData` 会随机抽取一个职业
                // 使治愈后的村民拥有交易。之后 NBT 加载的数据会覆盖它。
                papokin_data::villager::VillagerProfession::from_i32(rand::random_range(0..15))
                    .unwrap_or(papokin_data::villager::VillagerProfession::None),
                1,
            )),
            villager_nbt: std::sync::Mutex::new(NbtCompound::new()),
            villager_xp: AtomicI32::new(0),
            conversion_time: AtomicI32::new(0),
            conversion_starter: AtomicCell::new(None),
        })
    }

    #[must_use]
    pub fn get_villager_data(&self) -> VillagerData {
        *self
            .villager_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_villager_data(&self, data: VillagerData) {
        *self
            .villager_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = data;
        self.get_entity()
            .set_synced_data(tracked_data::zombie_villager::VILLAGER_DATA, data);
    }

    #[must_use]
    pub fn is_converting(&self) -> bool {
        self.conversion_time.load(Ordering::Relaxed) > 0
    }

    #[must_use]
    pub fn is_baby(&self) -> bool {
        self.get_entity().age.load(Ordering::Relaxed) < 0
    }

    /// 原版 `startConverting`：开始治疗，将虚弱换成力量
    /// 并让客户端播放治愈音效。
    pub fn start_converting(&self, starter: Option<Uuid>, ticks: i32) {
        let living = &self.mob_entity.mob_entity.living_entity;
        let entity = &living.entity;

        self.conversion_starter.store(starter);
        self.conversion_time.store(ticks, Ordering::Relaxed);
        entity.set_synced_data(tracked_data::zombie_villager::DATA_CONVERTING_ID, true);
        living.remove_effect(&StatusEffect::WEAKNESS);
        living.add_effect(Effect {
            effect_type: &StatusEffect::STRENGTH,
            duration: ticks,
            // 原版计算 `min(difficulty - 1, 0)`，在每个
            // 僵尸村民可被治愈的难度。
            amplifier: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
            blend: false,
        });
        entity
            .world
            .load()
            .send_entity_status(entity, EntityStatus::ZombieConverting);
    }

    /// 原版 `getConversionProgress`：附近的铁栏杆和床会加快治疗速度。
    #[expect(clippy::cast_possible_truncation)]
    fn conversion_progress(&self) -> i32 {
        let mut progress = 1;
        if rand::random::<f32>() >= 0.01 {
            return progress;
        }

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        let (x, y, z) = (pos.x as i32, pos.y as i32, pos.z as i32);
        let mut special_blocks = 0;

        for block_x in x - Self::SPECIAL_BLOCK_RADIUS_X..x + Self::SPECIAL_BLOCK_RADIUS_X {
            for block_y in y - Self::SPECIAL_BLOCK_RADIUS_Y..y + Self::SPECIAL_BLOCK_RADIUS_Y {
                for block_z in z - Self::SPECIAL_BLOCK_RADIUS_Z..z + Self::SPECIAL_BLOCK_RADIUS_Z {
                    if special_blocks >= Self::MAX_SPECIAL_BLOCKS_COUNT {
                        return progress;
                    }
                    let block = world.get_block(&BlockPos::new(block_x, block_y, block_z));
                    if speeds_up_conversion(block) {
                        if rand::random::<f32>() < 0.3 {
                            progress += 1;
                        }
                        special_blocks += 1;
                    }
                }
            }
        }

        progress
    }

    /// 原版 `finishConversion`：将僵尸村民替换为村民
    /// 它所记忆的状态。
    fn finish_conversion(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();

        let villager = VillagerEntity::new(Entity::new(
            world.clone(),
            entity.pos.load(),
            &EntityType::VILLAGER,
        ));
        let villager_entity = villager.get_entity();
        villager_entity.set_rotation(entity.yaw.load(), entity.pitch.load());
        villager_entity.head_yaw.store(entity.head_yaw.load());
        villager_entity.velocity.store(entity.velocity.load());
        villager_entity.invulnerable.store(
            entity.invulnerable.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        if self.is_baby() {
            villager_entity
                .age
                .store(entity.age.load(Ordering::Relaxed), Ordering::Relaxed);
        }
        if let Some(custom_name) = &**entity.custom_name.load() {
            villager_entity.set_custom_name(custom_name.clone());
            villager_entity
                .set_custom_name_visible(entity.custom_name_visible.load(Ordering::Relaxed));
        }
        villager
            .mob_entity
            .set_no_ai(self.mob_entity.mob_entity.is_no_ai());
        villager.mob_entity.persistence_required.store(
            self.mob_entity
                .mob_entity
                .persistence_required
                .load(Ordering::Relaxed),
            Ordering::Relaxed,
        );

        // 将村民的职业、交易、流言和交易经验返还给它。
        let data = self.get_villager_data();
        let mut nbt = self
            .villager_nbt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        nbt.put_int("Xp", self.villager_xp.load(Ordering::Relaxed));
        villager.mob_read_nbt(&nbt);
        villager.set_villager_data(data);

        if let Some(starter) = self.conversion_starter.load() {
            // 原版的 `ZOMBIE_VILLAGER_CURED` 声望事件。
            let mut gossips = villager
                .gossips
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let entries = gossips.entry(starter).or_default();
            for (gossip_type, amount) in [
                (GossipType::MajorPositive, 20),
                (GossipType::MinorPositive, 25),
            ] {
                let value = entries.entry(gossip_type).or_default();
                *value = (*value + amount).min(gossip_type.max_value());
            }
            drop(gossips);

            if let Some(player) = world.get_player_by_uuid(starter) {
                player.trigger_advancement(AdvancementTrigger::CuredZombieVillager);
            }
        }

        villager.mob_entity.living_entity.add_effect(Effect {
            effect_type: &StatusEffect::NAUSEA,
            duration: Self::NAUSEA_DURATION,
            amplifier: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
            blend: false,
        });

        world.spawn_entity(villager);
        entity.remove();

        if !entity.is_silent() {
            world.sync_world_event(WorldEvent::SoundZombieConverted, entity.block_pos.load(), 0);
        }
    }
}

impl Mob for ZombieVillagerEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity.mob_entity
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        !self.is_converting() && self.villager_xp.load(Ordering::Relaxed) == 0
    }

    fn mob_java_spawn_metadata(&self, version: JavaMinecraftVersion) -> Option<Box<[u8]>> {
        if version < JavaMinecraftVersion::V_1_9 {
            return None;
        }
        let mut metadata = Vec::new();
        Metadata::new(
            tracked_data::zombie_villager::VILLAGER_DATA,
            self.get_villager_data(),
        )
        .write(&mut metadata, &version)
        .ok()?;
        Metadata::new(
            tracked_data::zombie_villager::DATA_CONVERTING_ID,
            self.is_converting(),
        )
        .write(&mut metadata, &version)
        .ok()?;
        metadata.push(255);
        Some(metadata.into_boxed_slice())
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if item_stack.get_item() == &Item::GOLDEN_APPLE {
            if self
                .mob_entity
                .mob_entity
                .living_entity
                .has_effect(&StatusEffect::WEAKNESS)
            {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                self.start_converting(
                    Some(player.get_entity().entity_uuid),
                    rand::random_range(
                        Self::VILLAGER_CONVERSION_WAIT_MIN..=Self::VILLAGER_CONVERSION_WAIT_MAX,
                    ),
                );
            }
            return true;
        }
        self.mob_entity.mob_interact(player, item_stack)
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        if !self.get_entity().is_alive() || !self.is_converting() {
            return;
        }
        let progress = self.conversion_progress();
        if self.conversion_time.fetch_sub(progress, Ordering::Relaxed) - progress <= 0 {
            self.conversion_time.store(0, Ordering::Relaxed);
            self.finish_conversion();
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.mob_entity.mob_write_nbt(nbt);

        let data = self.get_villager_data();
        let mut villager_data_nbt = NbtCompound::new();
        villager_data_nbt.put_int("Type", data.r#type.0);
        villager_data_nbt.put_int("Profession", data.profession.0);
        villager_data_nbt.put_int("Level", data.level.0);
        nbt.put_compound("VillagerData", villager_data_nbt);
        nbt.put_int("Xp", self.villager_xp.load(Ordering::Relaxed));

        {
            let carried = self
                .villager_nbt
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(offers) = carried.get_compound("Offers") {
                nbt.put_compound("Offers", offers.clone());
            }
            if let Some(gossips) = carried.get_list("Gossips") {
                nbt.put_list("Gossips", gossips.to_vec());
            }
        }

        let conversion_time = self.conversion_time.load(Ordering::Relaxed);
        nbt.put_int(
            "ConversionTime",
            if conversion_time > 0 {
                conversion_time
            } else {
                -1
            },
        );
        if let Some(starter) = self.conversion_starter.load() {
            nbt.put_uuid("ConversionPlayer", starter);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.mob_entity.mob_read_nbt(nbt);

        if let Some(villager_data_nbt) = nbt.get_compound("VillagerData") {
            let mut data = self.get_villager_data();
            if let Some(r#type) = villager_data_nbt.get_int("Type") {
                data.r#type = VarInt(r#type);
            }
            if let Some(profession) = villager_data_nbt.get_int("Profession") {
                data.profession = VarInt(profession);
            }
            if let Some(level) = villager_data_nbt.get_int("Level") {
                data.level = VarInt(level);
            }
            self.set_villager_data(data);
        }

        if let Some(xp) = nbt.get_int("Xp") {
            self.villager_xp.store(xp, Ordering::Relaxed);
        }

        let mut carried = NbtCompound::new();
        if let Some(offers) = nbt.get_compound("Offers") {
            carried.put_compound("Offers", offers.clone());
        }
        if let Some(gossips) = nbt.get_list("Gossips") {
            carried.put_list("Gossips", gossips.to_vec());
        }
        *self
            .villager_nbt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = carried;

        if let Some(conversion_time) = nbt.get_int("ConversionTime")
            && conversion_time > 0
        {
            self.start_converting(nbt.get_uuid("ConversionPlayer"), conversion_time);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Block, speeds_up_conversion};

    #[test]
    fn only_iron_bars_and_beds_speed_up_the_cure() {
        assert!(speeds_up_conversion(&Block::IRON_BARS));
        assert!(speeds_up_conversion(&Block::RED_BED));
        assert!(speeds_up_conversion(&Block::WHITE_BED));
        assert!(!speeds_up_conversion(&Block::COPPER_BARS));
        assert!(!speeds_up_conversion(&Block::STONE));
        assert!(!speeds_up_conversion(&Block::RED_WOOL));
    }
}
