#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_chunk_batch(&self, player: &Player, packet: &SChunkBatch) {
        player
            .chunk_sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .on_batch_acknowledged(packet.chunks_per_tick);
        trace!("客户端请求每刻 {} 个区块", packet.chunks_per_tick);
    }
}
