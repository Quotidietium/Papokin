use super::{NBTStorage, NBTStorageInit, finite_non_negative_f32_or, player::Player};
use crossbeam::atomic::AtomicCell;
use papokin_data::damage::DamageType;
use papokin_nbt::compound::NbtCompound;
use papokin_util::Difficulty;

const MAX_FOOD: u8 = 20;
const EXHAUSTION_COST: f32 = 4.0;
const MAX_EXHAUSTION: f32 = 40.0;

pub struct HungerManager {
    pub level: AtomicCell<u8>,
    pub saturation: AtomicCell<f32>,
    pub exhaustion: AtomicCell<f32>,
    pub tick_timer: AtomicCell<u32>,
}

impl Default for HungerManager {
    fn default() -> Self {
        Self {
            level: AtomicCell::new(MAX_FOOD),
            saturation: AtomicCell::new(5.0),
            exhaustion: AtomicCell::new(0.0),
            tick_timer: AtomicCell::new(0),
        }
    }
}

impl HungerManager {
    pub fn tick(&self, player: &Player) {
        let mut level = self.level.load();
        let mut saturation = self.saturation.load();
        let mut exhaustion = self.exhaustion.load();
        let mut timer = self.tick_timer.load();

        let level_info = player.world().level_info.load();
        let difficulty = level_info.difficulty;
        let natural_regen = level_info.game_rules.natural_health_regeneration;
        let health = player.living_entity.health.load();
        let can_heal = player.can_food_heal();

        let mut needs_sync = false;
        let mut heal_amount = 0.0;
        let mut damage_amount = 0.0;

        if exhaustion > EXHAUSTION_COST {
            exhaustion -= EXHAUSTION_COST;
            if saturation > 0.0 {
                saturation = (saturation - 1.0).max(0.0);
            } else if difficulty != Difficulty::Peaceful {
                level = level.saturating_sub(1);
            }
            needs_sync = true;
        }

        if natural_regen && saturation > 0.0 && can_heal && level >= 20 {
            timer += 1;
            if timer >= 10 {
                let cost = saturation.min(6.0);
                heal_amount = cost / 6.0;
                exhaustion = (exhaustion + cost).min(MAX_EXHAUSTION);
                timer = 0;
                needs_sync = true;
            }
        } else if natural_regen && level >= 18 && can_heal {
            timer += 1;
            if timer >= 80 {
                heal_amount = 1.0;
                exhaustion = (exhaustion + 6.0).min(MAX_EXHAUSTION);
                timer = 0;
                needs_sync = true;
            }
        } else if level == 0 {
            timer += 1;
            if timer >= 80 {
                timer = 0;
                let should_starve = health > 10.0
                    || difficulty == Difficulty::Hard
                    || (health > 1.0 && difficulty == Difficulty::Normal);

                if should_starve {
                    damage_amount = 1.0;
                }
                self.tick_timer.store(0);
            }
        } else {
            timer = 0;
        }

        if needs_sync || timer != self.tick_timer.load() {
            self.level.store(level);
            self.saturation.store(saturation);
            self.exhaustion.store(exhaustion);
            self.tick_timer.store(timer);
        }

        if needs_sync {
            player.send_health();
        }
        if heal_amount > 0.0 {
            player.heal(heal_amount);
        }
        if damage_amount > 0.0 {
            player
                .living_entity
                .damage(player, damage_amount, DamageType::STARVE);
        }
    }

    pub fn eat(&self, player: &Player, food: u8, saturation: f32) {
        let new_level = self.level.load().saturating_add(food).min(MAX_FOOD);
        let new_sat = (self.saturation.load() + saturation).clamp(0.0, f32::from(new_level));

        self.level.store(new_level);
        self.saturation.store(new_sat);

        player.send_health();
    }

    /// 添加消耗值以触发饥饿度下降
    pub fn add_exhaustion(&self, exhaustion: f32) {
        let current = self.exhaustion.load();
        self.exhaustion
            .store((current + exhaustion).min(MAX_EXHAUSTION));
    }

    /// 手动添加饥饿值
    pub fn add_hunger(&self, hunger: u8) {
        let current = self.level.load();
        self.level.store((current + hunger).min(MAX_FOOD));
    }

    /// 手动添加饱和度
    pub fn add_saturation(&self, saturation: f32) {
        let current = self.saturation.load();
        self.saturation
            .store((current + saturation).min(f32::from(self.level.load())));
    }

    pub fn set_level(&self, level: u8) {
        self.level.store(level);
    }

    pub fn set_saturation(&self, saturation: f32) {
        self.saturation.store(saturation);
    }

    pub fn get_exhaustion(&self) -> f32 {
        self.exhaustion.load()
    }

    pub fn set_exhaustion(&self, exhaustion: f32) {
        self.exhaustion.store(exhaustion.min(MAX_EXHAUSTION));
    }

    pub fn restart(&self) {
        self.level.store(MAX_FOOD);
        self.saturation.store(5.0);
        self.exhaustion.store(0.0);
        self.tick_timer.store(0);
    }
}

impl NBTStorage for HungerManager {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("foodLevel", self.level.load().into());
        nbt.put_float("foodSaturationLevel", self.saturation.load());
        nbt.put_float("foodExhaustionLevel", self.exhaustion.load());
        nbt.put_int("foodTickTimer", self.tick_timer.load() as i32);
    }

    fn read_nbt_non_mut(&self, nbt: &NbtCompound) {
        // 饥饿值合法域为 0..=20；饱和/消耗值拒绝 NaN/Inf 与负数，
        // 防止畸形存档经进食/消耗算术持续污染饥饿系统。
        self.level
            .store(nbt.get_int("foodLevel").unwrap_or(20).clamp(0, 20) as u8);
        let saturation =
            finite_non_negative_f32_or(nbt.get_float("foodSaturationLevel").unwrap_or(5.0), 5.0);
        self.saturation.store(saturation);
        let exhaustion =
            finite_non_negative_f32_or(nbt.get_float("foodExhaustionLevel").unwrap_or(0.0), 0.0);
        self.exhaustion.store(exhaustion);
        self.tick_timer
            .store(nbt.get_int("foodTickTimer").unwrap_or(0) as u32);
    }
}

impl NBTStorageInit for HungerManager {}
