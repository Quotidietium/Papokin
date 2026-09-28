#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_edit_book(&self, player: &Player, packet: &SEditBook<'_>) {
        let held_stack = player.inventory().held_item();
        if held_stack.item.id != Item::WRITABLE_BOOK.id {
            return;
        }

        let mut pages: Vec<String> = packet.pages.iter().map(|p| (*p).to_string()).collect();
        let mut title = packet.title.map(std::string::ToString::to_string);
        let signing = title.is_some();
        let slot = player.inventory().get_selected_slot() as u32;

        if let Some(player_arc) = player.world().get_player_by_uuid(player.gameprofile.id)
            && let Some(server) = player.world().server.upgrade()
        {
            let mut event =
                crate::plugin::api::events::player::player_edit_book::PlayerEditBookEvent {
                    player: player_arc,
                    slot,
                    pages: pages.clone(),
                    title: title.clone(),
                    signing,
                    cancelled: false,
                };
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
            pages = event.pages;
            title = event.title;
        }

        if let Some(title) = title {
            let mut written_book = ItemStack::new(1, &Item::WRITTEN_BOOK);
            let content = WrittenBookContentImpl {
                title,
                author: player.gameprofile.name.clone(),
                pages,
            };
            written_book
                .patch
                .push((DataComponent::WrittenBookContent, Some(content.to_dyn())));
            // 对齐原版：签名消耗一本可写书（手持堆扣 1，剩余保留），
            // 成品书给予玩家（背包满则掉落）。此前直接以成书覆盖手持
            // 槽，手持堆多于一本时其余可写书会被凭空销毁。
            // 扣减/替换在写锁内原子完成，防止事件等待期间并入该
            // 槽位的物品被覆盖；返回 Some 表示需另外给予成品书
            let offered: Option<ItemStack> =
                player.inventory().update_held(Hand::Right, |mut s| {
                    if s.item.id != Item::WRITABLE_BOOK.id {
                        (s, None)
                    } else if s.item_count > 1 {
                        s.decrement(1);
                        (s, Some(written_book))
                    } else {
                        (written_book, None)
                    }
                });
            if let Some(book) = offered {
                player.inventory().offer_or_drop_stack(book, player);
            }
        } else {
            player.inventory().update_held(Hand::Right, |mut s| {
                if s.item.id != Item::WRITABLE_BOOK.id {
                    return (s, ());
                }
                let content = WritableBookContentImpl { pages };
                s.patch
                    .retain(|(component, _)| *component != DataComponent::WritableBookContent);
                s.patch
                    .push((DataComponent::WritableBookContent, Some(content.to_dyn())));
                (s, ())
            });
        }
    }
}
