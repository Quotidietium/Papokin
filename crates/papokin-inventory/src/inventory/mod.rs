use papokin_data::item_stack::ItemStack;

#[expect(clippy::module_inception)]
mod inventory;
mod simple_inventory;

pub use inventory::*;
pub use simple_inventory::*;

// split_stack 的工具函数
pub fn split_stack_slice(stacks: &mut [ItemStack], slot: usize, amount: u8) -> ItemStack {
    if slot < stacks.len() && !stacks[slot].is_empty() && amount > 0 {
        stacks[slot].split(amount)
    } else {
        ItemStack::EMPTY.clone()
    }
}
