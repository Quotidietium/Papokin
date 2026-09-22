use papokin_data::{Block, BlockState, block_properties::OakFenceLikeProperties};
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

pub struct PrisonHallPiece {
    piece: StrongholdPiece,
}

impl PrisonHallPiece {
    pub fn create(
        collector: &mut StructurePiecesCollector,
        random: &mut impl RandomImpl,
        x: i32,
        y: i32,
        z: i32,
        orientation: BlockDirection,
        chain_length: u32,
    ) -> Option<Box<dyn StructurePieceBase>> {
        // 主体轮廓 9x5x11 (0..8, 0..4, 0..10)
        let bounding_box = BlockBox::rotated(x, y, z, -1, -1, 0, 9, 5, 11, &orientation);

        if !StrongholdPiece::is_in_bounds(&bounding_box)
            || collector.get_intersecting(&bounding_box).is_some()
        {
            return None;
        }

        let mut piece = StrongholdPiece::new(
            StructurePieceType::StrongholdPrisonHall,
            chain_length,
            bounding_box,
        );
        piece.piece.set_facing(Some(orientation));
        piece.entry_door = EntranceType::get_random(random);

        Some(Box::new(Self { piece }))
    }
}

impl StructurePieceBase for PrisonHallPiece {
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
        // 监狱大厅通常只有一个向前的出口
        self.piece.fill_forward_opening(
            start,
            collector,
            random,
            weights,
            last_piece_type,
            1,
            1,
            pieces_to_process,
            None,
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

        // 1. 主体外壳 9x5x11
        inner.fill_outline_random(
            0,
            0,
            0,
            8,
            4,
            10,
            &randomizer,
            chunk,
            true,
            random,
            &box_limit,
        );

        // 2. 入口（Z=0）
        p.generate_entrance(chunk, &box_limit, self.piece.entry_door, 1, 1, 0);

        // 3. 出口（前方/Z=10）— 使用 fill_with_outline（cant_replace_air = false）
        inner.fill_with_outline(chunk, &box_limit, false, 1, 1, 10, 3, 3, 10, air, air);

        // 4. 囚室石质隔墙（X=4）
        // Note: Java 在此为 cant_replace_air 传入 'false'
        inner.fill_outline_random(
            4,
            1,
            1,
            4,
            3,
            1,
            &randomizer,
            chunk,
            false,
            random,
            &box_limit,
        );
        inner.fill_outline_random(
            4,
            1,
            3,
            4,
            3,
            3,
            &randomizer,
            chunk,
            false,
            random,
            &box_limit,
        );
        inner.fill_outline_random(
            4,
            1,
            7,
            4,
            3,
            7,
            &randomizer,
            chunk,
            false,
            random,
            &box_limit,
        );
        inner.fill_outline_random(
            4,
            1,
            9,
            4,
            3,
            9,
            &randomizer,
            chunk,
            false,
            random,
            &box_limit,
        );

        // 5. 带方向属性的铁栏杆
        let mut props = OakFenceLikeProperties::default(&Block::IRON_BARS);

        // 南北朝向的铁栏杆
        props.north = true;
        props.south = true;
        let bar_ns = BlockState::from_id(props.to_state_id(&Block::IRON_BARS));
        props.east = true;
        let bar_nse = BlockState::from_id(props.to_state_id(&Block::IRON_BARS));
        // 东西朝向的栏杆
        props.north = false;
        props.south = false;
        props.west = true;
        let bar_we = BlockState::from_id(props.to_state_id(&Block::IRON_BARS));

        for i in 1..=3 {
            inner.add_block(chunk, bar_ns, 4, i, 4, &box_limit);
            inner.add_block(chunk, bar_nse, 4, i, 5, &box_limit);
            inner.add_block(chunk, bar_ns, 4, i, 6, &box_limit);

            inner.add_block(chunk, bar_we, 5, i, 5, &box_limit);
            inner.add_block(chunk, bar_we, 6, i, 5, &box_limit);
            inner.add_block(chunk, bar_we, 7, i, 5, &box_limit);
        }

        // // 门上方的顶部横杆
        inner.add_block(chunk, bar_ns, 4, 3, 2, &box_limit);
        inner.add_block(chunk, bar_ns, 4, 3, 8, &box_limit);

        // // 6. 铁门（2 格高的结构）
        // let door_bottom = Block::IRON_DOOR.default_state.with("facing", "west").with("half", "lower");
        // let door_top = Block::IRON_DOOR.default_state.with("facing", "west").with("half", "upper");

        // // 门 1
        // inner.add_block(chunk, &door_bottom, 4, 1, 2, &box_limit);
        // inner.add_block(chunk, &door_top, 4, 2, 2, &box_limit);

        // // 门 2
        // inner.add_block(chunk, &door_bottom, 4, 1, 8, &box_limit);
        // inner.add_block(chunk, &door_top, 4, 2, 8, &box_limit);
    }
}
