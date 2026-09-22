//! 主要 Minecraft 生物实体的专用类型包装。
//!
//! 提供类型化访问器与便捷辅助方法（`Sheep::from_entity`、`Wolf::from_mob` 等），
//! 而无需数十个 WIT 资源。

use std::ops::Deref;

pub use crate::wit::papokin::plugin::uuid::Uuid;
pub use crate::wit::papokin::plugin::world::{
    AgeableData, BlockDirection, BrainMemory, BuiltinAiGoal, CatData, CreeperData, DyeColor,
    EndermanData, Entity, FoxData, IronGolemData, LivingEntity, MemoryStatus, Mob, MobData,
    SheepData, ShulkerData, SlimeData, VillagerData, VillagerProfession, WolfData, ZombieData,
};

/// [`Mob::get_brain_memory`] 所接受的原版大脑记忆类型名称。
///
/// 这些是 Bukkit `MemoryKey` 的等价物。`get_brain_memory` 也接受
/// 不带 `minecraft:` 命名空间的裸路径（例如 `"walk_target"`），并且
/// 未在此列出的任何其他原版记忆名称。
pub mod memory_keys {
    /// 该生物当前正走向的位置。
    pub const WALK_TARGET: &str = "minecraft:walk_target";
    /// 该生物当前注视的对象。
    pub const LOOK_TARGET: &str = "minecraft:look_target";
    /// 生物当前的攻击目标。
    pub const ATTACK_TARGET: &str = "minecraft:attack_target";
    /// 该生物的攻击是否正在冷却。
    pub const ATTACK_COOLING_DOWN: &str = "minecraft:attack_cooling_down";
    /// 此生物正在交互的实体。
    pub const INTERACTION_TARGET: &str = "minecraft:interaction_target";
    /// 此生物想要与之繁殖的实体。
    pub const BREED_TARGET: &str = "minecraft:breed_target";
    /// 生物的家所在位置（例如村民）。
    pub const HOME: &str = "minecraft:home";
    /// 生物的工作站点位置（村民）。
    pub const JOB_SITE: &str = "minecraft:job_site";
    /// 村民的集合点。
    pub const MEETING_POINT: &str = "minecraft:meeting_point";
    /// 该 mob 当前可见的生物实体。
    pub const NEAREST_VISIBLE_LIVING_ENTITIES: &str = "minecraft:visible_mobs";
    /// 该 mob 附近的玩家。
    pub const NEAREST_PLAYERS: &str = "minecraft:nearest_players";
    /// 生物可见的最近玩家。
    pub const NEAREST_VISIBLE_PLAYER: &str = "minecraft:nearest_visible_player";
    /// 生物可见的最近敌对实体。
    pub const NEAREST_HOSTILE: &str = "minecraft:nearest_hostile";
    /// 最近伤害该生物的伤害来源。
    pub const HURT_BY: &str = "minecraft:hurt_by";
    /// 最近伤害该生物的实体。
    pub const HURT_BY_ENTITY: &str = "minecraft:hurt_by_entity";
    /// 此生物正在躲避的实体。
    pub const AVOID_TARGET: &str = "minecraft:avoid_target";
    /// 生物所知道的最近的床。
    pub const NEAREST_BED: &str = "minecraft:nearest_bed";
    /// 生物当前的寻路路径。
    pub const PATH: &str = "minecraft:path";
    /// 自该生物无法再到达行走目标以来经过的刻数。
    pub const CANT_REACH_WALK_TARGET_SINCE: &str = "minecraft:cant_reach_walk_target_since";
    /// 吸引此生物的玩家。
    pub const TEMPTING_PLAYER: &str = "minecraft:tempting_player";
    /// 该生物当前是否被引诱。
    pub const IS_TEMPTED: &str = "minecraft:is_tempted";
    /// 该生物当前是否处于惊慌状态。
    pub const IS_PANICKING: &str = "minecraft:is_panicking";
    /// 该生物对其愤怒的实体的 UUID（例如僵尸猪灵）。
    pub const ANGRY_AT: &str = "minecraft:angry_at";
    /// 生物上次听到钟声的时间。
    pub const HEARD_BELL_TIME: &str = "minecraft:heard_bell_time";
}

