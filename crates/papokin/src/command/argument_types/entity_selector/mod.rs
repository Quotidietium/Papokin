mod option;
pub mod parser;

use crate::command::argument_types::entity;
use crate::command::argument_types::entity::ENTITY_SELECTOR_PERMISSION;
use crate::command::argument_types::entity_selector::parser::SELECTORS_NOT_ALLOWED_ERROR_TYPE;
use crate::command::context::command_source::CommandSource;
use crate::command::errors::command_syntax_error::CommandSyntaxError;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::world::World;
use papokin_data::Advancement;
use papokin_data::entity::EntityType;
use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;
use papokin_util::GameMode;
use papokin_util::math::boundingbox::BoundingBox;
use papokin_util::math::bounds::{DoubleBounds, FloatDegreeBounds, IntBounds};
use papokin_util::math::vector3::Vector3;
use papokin_util::math::wrap_degrees;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use uuid::Uuid;

/// 表示一种能够将实体作为目标的结构。
///
/// 此对象表示的选择器并非都以 `@` 开头；它也用于
/// 选择器，例如 *纯玩家名* 和 *裸 UUID*。
pub struct EntitySelector {
    /// 可被选中的最大实体数量。
    pub max_selected: i32,
    /// 此选择器是否包含所有实体而不仅是玩家。
    pub includes_entities: bool,
    /// 一个谓词列表，实体必须全部满足才能成为
    /// 此选择器的值。用于检查游戏模式之类的场景。
    pub predicates: Vec<EntitySelectorPredicate>,
    /// 此选择器允许的距离范围。
    pub distance: Option<DoubleBounds>,
    /// 一个与该选择器的位置提供方式相对应的函数，前提是
    /// 一个初始 [`Vector3<f64>`]。
    pub position_function: PositionFunction,
    /// 此选择器的限定边界框。
    pub bounding_box: Option<BoundingBox>,
    /// 此选择器的排序顺序。
    pub order: Order,
    /// 选择器是否表示当前执行的实体。仅在使用 `@s` 时为 true。
    pub is_current_entity: bool,
    /// 此选择器的限定玩家名称。
    pub player_name: Option<String>,
    /// 此选择器的限定 UUID。
    pub entity_uuid: Option<Uuid>,
    /// 此选择器的限定实体类型或标签。
    pub entity_type: Option<&'static EntityType>,
    /// 此选择器是否使用选择器变量（如 `@p`）。
    pub uses_selector_variable: bool,
    /// 此选择器是否将实体限定在某个世界。
    pub is_world_limited: bool,
}

impl EntitySelector {
    ///若 [`CommandSource`] 没有使用该命令的权限，则返回 [`Err`]
    /// 此实体选择器。
    pub fn check_permissions(&self, source: &CommandSource) -> Result<(), CommandSyntaxError> {
        if self.uses_selector_variable && !source.has_permission(ENTITY_SELECTOR_PERMISSION) {
            Err(SELECTORS_NOT_ALLOWED_ERROR_TYPE.create_without_context())
        } else {
            Ok(())
        }
    }

    const fn result_limit(&self) -> usize {
        if matches!(self.order, Order::Arbitrary) {
            self.max_selected as usize
        } else {
            usize::MAX
        }
    }

    /// 尝试查找此选择器所表示的单个实体。
    pub fn find_single_entity(
        &self,
        source: &CommandSource,
    ) -> Result<Arc<dyn EntityBase>, CommandSyntaxError> {
        let list = self.find_entities(source)?;
        match list.as_slice() {
            [] => Err(entity::NO_ENTITIES_ERROR_TYPE.create_without_context()),
            [entity] => Ok(entity.clone()),
            _ => Err(entity::NOT_SINGLE_ENTITY_ERROR_TYPE.create_without_context()),
        }
    }

