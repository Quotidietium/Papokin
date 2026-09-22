use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 音符盒播放时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct NotePlayEvent {
    pub block_pos: BlockPos,
    pub instrument: String,
    pub note: u8,
}

impl NotePlayEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, instrument: String, note: u8) -> Self {
        Self {
            block_pos,
            instrument,
            note,
            cancelled: false,
        }
    }
}