/// 由所有特化生物包装器实现的 trait，以支持通过 `.cast::<T>()` 进行泛型向下转换。
pub trait MobCast<'a>: Sized {
    /// 若底层实体数据匹配，则尝试包装 [`Mob`] 引用。
    fn from_mob(mob: &'a Mob) -> Option<Self>;

    /// 若 [`Entity`] 引用是匹配此类型的 AI mob，则尝试包装它。
    fn from_entity(entity: &'a Entity) -> Option<Self> {
        let mob = entity.as_mob()?;
        // 注意：as_mob() 返回一个新的 Resource 句柄，因此这里通过 MobData 检查来提取。
        Self::from_mob_owned(mob)
    }

    /// 若 [`LivingEntity`] 引用是匹配此类型的 AI mob，则尝试包装它。
    fn from_living(living: &'a LivingEntity) -> Option<Self> {
        let mob = living.as_mob()?;
        Self::from_mob_owned(mob)
    }

    #[doc(hidden)]
    fn from_mob_owned(mob: Mob) -> Option<Self>;
}

macro_rules! define_mob_wrapper {
    (
        $(#[$meta:meta])*
        $name:ident, $variant:ident, $data_ty:ident
    ) => {
        $(#[$meta])*
        pub struct $name<'a> {
            mob: &'a Mob,
            _owned: Option<Mob>,
        }

        impl<'a> $name<'a> {
            /// 若该生物为匹配类型，则包装其借用的 [`Mob`] 引用。
            #[must_use]
            pub fn from_mob(mob: &'a Mob) -> Option<Self> {
                if matches!(mob.get_mob_data(), MobData::$variant(_)) {
                    Some(Self { mob, _owned: None })
                } else {
                    None
                }
            }

            /// 若它是匹配类型的 AI 生物，则包装其借用的 [`Entity`] 引用。
            #[must_use]
            pub fn from_entity(entity: &'a Entity) -> Option<Self> {
                let mob = entity.as_mob()?;
                Self::from_mob_owned(mob)
            }

            /// 若它是匹配类型的 AI 生物，则包装其借用的 [`LivingEntity`] 引用。
            #[must_use]
            pub fn from_living(living: &'a LivingEntity) -> Option<Self> {
                let mob = living.as_mob()?;
                Self::from_mob_owned(mob)
            }

            fn from_mob_owned(mob: Mob) -> Option<Self> {
                if matches!(mob.get_mob_data(), MobData::$variant(_)) {
                    // 对所拥有 Mob 的安全引用扩展
                    let mob_ref = unsafe { &*(&mob as *const Mob) };
                    Some(Self {
                        mob: mob_ref,
                        _owned: Some(mob),
                    })
                } else {
                    None
                }
            }

            /// 检索该生物的底层数据记录。
            #[must_use]
            pub fn get_data(&self) -> Option<$data_ty> {
                match self.mob.get_mob_data() {
                    MobData::$variant(data) => Some(data),
                    _ => None,
                }
            }

            /// 更新此生物的底层数据记录。
            pub fn set_data(&self, data: $data_ty) -> bool {
                self.mob.set_mob_data(MobData::$variant(data))
            }
        }

        impl<'a> Deref for $name<'a> {
            type Target = Mob;

            fn deref(&self) -> &Self::Target {
                self.mob
            }
        }

        impl<'a> TryFrom<&'a Mob> for $name<'a> {
            type Error = ();

            fn try_from(mob: &'a Mob) -> Result<Self, Self::Error> {
                Self::from_mob(mob).ok_or(())
            }
        }

        impl<'a> TryFrom<&'a Entity> for $name<'a> {
            type Error = ();

            fn try_from(entity: &'a Entity) -> Result<Self, Self::Error> {
                Self::from_entity(entity).ok_or(())
            }
        }

        impl<'a> TryFrom<&'a LivingEntity> for $name<'a> {
            type Error = ();

            fn try_from(living: &'a LivingEntity) -> Result<Self, Self::Error> {
                Self::from_living(living).ok_or(())
            }
        }

        impl<'a> MobCast<'a> for $name<'a> {
            fn from_mob(mob: &'a Mob) -> Option<Self> {
                Self::from_mob(mob)
            }

            fn from_living(living: &'a LivingEntity) -> Option<Self> {
                Self::from_living(living)
            }

            fn from_mob_owned(mob: Mob) -> Option<Self> {
                Self::from_mob_owned(mob)
            }
        }
    };
}

define_mob_wrapper!(
    /// 针对绵羊实体的专用包装器。
    Sheep, Sheep, SheepData
);

impl<'a> Sheep<'a> {
    /// 获取此羊的羊毛染色。
    #[must_use]
    pub fn get_color(&self) -> DyeColor {
        self.get_data().map_or(DyeColor::White, |d| d.color)
    }

