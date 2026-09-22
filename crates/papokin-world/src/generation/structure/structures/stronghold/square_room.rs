use papokin_data::{
    Block, BlockState,
    block_properties::{HorizontalFacing, LadderLikeProperties},
};
use papokin_util::{
    BlockDirection,
    math::block_box::BlockBox,
    random::{RandomGenerator, RandomImpl},
};

use crate::{
    ProtoChunk,
    generation::structure::{
        piece::StructurePieceType,
        structures::{
            StructurePiece, StructurePieceBase, StructurePiecesCollector,
            stronghold::{
                EntranceType, PieceWeight, StoneBrickRandomizer, StrongholdPiece,
                StrongholdPieceType,
            },
        },
    },
};

use crate::world::WorldPortalExt;

/// 带箱子的房间变体中箱子的战利品表，与原版一致
/// `StrongholdPieces$RoomCrossing`。
const CROSSING_LOOT_TABLE: &str = "minecraft:chests/stronghold_crossing";

pub struct SquareRoomPiece {
    pub piece: StrongholdPiece,
    pub room_type: u32,
}

impl SquareRoomPiece {
    pub fn create(
        collector: &mut StructurePiecesCollector,
        random: &mut impl RandomImpl,
        x: i32,
        y: i32,
        z: i32,
        orientation: BlockDirection,
        chain_length: u32,
    ) -> Option<Box<dyn StructurePieceBase>> {
        let bounding_box = BlockBox::rotated(x, y, z, -4, -1, 0, 11, 7, 11, &orientation);

        // 2. 检查世界边界与交集
        if !StrongholdPiece::is_in_bounds(&bounding_box)
            || collector.get_intersecting(&bounding_box).is_some()
        {
            return None;
        }

        // 3. 构造该部件
        let mut piece = StrongholdPiece::new(
            StructurePieceType::StrongholdSquareRoom,
            chain_length,
            bounding_box,
        );
        piece.piece.set_facing(Some(orientation));
        piece.entry_door = EntranceType::get_random(random);

        let room_type = random.next_bounded_i32(5) as u32;

        Some(Box::new(Self { piece, room_type }))
    }
}

impl StructurePieceBase for SquareRoomPiece {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn get_structure_piece(&self) -> &StructurePiece {
        &self.piece.piece
    }

    fn get_structure_piece_mut(&mut self) -> &mut StructurePiece {
        &mut self.piece.piece
    }

    fn fill_openings(
        &self,
        start: &StructurePiece,
        random: &mut RandomGenerator,
        weights: &mut Vec<PieceWeight>,
        last_piece_type: &mut Option<StrongholdPieceType>,
        _has_portal_room: &mut bool,

        collector: &mut StructurePiecesCollector,
        pieces_to_process: &mut Vec<Box<dyn StructurePieceBase>>,
    ) {
        // 1. 向前开口
        // Java：this.fillForwardOpening((Start)start, holder, random, 4, 1);
        self.piece.fill_forward_opening(
            start,
            collector,
            random,
            weights,
            last_piece_type,
            4, // left_right_offset
            1, // height_offset
            pieces_to_process,
            None,
        );

        // 2. 左侧开口（西北）
        // Java：this.fillNWOpening((Start)start, holder, random, 1, 4);
        self.piece.fill_nw_opening(
            start,
            collector,
            random,
            weights,
            last_piece_type,
            1, // height_offset
            4, // left_right_offset
            pieces_to_process,
        );

        // 3. 右侧开口（东南）
        // Java：this.fillSEOpening((Start)start, holder, random, 1, 4);
        self.piece.fill_se_opening(
            start,
            collector,
            random,
            weights,
            last_piece_type,
            1, // height_offset
            4, // left_right_offset
            pieces_to_process,
        );
    }

