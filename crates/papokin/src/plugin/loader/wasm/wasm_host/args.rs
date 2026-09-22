use std::{collections::HashMap, sync::Arc};

use papokin_util::text::TextComponent;

use crate::{
    command::{
        argument_types::{
            argument_type::JavaClientArgumentType, coordinates::Coordinates,
            entity_anchor::EntityAnchor,
        },
        context::command_context::CommandContext,
        node::attached::AttachedNode,
    },
    entity::player::Player,
};
use papokin_util::math::{
    position::BlockPos,
    vector2::Vector2,
    vector3::{Axis, Vector3},
};

#[derive(Clone, Copy, Debug)]
pub enum Number {
    F64(f64),
    F32(f32),
    I32(i32),
    I64(i64),
}

#[derive(Clone, Copy, Debug)]
pub enum NotInBounds {
    LowerBound(Number, Number),
    UpperBound(Number, Number),
}

#[derive(Clone)]
pub enum OwnedArg {
    Entities(Vec<Arc<dyn crate::entity::EntityBase>>),
    Entity(Arc<dyn crate::entity::EntityBase>),
    Players(Vec<Arc<Player>>),
    GameProfiles(Vec<crate::net::GameProfile>),
    BlockPos(papokin_util::math::position::BlockPos),
    Pos3D(papokin_util::math::vector3::Vector3<f64>),
    Pos2D(papokin_util::math::vector2::Vector2<f64>),
    Rotation(f32, bool, f32, bool),
    GameMode(papokin_util::GameMode),
    Difficulty(papokin_util::Difficulty),
    Item(String),
    ItemPredicate(String),
    ResourceLocation(String),
    Block(String),
    BlockPredicate(String),
    BossbarColor(crate::world::bossbar::BossbarColor),
    BossbarStyle(crate::world::bossbar::BossbarDivisions),
    Particle(papokin_data::particle::Particle),
    Msg(String),
    TextComponent(TextComponent),
    Time(i32),
    Num(Result<Number, NotInBounds>),
    Bool(bool),
    Simple(String),
    SoundCategory(papokin_data::sound::SoundCategory),
    DamageType(papokin_data::damage::DamageType),
    Effect(&'static papokin_data::effect::StatusEffect),
    Enchantment(&'static papokin_data::Enchantment),
    EntityAnchor(EntityAnchor),
    Advancement(&'static papokin_data::Advancement),
}

#[must_use]
pub fn build_consumed_args_from_context(context: &CommandContext) -> HashMap<String, OwnedArg> {
    let mut map = HashMap::new();
    for (name, parsed) in &context.arguments {
        let res = &parsed.result;
        if let Some(&b) = res.downcast_ref::<bool>() {
            map.insert(name.clone(), OwnedArg::Bool(b));
        } else if let Some(&i) = res.downcast_ref::<i32>() {
            map.insert(name.clone(), OwnedArg::Num(Ok(Number::I32(i))));
        } else if let Some(&i) = res.downcast_ref::<i64>() {
            map.insert(name.clone(), OwnedArg::Num(Ok(Number::I64(i))));
        } else if let Some(&f) = res.downcast_ref::<f32>() {
            map.insert(name.clone(), OwnedArg::Num(Ok(Number::F32(f))));
        } else if let Some(&f) = res.downcast_ref::<f64>() {
            map.insert(name.clone(), OwnedArg::Num(Ok(Number::F64(f))));
        } else if let Some(s) = res.downcast_ref::<String>() {
            map.insert(name.clone(), OwnedArg::Simple(s.clone()));
        } else if let Some(&pos) = res.downcast_ref::<papokin_util::math::position::BlockPos>() {
            map.insert(name.clone(), OwnedArg::BlockPos(pos));
        } else if let Some(&v) = res.downcast_ref::<papokin_util::math::vector3::Vector3<f64>>() {
            map.insert(name.clone(), OwnedArg::Pos3D(v));
        } else if let Some(&v) = res.downcast_ref::<papokin_util::math::vector2::Vector2<f64>>() {
            map.insert(name.clone(), OwnedArg::Pos2D(v));
        } else if let Some(&mode) = res.downcast_ref::<papokin_util::GameMode>() {
            map.insert(name.clone(), OwnedArg::GameMode(mode));
        } else if let Some(&diff) = res.downcast_ref::<papokin_util::Difficulty>() {
            map.insert(name.clone(), OwnedArg::Difficulty(diff));
        } else if let Some(t) = res.downcast_ref::<TextComponent>() {
            map.insert(name.clone(), OwnedArg::TextComponent(t.clone()));
        } else if let Some(&anchor) = res.downcast_ref::<EntityAnchor>() {
            map.insert(name.clone(), OwnedArg::EntityAnchor(anchor));
        } else if let Some(&coords) = res.downcast_ref::<Coordinates>() {
            // `block_pos`、`column_pos`、`vec2`、`vec3` 和 `rotation` 都会解析
            // 为 `Coordinates`，且原始类型在这里无法被检测到
            if let Some(arg) = coordinates_to_owned_arg(context, name, coords) {
                map.insert(name.clone(), arg);
            }
        } else if let Some(selector) =
            res.downcast_ref::<crate::command::argument_types::entity_selector::EntitySelector>()
        {
            if let Ok(players) = selector.find_players(&context.source) {
                map.insert(name.clone(), OwnedArg::Players(players));
            } else if let Ok(entities) = selector.find_entities(&context.source) {
                map.insert(name.clone(), OwnedArg::Entities(entities));
            }
        }
    }
    map
}

/// 为参数 `name` 声明的 Java 客户端侧解析器
fn declared_parser(context: &CommandContext, name: &str) -> Option<JavaClientArgumentType> {
    context
        .nodes
        .iter()
        .find_map(|parsed| match &context.tree[parsed.node] {
            AttachedNode::Argument(argument) if argument.meta.name == name => {
                Some(argument.meta.argument_type.client_side_parser())
            }
            _ => None,
        })
}

/// 将解析后的 `Coordinates` 转换为与之匹配的 `OwnedArg` 形式
/// 它声明时所用的参数类型
fn coordinates_to_owned_arg(
    context: &CommandContext,
    name: &str,
    coords: Coordinates,
) -> Option<OwnedArg> {
    let source = context.source.as_ref();
    match declared_parser(context, name)? {
        JavaClientArgumentType::BlockPos => {
            let resolved: Vector3<f64> = coords.resolve(source);
            Some(OwnedArg::BlockPos(BlockPos::floored_v(resolved)))
        }
        JavaClientArgumentType::ColumnPos => {
            let resolved: Vector3<f64> = coords.resolve(source);
            let pos = BlockPos::floored_v(resolved);
            Some(OwnedArg::Pos2D(Vector2::new(
                f64::from(pos.0.x),
                f64::from(pos.0.z),
            )))
        }
        JavaClientArgumentType::Vec3 => Some(OwnedArg::Pos3D(coords.resolve(source))),
        JavaClientArgumentType::Vec2 => {
            let resolved: Vector3<f64> = coords.resolve(source);
            Some(OwnedArg::Pos2D(Vector2::new(resolved.x, resolved.z)))
        }
        JavaClientArgumentType::Rotation => {
            let rotation = coords.rotation(source);
            Some(OwnedArg::Rotation(
                rotation.x,
                coords.is_relative(Axis::X),
                rotation.y,
                coords.is_relative(Axis::Y),
            ))
        }
        _ => None,
    }
}

pub struct ConsumedArgsResource {
    pub provider: HashMap<String, OwnedArg>,
}