    /// 设置此绵羊羊毛的染色颜色。
    pub fn set_color(&self, color: DyeColor) {
        if let Some(mut data) = self.get_data() {
            data.color = color;
            self.set_data(data);
        }
    }

    /// 返回此绵羊是否已被剪过毛。
    #[must_use]
    pub fn is_sheared(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_sheared)
    }

    /// 设置此绵羊是否已被剪毛。
    pub fn set_sheared(&self, sheared: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_sheared = sheared;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对狼实体的专用包装器。
    Wolf, Wolf, WolfData
);

impl<'a> Wolf<'a> {
    /// 返回此狼是否已被驯服。
    #[must_use]
    pub fn is_tamed(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_tamed)
    }

    /// 设置此狼是否已被驯服。
    pub fn set_tamed(&self, tamed: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_tamed = tamed;
            self.set_data(data);
        }
    }

    /// 获取拥有此狼的玩家 UUID（如果有）。
    #[must_use]
    pub fn get_owner(&self) -> Option<Uuid> {
        self.get_data().and_then(|d| d.owner)
    }

    /// 通过 UUID 设置此狼的主人。
    pub fn set_owner(&self, owner: Option<Uuid>) {
        if let Some(mut data) = self.get_data() {
            data.owner = owner;
            self.set_data(data);
        }
    }

    /// 返回此狼当前是否处于坐下姿势。
    #[must_use]
    pub fn is_sitting(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_sitting)
    }

    /// 命令此狼坐下或站起。
    pub fn set_sitting(&self, sitting: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_sitting = sitting;
            self.set_data(data);
        }
    }

    /// 获取此狼的项圈颜色。
    #[must_use]
    pub fn get_collar_color(&self) -> DyeColor {
        self.get_data().map_or(DyeColor::Red, |d| d.collar_color)
    }

    /// 设置此狼项圈的染色颜色。
    pub fn set_collar_color(&self, color: DyeColor) {
        if let Some(mut data) = self.get_data() {
            data.collar_color = color;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对猫实体的专用包装器。
    Cat, Cat, CatData
);

impl<'a> Cat<'a> {
    /// 返回此猫是否已被驯服。
    #[must_use]
    pub fn is_tamed(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_tamed)
    }

    /// 设置此猫是否已被驯服。
    pub fn set_tamed(&self, tamed: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_tamed = tamed;
            self.set_data(data);
        }
    }

    /// 获取此猫的主人 UUID。
    #[must_use]
    pub fn get_owner(&self) -> Option<Uuid> {
        self.get_data().and_then(|d| d.owner)
    }

    /// 设置此猫主人的 UUID。
    pub fn set_owner(&self, owner: Option<Uuid>) {
        if let Some(mut data) = self.get_data() {
            data.owner = owner;
            self.set_data(data);
        }
    }

    /// 返回此猫是否处于坐下姿势。
    #[must_use]
    pub fn is_sitting(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_sitting)
    }

    /// 命令此猫坐下或站起。
    pub fn set_sitting(&self, sitting: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_sitting = sitting;
            self.set_data(data);
        }
    }

    /// 获取此猫的项圈颜色。
    #[must_use]
    pub fn get_collar_color(&self) -> DyeColor {
        self.get_data().map_or(DyeColor::Red, |d| d.collar_color)
    }

    /// 设置此猫项圈的染色颜色。
    pub fn set_collar_color(&self, color: DyeColor) {
        if let Some(mut data) = self.get_data() {
            data.collar_color = color;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对村民实体的专用包装器。
    Villager, Villager, VillagerData
);

impl<'a> Villager<'a> {
    /// 获取此村民的职业。
    #[must_use]
    pub fn get_profession(&self) -> VillagerProfession {
        self.get_data()
            .map_or(VillagerProfession::None, |d| d.profession)
    }

    /// 设置此村民的职业。
    pub fn set_profession(&self, profession: VillagerProfession) {
        if let Some(mut data) = self.get_data() {
            data.profession = profession;
            self.set_data(data);
        }
    }

    /// 获取此村民的交易等级（1-5）。
    #[must_use]
    pub fn get_level(&self) -> u8 {
        self.get_data().map_or(1, |d| d.level)
    }

    /// 设置此村民的交易等级（1-5）。
    pub fn set_level(&self, level: u8) {
        if let Some(mut data) = self.get_data() {
            data.level = level;
            self.set_data(data);
        }
    }

    /// 获取此村民的交易经验值。
    #[must_use]
    pub fn get_experience(&self) -> u32 {
        self.get_data().map_or(0, |d| d.experience)
    }

    /// 设置此村民的交易经验点数。
    pub fn set_experience(&self, experience: u32) {
        if let Some(mut data) = self.get_data() {
            data.experience = experience;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对苦力怕实体的专用包装器。
    Creeper, Creeper, CreeperData
);

impl<'a> Creeper<'a> {
    /// 返回此苦力怕是否为高压状态（被闪电击中过）。
    #[must_use]
    pub fn is_powered(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_powered)
    }

    /// 设置此苦力怕是否为高压（charged）状态。
    pub fn set_powered(&self, powered: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_powered = powered;
            self.set_data(data);
        }
    }

    /// 获取此苦力怕的引信时长（刻）。
    #[must_use]
    pub fn get_fuse(&self) -> i32 {
        self.get_data().map_or(30, |d| d.fuse)
    }

    /// 以刻为单位设置此苦力怕的引信时长。
    pub fn set_fuse(&self, fuse: i32) {
        if let Some(mut data) = self.get_data() {
            data.fuse = fuse;
            self.set_data(data);
        }
    }

    /// 返回此苦力怕是否已被打火石手动点燃。
    #[must_use]
    pub fn is_ignited(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_ignited)
    }

    /// 设置此苦力怕是否已被点燃。
    pub fn set_ignited(&self, ignited: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_ignited = ignited;
            self.set_data(data);
        }
    }

    /// 获取此苦力怕的爆炸半径。
    #[must_use]
    pub fn get_explosion_radius(&self) -> u8 {
        self.get_data().map_or(3, |d| d.explosion_radius)
    }

    /// 设置此苦力怕的爆炸半径。
    pub fn set_explosion_radius(&self, radius: u8) {
        if let Some(mut data) = self.get_data() {
            data.explosion_radius = radius;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对史莱姆与岩浆怪实体的专用包装器。
    Slime, Slime, SlimeData
);

impl<'a> Slime<'a> {
    /// 获取此史莱姆的尺寸级别。
    #[must_use]
    pub fn get_size(&self) -> i32 {
        self.get_data().map_or(1, |d| d.size)
    }

    /// 设置此史莱姆的尺寸缩放。
    pub fn set_size(&self, size: i32) {
        if let Some(mut data) = self.get_data() {
            data.size = size;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对末影人实体的专用包装器。
    Enderman, Enderman, EndermanData
);

impl<'a> Enderman<'a> {
    /// 获取此末影人携带的数字方块状态 ID（如果有）。
    #[must_use]
    pub fn get_carried_block(&self) -> Option<u16> {
        self.get_data().and_then(|d| d.carried_block_state)
    }

    /// 设置此末影人搬运的方块状态 ID。
    pub fn set_carried_block(&self, block_state: Option<u16>) {
        if let Some(mut data) = self.get_data() {
            data.carried_block_state = block_state;
            self.set_data(data);
        }
    }

    /// 返回此末影人当前是否正在尖叫（愤怒）。
    #[must_use]
    pub fn is_screaming(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_screaming)
    }

    /// 返回此末影人是否正盯着玩家看。
    #[must_use]
    pub fn is_staring(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_staring)
    }
}

define_mob_wrapper!(
    /// 针对铁傀儡实体的专用包装器。
    IronGolem, IronGolem, IronGolemData
);

impl<'a> IronGolem<'a> {
    /// 返回此铁傀儡是否由玩家创建。
    #[must_use]
    pub fn is_player_created(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_player_created)
    }

    /// 设置此铁傀儡是否被视为玩家创建。
    pub fn set_player_created(&self, created: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_player_created = created;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对狐狸实体的专用包装器。
    Fox, Fox, FoxData
);

impl<'a> Fox<'a> {
    /// 返回此狐狸是否正在坐下。
    #[must_use]
    pub fn is_sitting(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_sitting)
    }

    /// 设置此狐狸是否正在坐下。
    pub fn set_sitting(&self, sitting: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_sitting = sitting;
            self.set_data(data);
        }
    }

    /// 返回此狐狸是否正在睡觉。
    #[must_use]
    pub fn is_sleeping(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_sleeping)
    }

    /// 设置此狐狸是否正在睡觉。
    pub fn set_sleeping(&self, sleeping: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_sleeping = sleeping;
            self.set_data(data);
        }
    }

    /// 返回此狐狸是否正在蹲伏。
    #[must_use]
    pub fn is_crouching(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_crouching)
    }

    /// 设置此狐狸是否处于蹲伏状态。
    pub fn set_crouching(&self, crouching: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_crouching = crouching;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对潜影贝实体的专用包装器。
    Shulker, Shulker, ShulkerData
);

