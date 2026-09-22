use crate::entity::mob::Mob;
use std::{any::TypeId, ops::BitOr, ptr};

pub mod active_target;
pub mod ambient_stand;
pub mod avoid_entity;
pub mod beg;
pub mod blaze_attack;
pub mod bow_attack;
pub mod break_door;
pub mod breed;
pub mod chase_player;
pub mod creeper_ignite;
pub mod destroy_egg;
pub mod door_interact;
pub mod eat_grass;
pub mod escape_danger;
pub mod flee_sun;
pub mod follow_mob;
pub mod follow_owner;
pub mod follow_parent;
pub mod go_to_wanted_item;
pub mod goal_selector;
pub mod interact;
pub mod leap_at_target;
pub mod look_around;
pub mod look_at_entity;
pub mod melee_attack;
pub mod move_to_target_pos;
pub mod move_towards_restriction;
pub mod move_towards_target;
pub mod ocelot_attack;
pub mod offer_flower;
pub mod open_door;
pub mod owner_hurt_by_target;
pub mod owner_hurt_target;
pub mod pathfind_to_raid;
pub mod pick_up_block;
pub mod place_block;
pub mod ranged_attack;
pub mod ranged_crossbow_attack;
pub mod restrict_sun;
pub mod revenge;
pub mod run_around_like_crazy;
pub mod sit_when_ordered_to;
pub mod step_and_destroy_block;
pub mod swim;
pub mod teleport_towards_player;
pub mod tempt;
pub(crate) mod track_target;
pub mod trade_with_player;
pub mod try_find_water;
pub mod use_item;
pub mod wander_around;
pub mod water_avoiding_random_flying;
pub mod work_at_job_site;
pub mod zombie_attack;

#[must_use]
pub const fn to_goal_ticks(server_ticks: i32) -> i32 {
    -(-server_ticks).div_euclid(2)
}

pub trait Goal: Send + Sync {
    /// `Goal` 初始应如何启动？
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        false
    }

    /// 启动之后，它应如何继续运行？
    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_start(mob)
    }

    /// 目标开始时调用
    fn start(&mut self, _mob: &dyn Mob) {}

    /// 目标结束时调用
    fn stop(&mut self, _mob: &dyn Mob) {}

    /// 如果 `Goal` 正在运行，每个刻都会调用此方法。
    fn tick(&mut self, _mob: &dyn Mob) {}

    fn should_run_every_tick(&self) -> bool {
        false
    }

    fn can_stop(&self) -> bool {
        true
    }

    fn get_tick_count(&self, ticks: i32) -> i32 {
        if self.should_run_every_tick() {
            ticks
        } else {
            to_goal_ticks(ticks)
        }
    }

    fn controls(&self) -> Controls {
        Controls::empty()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
// 其实我们只用了前 4 位 ;)
pub struct Controls(u8);

impl Controls {
    pub const MOVE: Self = Self(1);
    pub const LOOK: Self = Self(2);
    pub const JUMP: Self = Self(4);
    pub const TARGET: Self = Self(8);

    pub const ITER: [Self; 4] = [Self::MOVE, Self::LOOK, Self::JUMP, Self::TARGET];

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub const fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    pub const fn set(&mut self, control: Self, value: bool) {
        if value {
            self.insert(control);
        } else {
            self.remove(control);
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn get(&self, control: Self) -> bool {
        (self.0 & control.0) != 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn idx(&self) -> usize {
        self.0.trailing_zeros() as usize
    }
}

impl BitOr for Controls {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

pub struct PrioritizedGoal {
    pub goal: Box<dyn Goal>,
    pub running: bool,
    pub priority: u8,
    /// 用于比较同类型的目标。
    /// 始终设置为 `TypeId::of::<G>()`，其中 `G: Goal`。
    type_id: TypeId,
}

impl PrioritizedGoal {
    #[must_use]
    pub fn new(type_id: TypeId, priority: u8, goal: Box<dyn Goal>) -> Self {
        Self {
            goal,
            running: false,
            priority,
            type_id,
        }
    }

    fn can_be_replaced_by(&self, goal: &Self) -> bool {
        self.can_stop() && goal.priority < self.priority
    }
}

impl Goal for PrioritizedGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        self.goal.can_start(mob)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.goal.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        if !self.running {
            self.running = true;
            self.goal.start(mob);
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        if self.running {
            self.running = false;
            self.goal.stop(mob);
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.goal.tick(mob);
    }

    fn should_run_every_tick(&self) -> bool {
        self.goal.should_run_every_tick()
    }

    fn can_stop(&self) -> bool {
        self.goal.can_stop()
    }

    fn get_tick_count(&self, ticks: i32) -> i32 {
        self.goal.get_tick_count(ticks)
    }

    fn controls(&self) -> Controls {
        self.goal.controls()
    }
}

#[derive(Clone)]
pub struct ParentHandle<P> {
    ptr: *const P,
}

impl<P> ParentHandle<P> {
    /// 此包装器允许子结构体持有对其父级的引用
    /// 而不会让代码过于冗长。
    ///
    /// # Safety
    /// - 父节点的存活时间必须长于此句柄。
    /// - 父节点必须位于智能指针内；否则它
    ///   会在内存中移动并导致未定义行为！
    ///
    /// # Example
    /// ```
    /// use papokin::entity::ai::goal::ParentHandle;
    ///
    /// struct Parent {
    ///     child: Child,
    ///     value: i32
    /// }
    ///
    /// struct Child {
    ///     parent: ParentHandle<Parent>,
    /// }
    ///
    /// impl Child {
    ///    fn value(&self) -> i32 {
    ///        self.parent.get().unwrap().value
    ///    }
    /// }
    ///
    /// let mut parent = Box::new(Parent {
    ///     child: Child {parent: ParentHandle::none()},
    ///     value: 7,
    /// });
    /// parent.child.parent = unsafe { ParentHandle::new(&parent) };
    ///
    /// assert_eq!(parent.child.value(), 7);
    /// ```
    pub const unsafe fn new(parent: &P) -> Self {
        Self {
            ptr: ptr::from_ref(parent),
        }
    }

    #[must_use]
    /// 创建一个空句柄（等价于 `Option::None`）。
    // 我们可以用 null 表示 None，因为会在 get 中处理它。
    pub const fn none() -> Self {
        Self { ptr: ptr::null() }
    }

    #[must_use]
    ///若可用，返回对父节点的引用。
    /// 如果未遵守 new 中的 #Safety 规则，将导致未定义行为
    pub const fn get(&self) -> Option<&P> {
        if self.ptr.is_null() {
            None
        } else {
            // SAFETY: `self.ptr` 在 `ParentHandle::new` 中由有效引用初始化，且生命周期超过 `ParentHandle`。
            unsafe { Some(&*self.ptr) }
        }
    }
}

impl<P> Default for ParentHandle<P> {
    fn default() -> Self {
        Self::none()
    }
}

// SAFETY: ParentHandle 存储指向同一 AI 引擎实例内管理的父目标结构的裸指针 `*const P`。
unsafe impl<P> Sync for ParentHandle<P> {}
// SAFETY: ParentHandle 存储指向同一 AI 引擎实例内管理的父目标结构的裸指针 `*const P`。
unsafe impl<P> Send for ParentHandle<P> {}
