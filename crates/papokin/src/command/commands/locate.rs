use std::borrow::Cow;

use papokin_data::biome::Biome;
use papokin_data::structures::{StructureKeys, StructurePlacementType, StructureSet};
use papokin_data::tag::{self, RegistryKey};
use papokin_data::translation;
use papokin_util::PermissionLvl;
use papokin_util::math::position::BlockPos;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::click::ClickEvent;
use papokin_util::text::hover::HoverEvent;
use papokin_util::text::{TextComponent, color::NamedColor};
use papokin_world::generation::generator::biome_finder::find_closest_biome_3d;
use papokin_world::generation::generator::structure_finder::{
    find_nearest_structure, find_nearest_structure_start,
};
use rustc_hash::FxHashSet;

use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::resource_key::BIOME_REGISTRY;
use crate::command::argument_types::resource_or_tag::{
    POI_REGISTRY, ResourceOrTag, ResourceOrTagArgument, ResourceOrTagKeyArgument,
    STRUCTURE_REGISTRY,
};
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};

const DESCRIPTION: &str = "定位最近的结构、生物群系或兴趣点。";

const PERMISSION: &str = "minecraft:command.locate";

const ARG_STRUCTURE: &str = "structure";
const ARG_BIOME: &str = "biome";
const ARG_POI: &str = "poi";

/// 以区块区域为单位的最大结构搜索半径，对应原版的
/// `LocateCommand` 中的 `findNearestMapStructure` 调用。
const STRUCTURE_SEARCH_RADIUS: i32 = 100;

/// 来自原版 `LocateCommand` 的生物群系搜索参数：6400 方块的
/// 半径，水平方向每 32 格、垂直方向每 64 格探测一次。
const BIOME_SEARCH_RADIUS: i32 = 6400;
const BIOME_SEARCH_HORIZONTAL_STEP: i32 = 32;
const BIOME_SEARCH_VERTICAL_STEP: i32 = 64;

/// POI 搜索半径（以方块为单位），与原版 `LocateCommand` 一致。
const POI_SEARCH_RADIUS: i32 = 256;

static STRUCTURE_INVALID_ERROR_TYPE: CommandErrorType<1> =
    CommandErrorType::new(translation::java::COMMANDS_LOCATE_STRUCTURE_INVALID);

static STRUCTURE_NOT_FOUND_ERROR_TYPE: CommandErrorType<1> =
    CommandErrorType::new(translation::java::COMMANDS_LOCATE_STRUCTURE_NOT_FOUND);

static BIOME_NOT_FOUND_ERROR_TYPE: CommandErrorType<1> =
    CommandErrorType::new(translation::java::COMMANDS_LOCATE_BIOME_NOT_FOUND);

static POI_NOT_FOUND_ERROR_TYPE: CommandErrorType<1> =
    CommandErrorType::new(translation::java::COMMANDS_LOCATE_POI_NOT_FOUND);

/// 构建可点击的绿色 `[x, ~, z]`（`absolute_y` 时为 `[x, y, z]`）
/// 坐标组件，原版的 locate 反馈会用到它。
fn coordinates_text(pos: &BlockPos, absolute_y: bool) -> TextComponent {
    let x = pos.0.x;
    let z = pos.0.z;
    let y = if absolute_y {
        pos.0.y.to_string()
    } else {
        "~".to_string()
    };

    TextComponent::translate(
        translation::java::CHAT_COORDINATES,
        [
            TextComponent::text(x.to_string()),
            TextComponent::text(y.clone()),
            TextComponent::text(z.to_string()),
        ],
    )
    .color_named(NamedColor::Green)
    .click_event(ClickEvent::SuggestCommand {
        command: Cow::from(format!("/tp @s {x} {y} {z}")),
    })
    .hover_event(HoverEvent::show_text(TextComponent::translate(
        translation::java::CHAT_COORDINATES_TOOLTIP,
        [],
    )))
}

/// 成功消息的第一个参数：所搜索的 ID，带有
/// 为标签搜索附加具体找到的条目，如同原版的
/// `LocateCommand.showLocateResult`。
fn result_name(searched: &ResourceOrTag, found: &str) -> String {
    match searched {
        ResourceOrTag::Resource(_) => searched.printable(),
        ResourceOrTag::Tag(_) => format!("{} ({found})", searched.printable()),
    }
}

