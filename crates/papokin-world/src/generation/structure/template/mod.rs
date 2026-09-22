//! NBT 结构模板系统
//!
//! 此模块提供加载与放置 Minecraft 结构
//! 模板（来自 `.nbt` 文件）。这使得精确匹配原版结构成为可能，并
//! 极大简化了雪屋、沉船、村庄等结构的实现。
//!
//! # Architecture
//!
//! - [`StructureTemplate`]：表示已加载的 NBT 模板，含尺寸、调色板与方块
//! - [`TemplatePiece`]：依据模板放置方块的结构部件
//! - [`Rotation`] 与 [`Mirror`]：变换位置与方块属性
//! - [`TemplateCache`]：内嵌模板文件的懒加载缓存
//!
//! # Example Usage
//!
//! ```ignore
//! use papokin_world::generation::structure::template::{TemplateCache, TemplatePiece};
//! use papokin_data::Rotation;
//!
//! // Load a template from the cache
//! let template = TemplateCache::get("igloo/top").expect("Template not found");
//!
//! // Create a piece to place the template
//! let piece = TemplatePiece::new(template, rotation, mirror, position);
//! ```

mod block_state_resolver;
mod cache;
pub mod processor;
mod structure_template;
mod template_piece;

use papokin_data::BlockStateId;
use papokin_data::Mirror;
use papokin_data::Rotation;
use papokin_nbt::{compound::NbtCompound, tag::NbtTag};
use papokin_util::math::vector3::Vector3;
use papokin_util::random::{RandomImpl, hash_block_pos, legacy_rand::LegacyRand};

use crate::ProtoChunk;

pub use block_state_resolver::BlockStateResolver;
pub use cache::{
    TemplateCache, all_embedded_datapack_names, all_pool_names, all_structure_names,
    all_template_names, get_pool_elements, get_processor_list_json, get_template,
    get_template_pool_json, global_cache, has_template, list_template_names, register_template,
    template_bytes,
};
pub use papokin_data::{BlockState, Mirror as BlockMirror, Rotation as BlockRotation};
pub use processor::StructureProcessor;
pub use structure_template::{
    JigsawBlockInfo, Palette, PaletteEntry, SimplePalette, StructureBlockInfo, StructureEntityInfo,
    StructurePlaceSettings, StructureTemplate, TemplateBlock, TemplateEntity, TemplateError,
};
pub use template_piece::TemplatePiece;

/// 方块放置的抽象，由 [`ProtoChunk`]（世界生成）和
/// [`WorldBlockPlacer`]（实际使用的 `/place template` 命令）。
pub trait BlockPlacer {
    fn get_block_state(&self, pos: &Vector3<i32>) -> BlockStateId;
    fn set_block_state(&mut self, pos: &Vector3<i32>, state: &BlockState);
    fn add_block_entity(&mut self, nbt: NbtCompound);
}

/// 在世界原点放置模板，并使用未旋转的 XZ 偏移。
///
/// 所有旋转均在内部处理：
/// - 偏移会被旋转以正确定位模板
/// - 模板内的方块位置会被旋转
/// - 方向性方块属性（facing、axis 等）会被旋转
/// - 方块实体从模板 NBT 数据创建
///
/// `origin` 是基准世界位置 (x, y, z)。
/// `offset` 是相对原点、未经旋转的 XZ 偏移（`x_offset`、`z_offset`）——旋转会自动应用。
#[allow(clippy::too_many_arguments)]
pub fn place_template(
    placer: &mut impl BlockPlacer,
    template: &StructureTemplate,
    origin: Vector3<i32>,
    offset: (i32, i32),
    rotation: Rotation,
    skip_air: bool,
    apply_waterlogging: bool,
    processors: &[StructureProcessor],
    chunk_box: Option<&papokin_util::math::block_box::BlockBox>,
) {
    place_template_with_options(
        placer,
        template,
        origin,
        offset,
        rotation,
        Mirror::None,
        skip_air,
        apply_waterlogging,
        processors,
        chunk_box,
        false,
    );
}