    fn place(
        &mut self,
        chunk: &mut ProtoChunk,
        _block_registry: &dyn WorldPortalExt,
        random: &mut RandomGenerator,
        _seed: i64,
        chunk_box: &BlockBox,
    ) {
        let randomizer = StoneBrickRandomizer;
        let box_limit = *chunk_box;
        let p = &self.piece;
        let inner = &p.piece;
        let air = Block::AIR.default_state;

        // 1. 主体轮廓（10x6x10）— 与 Java 一致：fillWithOutline(..., 0,0,0, 10,6,10, ...)
        inner.fill_outline_random(
            0,
            0,
            0,
            10,
            6,
            10,
            &randomizer,
            chunk,
            true,
            random,
            &box_limit,
        );

        // 2. 主入口（Z=0，对齐到 X=4）
        // 与 Java 一致：generateEntrance(..., 4, 1, 0)
        p.generate_entrance(chunk, &box_limit, self.piece.entry_door, 4, 1, 0);

        // 3. 清理出口
        // 前方出口（Z=10）-> fillWithOutline(..., 4,1,10, 6,3,10, AIR, AIR, false)
        inner.fill_with_outline(chunk, &box_limit, false, 4, 1, 10, 6, 3, 10, air, air);

        // 左侧出口 (X=0) -> fillWithOutline(..., 0,1,4, 0,3,6, AIR, AIR, false)
        inner.fill_with_outline(chunk, &box_limit, false, 0, 1, 4, 0, 3, 6, air, air);

        // 右侧出口（X=10）-> fillWithOutline(..., 10,1,4, 10,3,6, AIR, AIR, false)
        inner.fill_with_outline(chunk, &box_limit, false, 10, 1, 4, 10, 3, 6, air, air);

        // 4. 房间特定装饰
        match self.room_type {
            0 => {
                let stone_brick = Block::STONE_BRICKS.default_state;
                let smooth_slab = Block::SMOOTH_STONE_SLAB.default_state;

                // 中央柱
                inner.add_block(chunk, stone_brick, 5, 1, 5, &box_limit);
                inner.add_block(chunk, stone_brick, 5, 2, 5, &box_limit);
                inner.add_block(chunk, stone_brick, 5, 3, 5, &box_limit);

                // // 火把
                // inner.add_block(chunk, &Block::WALL_TORCH.default_state.with("facing", "west"), 4, 3, 5, &box_limit);
                // inner.add_block(chunk, &Block::WALL_TORCH.default_state.with("facing", "east"), 6, 3, 5, &box_limit);
                // inner.add_block(chunk, &Block::WALL_TORCH.default_state.with("facing", "south"), 5, 3, 4, &box_limit);
                // inner.add_block(chunk, &Block::WALL_TORCH.default_state.with("facing", "north"), 5, 3, 6, &box_limit);

                // 地板图案（台阶）
                inner.add_block(chunk, smooth_slab, 4, 1, 4, &box_limit);
                inner.add_block(chunk, smooth_slab, 4, 1, 5, &box_limit);
                inner.add_block(chunk, smooth_slab, 4, 1, 6, &box_limit);
                inner.add_block(chunk, smooth_slab, 6, 1, 4, &box_limit);
                inner.add_block(chunk, smooth_slab, 6, 1, 5, &box_limit);
                inner.add_block(chunk, smooth_slab, 6, 1, 6, &box_limit);
                inner.add_block(chunk, smooth_slab, 5, 1, 4, &box_limit);
                inner.add_block(chunk, smooth_slab, 5, 1, 6, &box_limit);
            }
            1 => {
                let stone_brick = Block::STONE_BRICKS.default_state;

                // 砖环
                for i in 0..5 {
                    inner.add_block(chunk, stone_brick, 3, 1, 3 + i, &box_limit);
                    inner.add_block(chunk, stone_brick, 7, 1, 3 + i, &box_limit);
                    inner.add_block(chunk, stone_brick, 3 + i, 1, 3, &box_limit);
                    inner.add_block(chunk, stone_brick, 3 + i, 1, 7, &box_limit);
                }

                // 中央水柱
                inner.add_block(chunk, stone_brick, 5, 1, 5, &box_limit);
                inner.add_block(chunk, stone_brick, 5, 2, 5, &box_limit);
                inner.add_block(chunk, stone_brick, 5, 3, 5, &box_limit);
                inner.add_block(chunk, Block::WATER.default_state, 5, 4, 5, &box_limit);
            }
            2 => {
                let cobble = Block::COBBLESTONE.default_state;

                // 圆石架/墙
                for i in 1..=9 {
                    inner.add_block(chunk, cobble, 1, 3, i, &box_limit);
                    inner.add_block(chunk, cobble, 9, 3, i, &box_limit);
                }
                for i in 1..=9 {
                    inner.add_block(chunk, cobble, i, 3, 1, &box_limit);
                    inner.add_block(chunk, cobble, i, 3, 9, &box_limit);
                }

                // 中央结构支撑
                inner.add_block(chunk, cobble, 5, 1, 4, &box_limit);
                inner.add_block(chunk, cobble, 5, 1, 6, &box_limit);
                inner.add_block(chunk, cobble, 5, 3, 4, &box_limit);
                inner.add_block(chunk, cobble, 5, 3, 6, &box_limit);
                inner.add_block(chunk, cobble, 4, 1, 5, &box_limit);
                inner.add_block(chunk, cobble, 6, 1, 5, &box_limit);
                inner.add_block(chunk, cobble, 4, 3, 5, &box_limit);
                inner.add_block(chunk, cobble, 6, 3, 5, &box_limit);

                // 柱子
                for i in 1..=3 {
                    inner.add_block(chunk, cobble, 4, i, 4, &box_limit);
                    inner.add_block(chunk, cobble, 6, i, 4, &box_limit);
                    inner.add_block(chunk, cobble, 4, i, 6, &box_limit);
                    inner.add_block(chunk, cobble, 6, i, 6, &box_limit);
                }

                inner.add_block(chunk, Block::WALL_TORCH.default_state, 5, 3, 5, &box_limit);

                // 上层地板（橡木木板）
                let oak = Block::OAK_PLANKS.default_state;
                for i in 2..=8 {
                    inner.add_block(chunk, oak, 2, 3, i, &box_limit);
                    inner.add_block(chunk, oak, 3, 3, i, &box_limit);

                    // 梯子/结构的孔洞
                    if i <= 3 || i >= 7 {
                        inner.add_block(chunk, oak, 4, 3, i, &box_limit);
                        inner.add_block(chunk, oak, 5, 3, i, &box_limit);
                        inner.add_block(chunk, oak, 6, 3, i, &box_limit);
                    }
                    inner.add_block(chunk, oak, 7, 3, i, &box_limit);
                    inner.add_block(chunk, oak, 8, 3, i, &box_limit);
                }

                // 梯子
                let mut props = LadderLikeProperties::default(&Block::LADDER);
                props.facing = HorizontalFacing::West;
                let ladder = BlockState::from_id(props.to_state_id(&Block::LADDER));
                inner.add_block(chunk, ladder, 9, 1, 3, &box_limit);
                inner.add_block(chunk, ladder, 9, 2, 3, &box_limit);
                inner.add_block(chunk, ladder, 9, 3, 3, &box_limit);

                // 箱子
                inner.add_chest(chunk, &box_limit, random, 3, 4, 8, CROSSING_LOOT_TABLE);
            }
            _ => {}
        }
    }
}