/// 原版对结构和兴趣点（POI）报告水平方块距离。
fn horizontal_distance(origin: &BlockPos, target: &BlockPos) -> i32 {
    let dx = f64::from(target.0.x - origin.0.x);
    let dz = f64::from(target.0.z - origin.0.z);
    dx.hypot(dz).floor().max(0.0) as i32
}

/// ... 而生物群系则使用完整的三维方块距离。
fn absolute_distance(origin: &BlockPos, target: &BlockPos) -> i32 {
    let dx = f64::from(target.0.x - origin.0.x);
    let dy = f64::from(target.0.y - origin.0.y);
    let dz = f64::from(target.0.z - origin.0.z);
    (dx * dx + dy * dy + dz * dz).sqrt().floor().max(0.0) as i32
}

fn send_success(
    context: &CommandContext<'_>,
    key: &'static str,
    name: String,
    target: &BlockPos,
    absolute_y: bool,
    distance: i32,
) {
    context.source.send_feedback(
        TextComponent::translate(
            key,
            [
                TextComponent::text(name),
                coordinates_text(target, absolute_y),
                TextComponent::text(distance.to_string()),
            ],
        ),
        false,
    );
}

struct LocateStructureExecutor;

impl CommandExecutor for LocateStructureExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let searched = context.get_argument::<ResourceOrTag>(ARG_STRUCTURE)?;

        // 生成器的放置数据模拟了原版的结构集，
        // 因此 id 会以这些为基准解析。不存在结构标签数据，
        // 因此这些标签尚无法指名任何已知结构（暂时）。
        let set = if let ResourceOrTag::Resource(id) = searched
            && id.is_vanilla()
        {
            StructureSet::get(id.path())
        } else {
            None
        };

        let Some(set) = set else {
            return Err(STRUCTURE_INVALID_ERROR_TYPE
                .create_without_context(TextComponent::text(searched.printable())));
        };

        let origin = BlockPos::floored_v(context.source.position);

        let world = context.source.world();
        let seed = world.level.seed.0;
        let world_gen = world.level.world_gen.load_full();

        let found = match &set.placement.placement_type {
            // 要塞来自预先计算的环带缓存，该缓存
            // 已持有它们实际占据的位置。
            StructurePlacementType::ConcentricRings(_) => {
                world_gen.global_structure_cache().and_then(|global_cache| {
                    find_nearest_structure(
                        origin,
                        &[&set.placement],
                        STRUCTURE_SEARCH_RADIUS,
                        seed as i64,
                        global_cache,
                    )
                })
            }
            // 其余部分散布在一个网格上，其候选区块
            // 仅是*可能的*位置：候选处的生物群系仍可能
            // 拒绝集合中的每个结构。解析起点能让
            // 确保所报告的位置上确实存在一个。
            StructurePlacementType::RandomSpread(_) => {
                let targets: Vec<StructureKeys> =
                    set.structures.iter().map(|entry| entry.structure).collect();
                find_nearest_structure_start(
                    origin,
                    set,
                    &targets,
                    STRUCTURE_SEARCH_RADIUS,
                    &world_gen,
                )
            }
        };

        let Some(target) = found else {
            return Err(STRUCTURE_NOT_FOUND_ERROR_TYPE
                .create_without_context(TextComponent::text(searched.printable())));
        };

        // 通知插件，并让它们重写定位到的位置。
        let structure_id = match &searched {
            ResourceOrTag::Resource(id) => id.to_string(),
            ResourceOrTag::Tag(_) => searched.printable(),
        };
        let mut event =
            crate::plugin::api::events::world::structures_locate::StructuresLocateEvent::new(
                world.clone(),
                origin,
                structure_id,
                STRUCTURE_SEARCH_RADIUS,
                vec![target],
            );
        let server = context.source.server();
        server.plugin_manager.fire_blocking(server, &mut event);

        let Some(&target) = event.results.first() else {
            return Err(STRUCTURE_NOT_FOUND_ERROR_TYPE
                .create_without_context(TextComponent::text(searched.printable())));
        };

        let distance = horizontal_distance(&origin, &target);
        send_success(
            context,
            translation::java::COMMANDS_LOCATE_STRUCTURE_SUCCESS,
            searched.printable(),
            &target,
            false,
            distance,
        );

        Ok(distance)
    }
}

