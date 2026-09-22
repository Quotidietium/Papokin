//! 酿造模块。
//!
//! 本模块处理酿造台机制：
//! - [`BrewingStandScreenHandler`] - 酿造台界面的屏幕处理器
//!
//! 酿造台允许玩家通过组合水瓶来酿造药水，
//! 与各种配料配合使用。

pub mod brewing_screen_handler;

pub use brewing_screen_handler::create_brewing;