/// 按 `mirror` 变换模板本地方块位置（围绕模板
/// 中心（保持占地不变），然后再按 `rotation`。
const fn transform_local_pos(
    pos: Vector3<i32>,
    size: Vector3<i32>,
    mirror: Mirror,
    rotation: Rotation,
) -> Vector3<i32> {
    let mirrored = match mirror {
        Mirror::None => pos,
        Mirror::LeftRight => Vector3::new(pos.x, pos.y, size.z - 1 - pos.z),
        Mirror::FrontBack => Vector3::new(size.x - 1 - pos.x, pos.y, pos.z),
    };
    rotation.transform_pos(mirrored, size)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn place_template_with_options(
    placer: &mut impl BlockPlacer,
    template: &StructureTemplate,
    origin: Vector3<i32>,
    offset: (i32, i32),
    rotation: Rotation,
    mirror: Mirror,
    skip_air: bool,
    apply_waterlogging: bool,
    processors: &[StructureProcessor],
    chunk_box: Option<&papokin_util::math::block_box::BlockBox>,
    keep_jigsaws: bool,
) {
    let (rotated_ox, rotated_oz) = rotation.rotate_offset(offset.0, offset.1);
    let world_x = origin.x + rotated_ox;
    let world_z = origin.z + rotated_oz;

    let mut context_rng = LegacyRand::from_seed(hash_block_pos(world_x, origin.y, world_z) as u64);
    let mut context = processor::ProcessorContext::new(origin, processors, &mut context_rng);

    for block in &template.blocks {
        let palette_entry = &template.palette[block.state as usize];

        // 结构方块是数据标记。
        if palette_entry.name == "minecraft:structure_block" {
            continue;
        }

        let mut block_entity_nbt = block.nbt.clone();
        let mut placed_entry = palette_entry.clone();

        // 拼图方块在模板处理期间被替换，早于方块实体被
        // 收集。将其保留在放置管线中可避免过时的拼图实体。
        if !keep_jigsaws && palette_entry.name == "minecraft:jigsaw" {
            let final_state = block_entity_nbt
                .as_ref()
                .and_then(|nbt| nbt.get_string("final_state"))
                .unwrap_or("minecraft:air");
            placed_entry = PaletteEntry::from_string(final_state);
            block_entity_nbt = None;
        }

        // 结构空位会保留现有方块，无论是来自调色板本身，还是来自
        // 拼图方块的最终状态。
        if placed_entry.name == "minecraft:structure_void" {
            continue;
        }

        // 解析应用了旋转与镜像到方向属性的方块状态
        let Some(mut state) = BlockStateResolver::resolve(&placed_entry, rotation, mirror) else {
            continue;
        };

        // 在模板边界内变换方块位置（先镜像，再旋转）
        let local_pos = transform_local_pos(block.pos, template.size, mirror, rotation);

        let wx = world_x + local_pos.x;
        let wy = origin.y + local_pos.y;
        let wz = world_z + local_pos.z;

        if let Some(bbox) = chunk_box
            && (wx < bbox.min.x
                || wx > bbox.max.x
                || wy < bbox.min.y
                || wy > bbox.max.y
                || wz < bbox.min.z
                || wz > bbox.max.z)
        {
            continue;
        }

        let world_pos = Vector3::new(wx, wy, wz);

        if apply_waterlogging
            && placer.get_block_state(&world_pos).to_block_id() == papokin_data::Block::WATER.id
            && let Some((_, waterlogged)) = placed_entry
                .properties
                .iter_mut()
                .find(|(name, _)| name == "waterlogged")
        {
            *waterlogged = "true".to_string();
            if let Some(waterlogged_state) =
                BlockStateResolver::resolve(&placed_entry, rotation, mirror)
            {
                state = waterlogged_state;
            }
        }

        // 应用处理器
        let mut should_place = true;
        let mut capped_idx = 0;
        let mut rng =
            LegacyRand::from_seed(hash_block_pos(world_pos.x, world_pos.y, world_pos.z) as u64);
        for processor in processors {
            let Some(processed_state) = processor.process_with_context(
                placer,
                world_pos,
                state,
                &mut block_entity_nbt,
                &mut context,
                &mut capped_idx,
                &mut rng,
            ) else {
                should_place = false;
                break;
            };
            state = processed_state;
        }
        if !should_place {
            continue;
        }
        // 旧版池元素在拼图替换和自定义处理器之后忽略空气。
        if skip_air && state.id.to_block_id() == papokin_data::Block::AIR.id {
            continue;
        }

        placer.set_block_state(&Vector3::new(wx, wy, wz), state);

        // 为交互式方块（熔炉、箱子等）创建方块实体
        let final_block = papokin_data::Block::from_id(state.id.to_block_id());
        let block_entity_id = get_block_entity_id(final_block.name);
        if block_entity_nbt.is_some() || block_entity_id.is_some() {
            let fallback_id = block_entity_id.unwrap_or(final_block.name);
            let mut placed_nbt = NbtCompound::new();

            placed_nbt.put_string("id", fallback_id.to_string());
            placed_nbt.put_int("x", wx);
            placed_nbt.put_int("y", wy);
            placed_nbt.put_int("z", wz);

            if let Some(template_nbt) = &block_entity_nbt {
                for (key, value) in &template_nbt.child_tags {
                    if key.as_ref() != "x" && key.as_ref() != "y" && key.as_ref() != "z" {
                        placed_nbt.child_tags.insert(key.clone(), value.clone());
                    }
                }
            }

            if placed_nbt.get_string("LootTable").is_some()
                && placed_nbt.get_long("LootTableSeed").is_none()
            {
                let mut random = LegacyRand::from_seed(hash_block_pos(wx, wy, wz) as u64);
                placed_nbt.put_long("LootTableSeed", random.next_i64());
            }

            placer.add_block_entity(placed_nbt);
        }
    }
}

pub(crate) fn place_template_entities(
    chunk: &mut ProtoChunk,
    template: &StructureTemplate,
    origin: Vector3<i32>,
    rotation: Rotation,
    chunk_box: &papokin_util::math::block_box::BlockBox,
) {
    for entity in &template.entities {
        let block_pos = rotation.transform_pos(entity.block_pos, template.size);
        let world_block_pos = Vector3::new(
            origin.x + block_pos.x,
            origin.y + block_pos.y,
            origin.z + block_pos.z,
        );
        if !chunk_box.contains_pos(&world_block_pos) {
            continue;
        }

        let pos = match rotation {
            Rotation::None => entity.pos,
            Rotation::Clockwise90 => Vector3::new(
                f64::from(template.size.z) - entity.pos.z,
                entity.pos.y,
                entity.pos.x,
            ),
            Rotation::Rotate180 => Vector3::new(
                f64::from(template.size.x) - entity.pos.x,
                entity.pos.y,
                f64::from(template.size.z) - entity.pos.z,
            ),
            Rotation::CounterClockwise90 => Vector3::new(
                entity.pos.z,
                entity.pos.y,
                f64::from(template.size.x) - entity.pos.x,
            ),
        };
        let mut nbt = entity.nbt.clone();
        nbt.put(
            "Pos",
            NbtTag::List(vec![
                (f64::from(origin.x) + pos.x).into(),
                (f64::from(origin.y) + pos.y).into(),
                (f64::from(origin.z) + pos.z).into(),
            ]),
        );
        nbt.child_tags.remove("UUID");

        if let Some(rotation_nbt) = nbt.get_list("Rotation")
            && rotation_nbt.len() == 2
        {
            let yaw = rotation_nbt[0].extract_float().unwrap_or_default()
                + match rotation {
                    Rotation::None => 0.0,
                    Rotation::Clockwise90 => 90.0,
                    Rotation::Rotate180 => 180.0,
                    Rotation::CounterClockwise90 => 270.0,
                };
            let pitch = rotation_nbt[1].extract_float().unwrap_or_default();
            nbt.put("Rotation", NbtTag::List(vec![yaw.into(), pitch.into()]));
        }

        chunk.add_structure_entity(nbt);
    }
}

///返回需要方块实体的方块的方块实体 ID，若不需要则返回 None。
pub(crate) fn get_block_entity_id(block_name: &str) -> Option<&'static str> {
    let name = block_name.strip_prefix("minecraft:").unwrap_or(block_name);
    match name {
        "furnace" => Some("minecraft:furnace"),
        "chest" => Some("minecraft:chest"),
        "trapped_chest" => Some("minecraft:trapped_chest"),
        "barrel" => Some("minecraft:barrel"),
        "hopper" => Some("minecraft:hopper"),
        "dropper" => Some("minecraft:dropper"),
        "dispenser" => Some("minecraft:dispenser"),
        "brewing_stand" => Some("minecraft:brewing_stand"),
        "blast_furnace" => Some("minecraft:blast_furnace"),
        "smoker" => Some("minecraft:smoker"),
        "shulker_box" => Some("minecraft:shulker_box"),
        "bed" => Some("minecraft:bed"),
        "suspicious_sand" | "suspicious_gravel" => Some("minecraft:brushable_block"),
        "decorated_pot" => Some("minecraft:decorated_pot"),
        "spawner" => Some("minecraft:mob_spawner"),
        "trial_spawner" => Some("minecraft:trial_spawner"),
        "vault" => Some("minecraft:vault"),
        "crafter" => Some("minecraft:crafter"),
        "creaking_heart" => Some("minecraft:creaking_heart"),
        "chiseled_bookshelf" => Some("minecraft:chiseled_bookshelf"),
        "beehive" | "bee_nest" => Some("minecraft:beehive"),
        "campfire" | "soul_campfire" => Some("minecraft:campfire"),
        "sculk_sensor" | "calibrated_sculk_sensor" => Some("minecraft:sculk_sensor"),
        "sculk_catalyst" => Some("minecraft:sculk_catalyst"),
        "sculk_shrieker" => Some("minecraft:sculk_shrieker"),
        "dragon_head"
        | "dragon_wall_head"
        | "skeleton_skull"
        | "skeleton_wall_skull"
        | "wither_skeleton_skull"
        | "wither_skeleton_wall_skull"
        | "zombie_head"
        | "zombie_wall_head"
        | "player_head"
        | "player_wall_head"
        | "creeper_head"
        | "creeper_wall_head"
        | "piglin_head"
        | "piglin_wall_head" => Some("minecraft:skull"),
        "sign" | "oak_sign" | "spruce_sign" | "birch_sign" | "jungle_sign" | "acacia_sign"
        | "dark_oak_sign" | "mangrove_sign" | "cherry_sign" | "bamboo_sign" | "crimson_sign"
        | "warped_sign" | "pale_oak_sign" => Some("minecraft:sign"),
        "hanging_sign"
        | "oak_hanging_sign"
        | "spruce_hanging_sign"
        | "birch_hanging_sign"
        | "jungle_hanging_sign"
        | "acacia_hanging_sign"
        | "dark_oak_hanging_sign"
        | "mangrove_hanging_sign"
        | "cherry_hanging_sign"
        | "bamboo_hanging_sign"
        | "crimson_hanging_sign"
        | "warped_hanging_sign"
        | "pale_oak_hanging_sign" => Some("minecraft:hanging_sign"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::Block;

    struct CollectingPlacer(Vec<BlockStateId>);

    impl BlockPlacer for CollectingPlacer {
        fn get_block_state(&self, _pos: &Vector3<i32>) -> BlockStateId {
            Block::AIR.default_state.id
        }

        fn set_block_state(&mut self, _pos: &Vector3<i32>, state: &BlockState) {
            self.0.push(state.id);
        }

        fn add_block_entity(&mut self, _nbt: NbtCompound) {}
    }

    #[test]
    fn structure_void_is_never_placed() {
        let mut placed_templates = 0;

        for name in all_template_names() {
            let Some(template) = get_template(name) else {
                continue;
            };

            let mut placer = CollectingPlacer(Vec::new());
            place_template(
                &mut placer,
                &template,
                Vector3::new(0, 0, 0),
                (0, 0),
                Rotation::None,
                false,
                false,
                &[],
                None,
            );

            for state_id in &placer.0 {
                assert_ne!(
                    state_id.to_block_id(),
                    Block::STRUCTURE_VOID.id,
                    "{name} placed a structure void"
                );
            }

            placed_templates += 1;
        }

        assert!(placed_templates > 0);
    }

    struct PositionPlacer(Vec<(Vector3<i32>, BlockStateId)>);

    impl BlockPlacer for PositionPlacer {
        fn get_block_state(&self, _pos: &Vector3<i32>) -> BlockStateId {
            Block::AIR.default_state.id
        }

        fn set_block_state(&mut self, pos: &Vector3<i32>, state: &BlockState) {
            self.0.push((*pos, state.id));
        }

        fn add_block_entity(&mut self, _nbt: NbtCompound) {}
    }

    /// 一个 2x1x1 模板：局部坐标 (0,0,0) 处为 `minecraft:stone`，
    /// 位于局部坐标 (1,0,0) 的 `minecraft:gold_block`。
    fn two_block_template() -> StructureTemplate {
        let mut root = NbtCompound::new();
        root.put_list("size", vec![NbtTag::Int(2), NbtTag::Int(1), NbtTag::Int(1)]);

        let mut stone = NbtCompound::new();
        stone.put_string("Name", "minecraft:stone".to_string());
        let mut gold = NbtCompound::new();
        gold.put_string("Name", "minecraft:gold_block".to_string());
        root.put_list("palette", vec![stone.into(), gold.into()]);

        let mut first = NbtCompound::new();
        first.put_list("pos", vec![NbtTag::Int(0), NbtTag::Int(0), NbtTag::Int(0)]);
        first.put_int("state", 0);
        let mut second = NbtCompound::new();
        second.put_list("pos", vec![NbtTag::Int(1), NbtTag::Int(0), NbtTag::Int(0)]);
        second.put_int("state", 1);
        root.put_list("blocks", vec![first.into(), second.into()]);

        StructureTemplate::from_nbt_compound(&root).expect("构建模板失败")
    }

    fn place_two_block_template(
        mirror: Mirror,
        rotation: Rotation,
    ) -> Vec<(Vector3<i32>, BlockStateId)> {
        let template = two_block_template();
        let mut placer = PositionPlacer(Vec::new());
        place_template_with_options(
            &mut placer,
            &template,
            Vector3::new(0, 0, 0),
            (0, 0),
            rotation,
            mirror,
            false,
            false,
            &[],
            None,
            false,
        );
        placer.0.sort_by_key(|(pos, _)| (pos.x, pos.y, pos.z));
        placer.0
    }

    #[test]
    fn mirror_front_back_swaps_positions_within_footprint() {
        let stone = Block::STONE.default_state.id;
        let gold = Block::GOLD_BLOCK.default_state.id;

        // 基线：不镜像时保留模板内局部坐标。
        let placed = place_two_block_template(Mirror::None, Rotation::None);
        assert_eq!(
            placed,
            vec![
                (Vector3::new(0, 0, 0), stone),
                (Vector3::new(1, 0, 0), gold),
            ]
        );

        // FrontBack 围绕模板中心沿 X 轴镜像：其
        // 方块交换位置但仍位于原始占地范围内。
        let placed = place_two_block_template(Mirror::FrontBack, Rotation::None);
        assert_eq!(
            placed,
            vec![
                (Vector3::new(0, 0, 0), gold),
                (Vector3::new(1, 0, 0), stone),
            ]
        );

        // 在此模板上镜像 + 180 度旋转相互抵消，与
        // 与未镜像的放置完全一致。
        let placed = place_two_block_template(Mirror::FrontBack, Rotation::Rotate180);
        assert_eq!(
            placed,
            vec![
                (Vector3::new(0, 0, 0), stone),
                (Vector3::new(1, 0, 0), gold),
            ]
        );
    }
}