struct LocateBiomeExecutor;

impl CommandExecutor for LocateBiomeExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let searched = context.get_argument::<ResourceOrTag>(ARG_BIOME)?.clone();

        let targets: FxHashSet<u8> = match &searched {
            ResourceOrTag::Resource(id) => Biome::from_name(id.path())
                .map(|biome| biome.id)
                .into_iter()
                .collect(),
            ResourceOrTag::Tag(id) => {
                tag::get_tag_values(RegistryKey::WorldgenBiome, &id.to_string())
                    .into_iter()
                    .flatten()
                    .filter_map(|name| Biome::from_name(name))
                    .map(|biome| biome.id)
                    .collect()
            }
        };

        let not_found = || {
            BIOME_NOT_FOUND_ERROR_TYPE
                .create_without_context(TextComponent::text(searched.printable()))
        };
        if targets.is_empty() {
            return Err(not_found());
        }

        let origin = BlockPos::floored_v(context.source.position);
        let world = context.source.world().clone();
        let world_gen = world.level.world_gen.load_full();

        let found = find_closest_biome_3d(
            &world_gen,
            origin,
            &targets,
            BIOME_SEARCH_RADIUS,
            BIOME_SEARCH_HORIZONTAL_STEP,
            BIOME_SEARCH_VERTICAL_STEP,
        );

        let Some((target, biome)) = found else {
            return Err(not_found());
        };

        let distance = absolute_distance(&origin, &target);
        send_success(
            context,
            translation::java::COMMANDS_LOCATE_BIOME_SUCCESS,
            result_name(&searched, &format!("minecraft:{}", biome.registry_id)),
            &target,
            true,
            distance,
        );

        Ok(distance)
    }
}

struct LocatePoiExecutor;

impl CommandExecutor for LocatePoiExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let searched = context.get_argument::<ResourceOrTag>(ARG_POI)?;

        // POI 条目存储带命名空间的类型 ID，标签数据使用裸
        // 原版名称；将所有内容规范化为 `namespace:path`。
        let targets: FxHashSet<String> = match searched {
            ResourceOrTag::Resource(id) => std::iter::once(id.to_string()).collect(),
            ResourceOrTag::Tag(id) => {
                tag::get_tag_values(RegistryKey::PointOfInterestType, &id.to_string())
                    .into_iter()
                    .flatten()
                    .map(|name| {
                        if name.contains(':') {
                            (*name).to_string()
                        } else {
                            format!("minecraft:{name}")
                        }
                    })
                    .collect()
            }
        };

        let origin = BlockPos::floored_v(context.source.position);
        let world = context.source.world().clone();

        let found = {
            let mut poi_storage = world
                .portal_poi
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            poi_storage.find_closest_matching(origin, POI_SEARCH_RADIUS, |poi_type| {
                targets.contains(poi_type)
            })
        };

        let Some((target, poi_type)) = found else {
            return Err(POI_NOT_FOUND_ERROR_TYPE
                .create_without_context(TextComponent::text(searched.printable())));
        };

        let distance = horizontal_distance(&origin, &target);
        send_success(
            context,
            translation::java::COMMANDS_LOCATE_POI_SUCCESS,
            result_name(searched, &poi_type),
            &target,
            false,
            distance,
        );

        Ok(distance)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("locate", DESCRIPTION)
            .requires(PERMISSION)
            .then(
                literal("structure").then(
                    argument(
                        ARG_STRUCTURE,
                        ResourceOrTagKeyArgument(STRUCTURE_REGISTRY.clone()),
                    )
                    .executes(LocateStructureExecutor),
                ),
            )
            .then(
                literal("biome").then(
                    argument(ARG_BIOME, ResourceOrTagArgument(BIOME_REGISTRY.clone()))
                        .executes(LocateBiomeExecutor),
                ),
            )
            .then(
                literal("poi").then(
                    argument(ARG_POI, ResourceOrTagArgument(POI_REGISTRY.clone()))
                        .executes(LocatePoiExecutor),
                ),
            ),
    );
}
