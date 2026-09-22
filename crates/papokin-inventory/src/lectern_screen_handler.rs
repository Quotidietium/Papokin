use std::any::Any;
use std::sync::Arc;

use crate::screen_handler::{
    InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour, ScreenProperty, offer_or_drop_stack,
};
use crate::slot::NormalSlot;

use crate::inventory::Inventory;
use crate::window_property::PropertyDelegate;
use papokin_data::item_stack::ItemStack;
use papokin_data::screen::WindowType;

/// 回调到讲台方块，使翻页和取书能够驱动
/// 方块状态变更（红石脉冲、`has_book`），它们位于本 crate 之外。
pub trait LecternController: Send + Sync {
    /// 当前显示的页面。
    fn current_page(&self) -> i32;

    /// 对 `page` 进行限制并持久化，在其变化时发出红石脉冲。
    fn set_page(&self, page: i32);

    /// 在书被取走后，恢复为无书的方块状态。
    fn on_book_taken(&self);

    /// 当玩家点击“取书”时调用，发生在书被移除之前。
    ///返回 `false` 以否决此次取出。
    fn on_book_take_click(&self, _player: &dyn InventoryPlayer) -> bool {
        true
    }

    /// 当玩家请求翻页（上一页/下一页/跳转）时调用，
    /// 在页面被应用之前调用。返回要应用的页面（`Some`），
    /// 可以与 `new_page` 不同，或为 `None` 以否决此更改。
    fn on_page_change_click(&self, _player: &dyn InventoryPlayer, new_page: i32) -> Option<i32> {
        Some(new_page)
    }
}

/// 将当前页码暴露为容器属性 0（参见 `window_property::Lectern`）。
struct PageDelegate(Arc<dyn LecternController>);

impl PropertyDelegate for PageDelegate {
    fn get_property(&self, index: i32) -> i32 {
        if index == 0 { self.0.current_page() } else { 0 }
    }

    fn set_property(&self, _index: i32, _value: i32) {}

    fn get_properties_size(&self) -> i32 {
        1
    }
}

/// 原版 `LecternScreenHandler`：只有一个书槽位，没有玩家槽位，且
/// 当前页会同步为属性 0。翻页与取书
/// 客户端发送的普通按钮点击。
pub struct LecternScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    inventory: Arc<dyn Inventory>,
    controller: Arc<dyn LecternController>,
}

impl LecternScreenHandler {
    const PREVIOUS_PAGE_BUTTON_ID: i32 = 1;
    const NEXT_PAGE_BUTTON_ID: i32 = 2;
    const TAKE_BOOK_BUTTON_ID: i32 = 3;
    /// 达到或超过此值的按钮 ID 将直接跳转到 `id - JUMP_TO_PAGE_OFFSET`。
    const JUMP_TO_PAGE_OFFSET: i32 = 100;

    pub fn new(
        sync_id: u8,
        inventory: Arc<dyn Inventory>,
        controller: Arc<dyn LecternController>,
    ) -> Self {
        let mut handler = Self {
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Lectern)),
            inventory: inventory.clone(),
            controller: controller.clone(),
        };

        handler.add_slot(Arc::new(NormalSlot::new(inventory, 0)));
        handler.add_property(ScreenProperty::new(Arc::new(PageDelegate(controller)), 0));

        handler
    }
}

impl ScreenHandler for LecternScreenHandler {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn get_behaviour(&self) -> &ScreenHandlerBehaviour {
        &self.behaviour
    }

    fn get_behaviour_mut(&mut self) -> &mut ScreenHandlerBehaviour {
        &mut self.behaviour
    }

    fn on_button_click(&mut self, player: &dyn InventoryPlayer, id: i32) -> bool {
        match id {
            Self::PREVIOUS_PAGE_BUTTON_ID => {
                let new_page = self.controller.current_page() - 1;
                if let Some(page) = self.controller.on_page_change_click(player, new_page) {
                    self.controller.set_page(page);
                    true
                } else {
                    false
                }
            }
            Self::NEXT_PAGE_BUTTON_ID => {
                let new_page = self.controller.current_page() + 1;
                if let Some(page) = self.controller.on_page_change_click(player, new_page) {
                    self.controller.set_page(page);
                    true
                } else {
                    false
                }
            }
            Self::TAKE_BOOK_BUTTON_ID => {
                let stack = self.inventory.get_stack(0);
                if stack.is_empty() {
                    return false;
                }
                if !self.controller.on_book_take_click(player) {
                    return false;
                }
                let stack = self.inventory.remove_stack(0);
                self.inventory.mark_dirty();
                self.controller.on_book_taken();
                offer_or_drop_stack(player, stack);
                self.send_content_updates();
                true
            }
            _ if id >= Self::JUMP_TO_PAGE_OFFSET => {
                let new_page = id - Self::JUMP_TO_PAGE_OFFSET;
                if let Some(page) = self.controller.on_page_change_click(player, new_page) {
                    self.controller.set_page(page);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn quick_move(&mut self, _player: &dyn InventoryPlayer, _slot_index: i32) -> ItemStack {
        // 讲台界面没有玩家槽位，因此没有任何东西可被 Shift+点击。
        ItemStack::EMPTY.clone()
    }
}