    /// 尝试查找此选择器所表示的所有实体。如果未找到任何实体，仍会返回空的 `Vec`。
    pub fn find_entities(
        &self,
        source: &CommandSource,
    ) -> Result<Vec<Arc<dyn EntityBase>>, CommandSyntaxError> {
        self.check_permissions(source)?;
        if !self.includes_entities {
            self.find_players(source)
                .map(|v| v.into_iter().map(|p| p as Arc<dyn EntityBase>).collect())
        } else if let Some(name) = self.player_name.as_ref() {
            // 尝试通过名称获取玩家。
            let player = source
                .server
                .as_ref()
                .and_then(|s| s.get_player_by_name(name));
            Ok(player.map_or_else(Vec::new, |p| vec![p as Arc<dyn EntityBase>]))
        } else if let Some(uuid) = self.entity_uuid.as_ref() {
            // 尝试通过 UUID 获取实体。
            for world in source.server().worlds.load().iter() {
                if let Some(entity) = world.get_entity_by_uuid(*uuid) {
                    return Ok(vec![entity]);
                }
            }
            Ok(Vec::new())
        } else {
            let origin = self.position_function.apply(source.position);
            let bounding_box = self.absolute_bounding_box(origin);
            let predicate = self.predicate(origin, bounding_box);
            if self.is_current_entity {
                Ok(source
                    .entity
                    .as_ref()
                    .filter(|p| predicate.test(p.as_ref()))
                    .map_or_else(Vec::new, |p| vec![p.clone()]))
            } else {
                let mut list = Vec::new();
                if self.is_world_limited {
                    self.add_entities(&mut list, source.world().as_ref(), bounding_box, &predicate);
                } else {
                    for world in source.server().worlds.load().iter() {
                        self.add_entities(&mut list, world, bounding_box, &predicate);
                    }
                }

                Ok(self.sort_and_limit(origin, list))
            }
        }
    }

    fn add_entities(
        &self,
        list: &mut Vec<Arc<dyn EntityBase>>,
        world: &World,
        bounding_box: Option<BoundingBox>,
        predicate: &EntitySelectorPredicate,
    ) {
        let limit = self.result_limit();
        if let Some(b) = bounding_box {
            world.extend_entities_in_box_where(list, limit, b, |e| predicate.test(e));
        } else {
            world.extend_entities_where(list, limit, |e| predicate.test(e));
        }
    }

    /// 尝试查找此选择器所表示的单个玩家。
    pub fn find_single_player(
        &self,
        source: &CommandSource,
    ) -> Result<Arc<Player>, CommandSyntaxError> {
        let mut list = self.find_players(source)?;
        match list.len() {
            0 => Err(entity::NO_PLAYERS_ERROR_TYPE.create_without_context()),
            1 => Ok(list.pop().unwrap()),
            _ => Err(entity::NOT_SINGLE_PLAYER_ERROR_TYPE.create_without_context()),
        }
    }

    /// 尝试查找此选择器所表示的所有玩家。
    pub fn find_players(
        &self,
        source: &CommandSource,
    ) -> Result<Vec<Arc<Player>>, CommandSyntaxError> {
        self.check_permissions(source)?;
        if let Some(name) = self.player_name.as_ref() {
            // 尝试通过名称获取玩家。
            let player = source
                .server
                .as_ref()
                .and_then(|s| s.get_player_by_name(name));
            Ok(player.map_or_else(Vec::new, |p| vec![p]))
        } else if let Some(uuid) = self.entity_uuid.as_ref() {
            // 尝试通过 UUID 获取实体。
            for world in source.server().worlds.load().iter() {
                if let Some(player) = world.get_player_by_uuid(*uuid) {
                    return Ok(vec![player]);
                }
            }
            Ok(Vec::new())
        } else {
            let origin = self.position_function.apply(source.position);
            let bounding_box = self.absolute_bounding_box(origin);
            let predicate = self.predicate(origin, bounding_box);
            if self.is_current_entity {
                Ok(source
                    .entity
                    .as_ref()
                    .and_then(|e| {
                        source
                            .server()
                            .get_player_by_uuid(e.get_entity().entity_uuid)
                    })
                    .filter(|p| predicate.test(p.as_ref()))
                    .map_or_else(Vec::new, |p| vec![p]))
            } else {
                let limit = self.result_limit();
                let mut list = Vec::new();
                if limit > 0 {
                    if self.is_world_limited {
                        Self::add_players_from_world(source.world(), &mut list, &predicate, limit);
                    } else {
                        for world in source.server().worlds.load().iter() {
                            Self::add_players_from_world(
                                world.as_ref(),
                                &mut list,
                                &predicate,
                                limit,
                            );
                        }
                    }
                }

                Ok(self.sort_and_limit(origin, list))
            }
        }
    }

