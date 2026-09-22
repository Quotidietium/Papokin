//! 配方相关的类型与 trait。
//!
//! 本模块定义合成系统中配方处理的接口。
//! 它为可查找配方的屏幕处理器和物品栏提供 trait，
//! 能够作为配方输入。
//!
//! # Recipe System
//!
//! 配方系统涉及：
//! - [`RecipeFinderScreenHandler`] - 能查找匹配配方的屏幕处理器
//! - [`RecipeInputInventory`] - 提供合成输入的物品栏
//! - [`RecipeMatcher`] - 将物品与配方进行匹配的辅助工具
//! - [`RecipeFinder`] - 查找配方的辅助工具

use crate::inventory::Inventory;

/// 用于匹配配方原料的辅助结构体。
// RecipeMatcher.java
pub struct RecipeMatcher;

/// 用于查找配方的辅助结构体。
// RecipeFinder.java
pub struct RecipeFinder;

/// 用于可查找合成配方的界面处理器的 trait。
///
/// 实现此 trait 的屏幕处理器可以搜索配方
/// 与当前输入物品栏状态相匹配。
// AbstractRecipeScreenHandle.java
pub trait RecipeFinderScreenHandler {}

/// 用作配方输入的物品栏的 trait。
///
/// 合成网格实现此 trait，以提供其尺寸
/// 以及用于配方匹配的物品访问。
pub trait RecipeInputInventory: Inventory {
    /// 获取合成网格的宽度。
    fn get_width(&self) -> usize;

    /// 获取合成网格的高度。
    fn get_height(&self) -> usize;

    // TODO: 用于配方输入处理的更多方法
    // fn get_held_stacks()，应改为获取物品栏的锁
    // createRecipeInput
    // createPositionedRecipeInput
}
