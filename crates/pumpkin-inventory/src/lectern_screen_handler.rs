use std::any::Any;
use std::sync::Arc;

use crate::screen_handler::{
    InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour, ScreenProperty, offer_or_drop_stack,
};
use crate::slot::NormalSlot;

use crate::inventory::Inventory;
use crate::window_property::PropertyDelegate;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::screen::WindowType;

/// Callbacks into the lectern block so page turns and book removal can drive
/// block-state changes (redstone pulse, `has_book`) that live outside this crate.
pub trait LecternController: Send + Sync {
    /// The page currently displayed.
    fn current_page(&self) -> i32;

    /// Clamps and persists `page`, emitting a redstone pulse when it changes.
    fn set_page(&self, page: i32);

    /// Restores the bookless block state after the book was taken.
    fn on_book_taken(&self);

    /// Called when a player clicks "take book", before the book is removed.
    /// Returns `false` to veto the take.
    fn on_book_take_click(&self, _player: &dyn InventoryPlayer) -> bool {
        true
    }

    /// Called when a player requests a page change (previous/next/jump),
    /// before the page is applied. Returns the page to apply (`Some`), which
    /// may differ from `new_page`, or `None` to veto the change.
    fn on_page_change_click(&self, _player: &dyn InventoryPlayer, new_page: i32) -> Option<i32> {
        Some(new_page)
    }
}

/// Exposes the current page as container property 0 (see `window_property::Lectern`).
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

/// Vanilla `LecternScreenHandler`: a single book slot, no player slots and the
/// current page synced as property 0. Page navigation and taking the book are
/// plain button clicks sent by the client.
pub struct LecternScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    inventory: Arc<dyn Inventory>,
    controller: Arc<dyn LecternController>,
}

impl LecternScreenHandler {
    const PREVIOUS_PAGE_BUTTON_ID: i32 = 1;
    const NEXT_PAGE_BUTTON_ID: i32 = 2;
    const TAKE_BOOK_BUTTON_ID: i32 = 3;
    /// Button ids at or above this jump directly to `id - JUMP_TO_PAGE_OFFSET`.
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
        // The lectern screen has no player slots, so nothing can be shift-clicked.
        ItemStack::EMPTY.clone()
    }
}
