#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_jigsaw_generate(&self, player: &Arc<Player>, generate: &SJigsawGenerate) {
        if !player.is_creative() {
            return;
        }
        if player.permission_lvl.load() < PermissionLvl::Two {
            return;
        }
        let pos = generate.pos;
        if let Some(block_entity) = player.world().get_block_entity(&pos)
            && let Some(jigsaw_block) = block_entity.as_any().downcast_ref::<JigsawBlockEntity>()
        {
            // 原版客户端 UI 将生成层数限制为 0..=7；对改包客户端的
            // 任意 varint 做同等钳制，防止巨值/负值进入结构递归
            // 生成（负值可能绕过深度判断、巨值放大生成开销）
            let levels = generate.levels.0.clamp(0, 7);
            jigsaw_block.generate(&player.world(), levels, generate.keep_jigsaws);
        }
    }
}