impl<'a> Shulker<'a> {
    /// 获取此潜影贝附着的方块方向。
    #[must_use]
    pub fn get_attached_face(&self) -> BlockDirection {
        self.get_data()
            .map_or(BlockDirection::Down, |d| d.attached_face)
    }

    /// 设置此潜影贝附着方块的方向。
    pub fn set_attached_face(&self, face: BlockDirection) {
        if let Some(mut data) = self.get_data() {
            data.attached_face = face;
            self.set_data(data);
        }
    }

    /// 获取此潜影贝的原始探头幅度（0-100）。
    #[must_use]
    pub fn get_peek_amount(&self) -> u8 {
        self.get_data().map_or(0, |d| d.peek_amount)
    }

    /// 设置此潜影贝的原始探出量（0-100）。
    pub fn set_peek_amount(&self, amount: u8) {
        if let Some(mut data) = self.get_data() {
            data.peek_amount = amount;
            self.set_data(data);
        }
    }

    /// 获取此潜影贝的自定义染色（若已染色）。
    #[must_use]
    pub fn get_color(&self) -> Option<DyeColor> {
        self.get_data().and_then(|d| d.color)
    }

    /// 设置或移除此潜影贝的自定义染色颜色。
    pub fn set_color(&self, color: Option<DyeColor>) {
        if let Some(mut data) = self.get_data() {
            data.color = color;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对僵尸实体的专用包装器。
    Zombie, Zombie, ZombieData
);

impl<'a> Zombie<'a> {
    /// 返回此僵尸是否为幼年僵尸。
    #[must_use]
    pub fn is_baby(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_baby)
    }

    /// 设置此僵尸是否为幼年个体。
    pub fn set_baby(&self, baby: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_baby = baby;
            self.set_data(data);
        }
    }