    fn add_players_from_world(
        world: &World,
        list: &mut Vec<Arc<Player>>,
        predicate: &EntitySelectorPredicate,
        limit: usize,
    ) {
        for player in world.players.load().iter() {
            if predicate.test(player.as_ref()) {
                list.push(player.clone());
                if list.len() >= limit {
                    return;
                }
            }
        }
    }

    #[must_use]
    pub fn absolute_bounding_box(&self, pos: Vector3<f64>) -> Option<BoundingBox> {
        self.bounding_box.map(|b| b.shift(pos))
    }

    ///返回一个用于对实体进行测试的 [`EntitySelectorPredicate`]。
    #[must_use]
    fn predicate(
        &self,
        pos: Vector3<f64>,
        bounding_box: Option<BoundingBox>,
    ) -> EntitySelectorPredicate {
        let mut list = self.predicates.clone();

        if let Some(bounding_box) = bounding_box {
            list.push(EntitySelectorPredicate::BoundingBox(bounding_box));
        }
        if let Some(distance_bounds) = self.distance {
            list.push(EntitySelectorPredicate::Distance(distance_bounds, pos));
        }

        EntitySelectorPredicate::new_all_of(list)
    }

    /// 依照此选择器的排序方式（默认为 [`Order::Arbitrary`]）对提供的实体进行排序
    /// 并根据此选择器的限制来约束条目数量。
    fn sort_and_limit<T: EntityBase + ?Sized>(
        &self,
        origin: Vector3<f64>,
        entities: Vec<Arc<T>>,
    ) -> Vec<Arc<T>> {
        self.order.sort_and_limit(
            entities.len().min(self.max_selected as usize),
            origin,
            entities,
        )
    }
}

/// 一个可能操作也可能不操作所提供位置的函数，供解析器使用。
pub enum PositionFunction {
    /// 一个不影响位置、直接将其返回的函数。
    Identity,
    /// 一个可能会覆盖某个位置一个或多个坐标的函数，具体取决于
    /// 取决于解析的位置。
    ///
    /// 若已设置解析器的位置坐标，则所提供位置的
    /// 相应坐标会被替换，并返回新位置。
    OverrideWithParser(Vector3<Option<f64>>),
}

impl PositionFunction {
    fn apply(&self, pos: Vector3<f64>) -> Vector3<f64> {
        match self {
            Self::Identity => pos,
            Self::OverrideWithParser(function_pos) => Vector3::new(
                function_pos.x.unwrap_or(pos.x),
                function_pos.y.unwrap_or(pos.y),
                function_pos.z.unwrap_or(pos.z),
            ),
        }
    }
}

/// 实体选择器选择实体时可采用的排序方式。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Order {
    Nearest,
    Furthest,
    Random,
    Arbitrary,
}

impl Order {
    fn sort_with_comparator_usage_function<T: EntityBase + ?Sized>(
        mut entities: Vec<Arc<T>>,
        f: impl Fn(&Arc<T>, &Arc<T>) -> std::cmp::Ordering,
    ) -> Vec<Arc<T>> {
        entities.sort_by(f);
        entities
    }

    fn distance_comparator<T: EntityBase + ?Sized>(
        origin: &Vector3<f64>,
        a: &Arc<T>,
        b: &Arc<T>,
    ) -> std::cmp::Ordering {
        a.get_entity()
            .pos
            .load()
            .squared_distance_to_vec(origin)
            .total_cmp(&b.get_entity().pos.load().squared_distance_to_vec(origin))
    }

