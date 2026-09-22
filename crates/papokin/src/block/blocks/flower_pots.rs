use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, PathComputationType, RandomTickArgs, UseWithItemArgs};
use papokin_data::flower_pot_transformations::get_potted_item;
use papokin_data::{Block, BlockId, BlockState};
use papokin_macros::pumpkin_block_from_tag;
use papokin_world::world::BlockFlags;

#[pumpkin_block_from_tag("minecraft:flower_pots")]
pub struct FlowerPotBlock;

impl BlockBehaviour for FlowerPotBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        {
            let item = args.item_stack.item;
            //将花放入花盆中
            let potted_block_id = get_potted_item(item.id);
            if args.block.eq(&Block::FLOWER_POT) {
                if potted_block_id != BlockId::AIR {
                    // 种植钩子：取消即否决此次种植。
                    let mut pot_event = crate::plugin::api::events::player::player_flower_pot_manipulate::PlayerFlowerPotManipulateEvent::new(
                        args.player.clone(),
                        *args.position,
                        args.item_stack.clone(),
                    );
                    if let Some(server) = args.world.server.upgrade() {
                        server.plugin_manager.fire_blocking(&server, &mut pot_event);
                    }
                    if pot_event.cancelled {
                        return BlockActionResult::Consume;
                    }
                    args.world.set_block_state(
                        args.position,
                        Block::from_id(potted_block_id).default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    );
                    args.player.increment_stat(
                        papokin_data::statistic::StatisticCategory::Custom,
                        papokin_data::statistic::CustomStatistic::PotFlower as i32,
                        1,
                    );
                }
                return BlockActionResult::Success;
            } else if potted_block_id != BlockId::AIR {
                //如果玩家手持可以栽入花盆的物品，则不会有任何事情发生
                return BlockActionResult::Consume;
            }

            //取出花并清空花盆
            // 采摘钩子：被移除的花由当前的
            // 盆栽方块；取消即否决此次移除。
            let potted_item = papokin_data::item::Item::from_id(args.block.item_id)
                .map(|item| papokin_data::item_stack::ItemStack::new(1, item));
            if let Some(flower) = potted_item {
                let mut pot_event = crate::plugin::api::events::player::player_flower_pot_manipulate::PlayerFlowerPotManipulateEvent::new(
                    args.player.clone(),
                    *args.position,
                    flower,
                );
                if let Some(server) = args.world.server.upgrade() {
                    server.plugin_manager.fire_blocking(&server, &mut pot_event);
                }
                if pot_event.cancelled {
                    return BlockActionResult::Consume;
                }
            }
            args.world.set_block_state(
                args.position,
                Block::FLOWER_POT.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
            BlockActionResult::Success
        }
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let is_open_potted = args.block.eq(&Block::POTTED_OPEN_EYEBLOSSOM);
        let is_closed_potted = args.block.eq(&Block::POTTED_CLOSED_EYEBLOSSOM);
        if !is_open_potted && !is_closed_potted {
            return;
        }

        let is_open = is_open_potted;
        let should_be_open = args.world.eyeblossom_open(args.position).unwrap_or(is_open);

        if is_open != should_be_open {
            let next_block = if should_be_open {
                &Block::POTTED_OPEN_EYEBLOSSOM
            } else {
                &Block::POTTED_CLOSED_EYEBLOSSOM
            };
            args.world.set_block_state(
                args.position,
                next_block.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