    /// 返回此僵尸是否能够破坏门。
    #[must_use]
    pub fn can_break_doors(&self) -> bool {
        self.get_data().is_some_and(|d| d.can_break_doors)
    }

    /// 设置此僵尸能否破坏门。
    pub fn set_can_break_doors(&self, can_break: bool) {
        if let Some(mut data) = self.get_data() {
            data.can_break_doors = can_break;
            self.set_data(data);
        }
    }
}

define_mob_wrapper!(
    /// 针对 Ageable（可成长）动物生物的专用包装器（牛、猪、鸡、兔子等）。
    Ageable, Ageable, AgeableData
);

impl<'a> Ageable<'a> {
    /// 返回此动物是否为幼崽。
    #[must_use]
    pub fn is_baby(&self) -> bool {
        self.get_data().is_some_and(|d| d.is_baby)
    }

    /// 设置此动物是否为幼年个体。
    pub fn set_baby(&self, baby: bool) {
        if let Some(mut data) = self.get_data() {
            data.is_baby = baby;
            self.set_data(data);
        }
    }

    /// 获取此动物的年龄（刻，幼年为负值）。
    #[must_use]
    pub fn get_age(&self) -> i32 {
        self.get_data().map_or(0, |d| d.age)
    }

    /// 以刻为单位设置此动物的年龄。
    pub fn set_age(&self, age: i32) {
        if let Some(mut data) = self.get_data() {
            data.age = age;
            self.set_data(data);
        }
    }
}

/// 在 [`Entity`] 与 [`Mob`] 上提供泛型 `.cast::<T>()` 向下转换的扩展 trait。
pub trait EntityCastExt {
    /// 尝试将该实体或 mob 引用转换为专用 mob 包装类型。
    fn cast<'a, T: MobCast<'a>>(&'a self) -> Option<T>;
}

impl EntityCastExt for Mob {
    fn cast<'a, T: MobCast<'a>>(&'a self) -> Option<T> {
        T::from_mob(self)
    }
}

impl EntityCastExt for Entity {
    fn cast<'a, T: MobCast<'a>>(&'a self) -> Option<T> {
        T::from_entity(self)
    }
}

impl EntityCastExt for LivingEntity {
    fn cast<'a, T: MobCast<'a>>(&'a self) -> Option<T> {
        T::from_living(self)
    }
}