    #[must_use]
    pub fn sort_and_limit<T: EntityBase + ?Sized>(
        &self,
        limit: usize,
        origin: Vector3<f64>,
        mut entities: Vec<Arc<T>>,
    ) -> Vec<Arc<T>> {
        match self {
            Self::Nearest => Self::sort_with_comparator_usage_function(entities, |a, b| {
                Self::distance_comparator(&origin, a, b)
            }),
            Self::Furthest => Self::sort_with_comparator_usage_function(entities, |a, b| {
                Self::distance_comparator(&origin, b, a)
            }),
            Self::Random => {
                let mut rng = rand::rng();
                entities.shuffle(&mut rng);
                entities
            }
            Self::Arbitrary => entities,
        }
        .into_iter()
        .take(limit)
        .collect()
    }
}

/// 用于实体选择器的谓词。
#[derive(Debug, Clone)]
pub enum EntitySelectorPredicate {
    /// 用于检查实体是否存活的谓词。
    IsAlive,
    /// 用于检查玩家游戏模式的谓词。此检查也可以取反。
    GameMode(GameMode, bool),
    /// 用于检查实体经验等级（如果有）的谓词。
    ExperienceLevel(IntBounds),
    /// 用于检查实体旋转坐标的谓词。
    Rotation(FloatDegreeBounds, RotationType),
    /// 用于检查实体是否与边界框相交的谓词。
    BoundingBox(BoundingBox),
    /// 用于检查实体是否处于距某一位置的指定范围内的谓词。
    Distance(DoubleBounds, Vector3<f64>),
    /// 用于检查实体类型的谓词。此检查也可以取反。
    EntityType(&'static EntityType, bool),
    /// 用于检查实体名称（自定义名称或档案名）的谓词。
    Name(String, bool),
    /// 用于检查实体记分板标签的谓词。
    Tag(String, bool),
    /// 用于检查实体所属队伍的谓词。
    Team(String, bool),
    /// 用于检查实体分数的谓词。
    Scores(HashMap<String, IntBounds>),
    /// 用于检查玩家进度的谓词。
    Advancements(HashMap<String, bool>),
    /// 用于检查实体原始 NBT 数据的谓词。
    Nbt(NbtCompound, bool),
    /// 用于检查战利品表条件/谓词的谓词。
    Predicate(String, bool),

    /// 用于组合子谓词。
    AllOf(Vec<Self>),
}

#[derive(Debug, Clone, Copy)]
pub enum RotationType {
    Yaw,
    Pitch,
}

impl RotationType {
    /// 返回与此旋转类型对应的旋转值
    /// 所提供实体的。
    pub fn value_from_entity(self, entity: &dyn EntityBase) -> f32 {
        match self {
            Self::Yaw => entity.get_entity().yaw.load(),
            Self::Pitch => entity.get_entity().pitch.load(),
        }
    }
}

fn matches_nbt(expected: &NbtTag, actual: &NbtTag) -> bool {
    match (expected, actual) {
        (NbtTag::Compound(expected_comp), NbtTag::Compound(actual_comp)) => {
            for (key, expected_val) in &expected_comp.child_tags {
                if let Some(actual_val) = actual_comp.child_tags.get(key) {
                    if !matches_nbt(expected_val, actual_val) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        }
        (NbtTag::List(expected_list), NbtTag::List(actual_list)) => {
            for expected_val in expected_list {
                if !actual_list
                    .iter()
                    .any(|actual_val| matches_nbt(expected_val, actual_val))
                {
                    return false;
                }
            }
            true
        }
        (a, b) => a == b,
    }
}

fn matches_nbt_compound(expected: &NbtCompound, actual: &NbtCompound) -> bool {
    for (key, expected_val) in &expected.child_tags {
        if let Some(actual_val) = actual.child_tags.get(key) {
            if !matches_nbt(expected_val, actual_val) {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

#[allow(clippy::option_if_let_else)]
fn entity_actual_name(entity: &dyn EntityBase) -> String {
    if let Some(player) = entity.get_player() {
        player.gameprofile.name.clone()
    } else if let Some(custom_name) = &**entity.get_entity().custom_name.load() {
        custom_name.clone().get_text()
    } else {
        entity.get_entity().entity_type.resource_name.to_string()
    }
}

impl EntitySelectorPredicate {
    #[must_use]
    pub const fn new_all_of(predicates: Vec<Self>) -> Self {
        Self::AllOf(predicates)
    }

    #[allow(clippy::too_many_lines)]
    pub fn test(&self, entity: &dyn EntityBase) -> bool {
        match self {
            Self::IsAlive => entity.get_entity().is_alive(),
            Self::GameMode(mode, invert) => entity
                .get_player()
                .is_some_and(|p| (p.gamemode.load() == *mode) ^ invert),
            Self::ExperienceLevel(bounds) => entity
                .get_player()
                .is_some_and(|p| bounds.matches(p.experience_level.load(Ordering::Relaxed))),
            Self::Rotation(bounds, f) => {
                let min = wrap_degrees(bounds.min().unwrap_or(0.0f32));
                let max = wrap_degrees(bounds.max().unwrap_or(360.0f32));
                let degrees = wrap_degrees(f.value_from_entity(entity));
                if min > max {
                    degrees >= min || degrees <= max
                } else {
                    degrees >= min && degrees <= max
                }
            }
            Self::BoundingBox(bounding_box) => entity
                .get_entity()
                .bounding_box
                .load()
                .intersects(bounding_box),
            Self::Distance(bounds, pos) => {
                bounds.matches_square(entity.get_entity().pos.load().squared_distance_to_vec(pos))
            }
            Self::EntityType(expected_type, invert) => {
                let actual_type = entity.get_entity().entity_type;
                (actual_type.id == expected_type.id) ^ invert
            }
            Self::Name(expected_name, invert) => {
                let actual_name = entity_actual_name(entity);
                (actual_name == *expected_name) ^ invert
            }
            Self::Tag(expected_tag, invert) => {
                let has_tag = entity
                    .get_entity()
                    .scoreboard_tags
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains(expected_tag);
                has_tag ^ invert
            }
            Self::Team(expected_team, invert) => {
                let actual_name = entity_actual_name(entity);
                let world = entity.get_entity().world.load();
                let scoreboard = world
                    .scoreboard
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let has_team = scoreboard.get_teams().iter().any(|(name, team)| {
                    name == expected_team && team.players.contains(&actual_name)
                });
                has_team ^ invert
            }
            Self::Scores(scores_map) => {
                let actual_name = entity_actual_name(entity);
                let world = entity.get_entity().world.load();
                let scoreboard = world
                    .scoreboard
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let entity_scores = scoreboard.get_scores().get(&actual_name);
                for (objective, bounds) in scores_map {
                    let score_val = entity_scores
                        .and_then(|obj_map| obj_map.get(objective))
                        .map_or(0, |score| score.value.0);
                    if !bounds.matches(score_val) {
                        return false;
                    }
                }
                true
            }
            Self::Advancements(advancements_map) => {
                let Some(player) = entity.get_player() else {
                    return false;
                };
                for (adv_id, expected_done) in advancements_map {
                    if let Some(advancement) = Advancement::from_name(adv_id) {
                        let is_done = player.has_advancement(advancement);
                        if is_done != *expected_done {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            }
            Self::Nbt(expected_nbt, invert) => {
                let mut actual_nbt = NbtCompound::default();
                entity.write_nbt(&mut actual_nbt);
                matches_nbt_compound(expected_nbt, &actual_nbt) ^ invert
            }
            Self::Predicate(_predicate_id, invert) => {
                // 由于战利品表谓词尚未完全实现，我们默认为 false（或 true ^ invert）
                false ^ invert
            }
            Self::AllOf(predicates) => predicates.iter().all(|predicate| predicate.test(entity)),
        }
    }
}
