use crate::block::entities::BlockEntity;
use crate::world::World;
use crossbeam::atomic::AtomicCell;
use papokin_data::block_properties::HorizontalFacing;
use papokin_data::effect::StatusEffect;
use papokin_data::potion::Effect;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_data::tag::{self, Taggable};
use papokin_nbt::compound::NbtCompound;
use papokin_util::math::position::BlockPos;
use std::any::Any;
use std::sync::Arc;
use std::sync::RwLock;

/// 共振期间高亮袭击者的搜索半径（原版 `HIGHLIGHT_RAIDERS_RADIUS`）。
const HIGHLIGHT_RADIUS: f64 = 48.0;
/// 发光效果时长（原版 `GLOW_DURATION`，3 秒，共振期间每刻刷新）。
const GLOW_DURATION: i32 = 60;
/// 响铃瞬间侦测袭击者的半径（原版为响铃位置 32 格）。
const HEAR_RADIUS: f64 = 32.0;

pub struct BellBlockEntity {
    pub position: BlockPos,
    pub last_side_hit: AtomicCell<Option<HorizontalFacing>>,
    pub ring_ticks: AtomicCell<i32>,
    pub ringing: AtomicCell<bool>,
    resonating: AtomicCell<bool>,
    resonate_time: AtomicCell<i32>,
    /// 响铃瞬间记录的 32 格内袭击者实体 ID（原版同样在响铃时捕获列表）。
    nearby_raiders: RwLock<Vec<i32>>,
    /// 本次共振是否允许高亮袭击者（由 `BellResonateEvent` 决定）。
    highlight_allowed: AtomicCell<bool>,
}

impl BellBlockEntity {
    pub const ID: &'static str = "minecraft:bell";
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            last_side_hit: AtomicCell::new(None),
            ring_ticks: AtomicCell::new(0),
            resonate_time: AtomicCell::new(0),
            resonating: AtomicCell::new(false),
            ringing: AtomicCell::new(false),
            nearby_raiders: RwLock::new(Vec::new()),
            highlight_allowed: AtomicCell::new(true),
        }
    }
    pub fn activate(&self, direction: HorizontalFacing, world: &Arc<World>) {
        self.last_side_hit.store(Some(direction));
        if self.ringing.load() {
            self.ring_ticks.store(0);
        } else {
            self.ringing.store(true);
        }

        // 原版语义：响铃瞬间捕获 32 格内的袭击者（#minecraft:raiders），
        // 后续共振高亮只针对这一批。
        let center = self.position.to_centered_f64();
        let raiders = world
            .get_nearby_entities(center, HEAR_RADIUS)
            .into_values()
            .filter(|entity| {
                entity
                    .get_entity()
                    .entity_type
                    .has_tag(&tag::EntityType::MINECRAFT_RAIDERS)
                    && entity.get_living_entity().is_some()
            })
            .map(|entity| entity.get_entity().entity_id)
            .collect::<Vec<i32>>();
        *self
            .nearby_raiders
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = raiders;
    }

    pub fn raiders_hear_bell(&self) -> bool {
        !self
            .nearby_raiders
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }

    /// 共振期间给仍在 48 格内且存活的袭击者上无粒子发光（每刻刷新，原版 `highlightRaiders`）。
    fn highlight_raiders(&self, world: &Arc<World>) {
        if !self.highlight_allowed.load() {
            return;
        }
        let center = self.position.to_centered_f64();
        let ids = self
            .nearby_raiders
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for id in ids {
            let Some(entity) = world.get_entity_by_id(id) else {
                continue;
            };
            let pos = entity.get_entity().pos.load();
            if pos.squared_distance_to_vec(&center) > HIGHLIGHT_RADIUS * HIGHLIGHT_RADIUS {
                continue;
            }
            if let Some(living) = entity.get_living_entity() {
                living.add_effect(Effect {
                    effect_type: &StatusEffect::GLOWING,
                    duration: GLOW_DURATION,
                    amplifier: 0,
                    ambient: false,
                    show_particles: false,
                    show_icon: false,
                    blend: false,
                });
            }
        }
    }
}

impl BlockEntity for BellBlockEntity {
    fn write_nbt(&self, _nbt: &mut NbtCompound) {}

    fn from_nbt(_nbt: &NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        Self::new(position)
    }

    fn tick(&self, world: &Arc<World>) {
        if self.ringing.load() {
            self.ring_ticks.fetch_add(1);
        }
        if self.ring_ticks.load() >= 50 {
            self.ringing.store(false);
            self.ring_ticks.store(0);
        }
        if self.ring_ticks.load() >= 5 && self.resonate_time.load() == 0 && self.raiders_hear_bell()
        {
            self.resonating.store(true);
            self.highlight_allowed.store(true);
            world.play_sound_fine(
                Sound::BlockBellResonate,
                SoundCategory::Blocks,
                &self.position.to_f64(),
                1.0,
                1.0,
            );
            // 钟共鸣事件，取消则本次共振不高亮袭击者
            let mut event =
                crate::plugin::api::events::block::bell_resonate::BellResonateEvent::new(
                    self.position,
                    world.clone(),
                );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    self.highlight_allowed.store(false);
                }
            }
        }

        if self.resonating.load() {
            if self.resonate_time.load() < 40 {
                self.resonate_time.fetch_add(1);
            } else {
                self.resonating.store(false);
                // 清零，否则后续响铃永远无法再次进入共振
                self.resonate_time.store(0);
            }
            self.highlight_raiders(world);
        }
    }

    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
