//! 自定义地图渲染（MapView）。
//!
//! 本模块让插件创建并绘制服务器端地图：设置
//! 单个像素或整个 128x128 画布（使用普通的 RGB(A) 颜色），
//! 重绘原版地形视图、锁定画布，并在其上放置光标
//! 图标（地图装饰）。通过创建
//! 新地图（[`create_map`] 或 [`WorldMapExt`] 便捷 trait）
//! 或通过 [`get_map`] 附着到已有地图 id 来获得 [`MapView`]。
//!
//! 颜色以 `0xAARRGGBB` 给出，并在宿主侧量化到原版地图
//! 调色板，插件永远无需处理调色板索引
//! 本身（仍可通过
//! [`MapView::set_colors_data`] / [`MapView::get_colors_data`]）。
//!
//! 每次修改都会立即推送更新后的地图数据包（`CMapItemData`）
//! 给持有该地图的玩家，并将其标记为待逐刻地图同步。
//!
//! # Examples
//!
//! ## Drawing on a brand-new map
//! ```rust,ignore
//! use papokin_plugin_api::map::{self, MapCursor, WorldMapExt, cursor_types};
//!
//! fn draw(world: &papokin_plugin_api::wit::papokin::plugin::world::World) {
//!     let map = world.create_map(0, 0, 0);
//!     // Red diagonal line.
//!     for i in 0..128 {
//!         map.set_pixel(i, i, map::rgb(255, 0, 0));
//!     }
//!     map.add_cursor(&MapCursor {
//!         icon_type: cursor_types::RED_X,
//!         x: 64,
//!         z: 64,
//!         direction: 0,
//!         display_name: Some("X marks the spot".to_string()),
//!     });
//!     map.lock();
//! }
//! ```

pub use crate::wit::papokin::plugin::map::{MapCursor, MapView, create_map, get_map};

use crate::wit::papokin::plugin::world::World;

/// 地图画布的宽度和高度（单位为像素）。
pub const MAP_CANVAS_SIZE: u32 = 128;

/// 整张地图画布的像素数（调色板字节数）。
pub const MAP_CANVAS_PIXELS: usize = (MAP_CANVAS_SIZE * MAP_CANVAS_SIZE) as usize;

/// 将 RGBA 通道打包为
/// [`MapView::set_pixel`]。Alpha `0` 会将像素清除为原版的
/// “未探索”颜色；其他任何 alpha 值都视为不透明。
#[must_use]
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

/// 将 RGB 通道打包为不透明的 `0xFFRRGGBB` 颜色。
#[must_use]
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    rgba(r, g, b, 255)
}

/// 原版地图光标（装饰）类型 id，供……使用
/// [`MapView::add_cursor`]，对应于
/// `papokin_data::map_decoration::MapDecorationType`。
pub mod cursor_types {
    /// 白色玩家标记。
    pub const PLAYER: i32 = 0;
    /// 物品展示框标记。
    pub const FRAME: i32 = 1;
    /// 红色标记（某些游戏模式中同队玩家）。
    pub const RED_MARKER: i32 = 2;
    /// 蓝色标记。
    pub const BLUE_MARKER: i32 = 3;
    /// 目标 X 标记。
    pub const TARGET_X: i32 = 4;
    /// 目标点标记。
    pub const TARGET_POINT: i32 = 5;
    /// 超出地图边缘时显示的玩家标记。
    pub const PLAYER_OFF_MAP: i32 = 6;
    /// 远离地图时显示的玩家标记。
    pub const PLAYER_OFF_LIMITS: i32 = 7;
    /// 林地府邸图标。
    pub const MANSION: i32 = 8;
    /// 海底神殿图标。
    pub const MONUMENT: i32 = 9;
    /// 白色旗帜标记。
    pub const BANNER_WHITE: i32 = 10;
    /// 橙色旗帜标记。
    pub const BANNER_ORANGE: i32 = 11;
    /// 品红色旗帜标记。
    pub const BANNER_MAGENTA: i32 = 12;
    /// 淡蓝色旗帜标记。
    pub const BANNER_LIGHT_BLUE: i32 = 13;
    /// 黄色旗帜标记。
    pub const BANNER_YELLOW: i32 = 14;
    /// 黄绿色旗帜标记。
    pub const BANNER_LIME: i32 = 15;
    /// 粉红色旗帜标记。
    pub const BANNER_PINK: i32 = 16;
    /// 灰色旗帜标记。
    pub const BANNER_GRAY: i32 = 17;
    /// 淡灰色旗帜标记。
    pub const BANNER_LIGHT_GRAY: i32 = 18;
    /// 青色旗帜标记。
    pub const BANNER_CYAN: i32 = 19;
    /// 紫色旗帜标记。
    pub const BANNER_PURPLE: i32 = 20;
    /// 蓝色旗帜标记。
    pub const BANNER_BLUE: i32 = 21;
    /// 棕色旗帜标记。
    pub const BANNER_BROWN: i32 = 22;
    /// 绿色旗帜标记。
    pub const BANNER_GREEN: i32 = 23;
    /// 红色旗帜标记。
    pub const BANNER_RED: i32 = 24;
    /// 黑色旗帜标记。
    pub const BANNER_BLACK: i32 = 25;
    /// 红叉（藏宝图目的地）。
    pub const RED_X: i32 = 26;
    /// 沙漠村庄图标。
    pub const VILLAGE_DESERT: i32 = 27;
    /// 平原村庄图标。
    pub const VILLAGE_PLAINS: i32 = 28;
    /// 热带草原村庄图标。
    pub const VILLAGE_SAVANNA: i32 = 29;
    /// 雪原村庄图标。
    pub const VILLAGE_SNOWY: i32 = 30;
    /// 针叶林村庄图标。
    pub const VILLAGE_TAIGA: i32 = 31;
    /// 丛林神庙图标。
    pub const JUNGLE_TEMPLE: i32 = 32;
    /// 沼泽小屋图标。
    pub const SWAMP_HUT: i32 = 33;
    /// 试炼密室图标。
    pub const TRIAL_CHAMBERS: i32 = 34;
}

/// 在 [`World`] 上创建地图的扩展 trait。
pub trait WorldMapExt {
    /// 在此世界中创建以（`center_x`, `center_z`）为中心的新地图
    /// 并返回其可绘制视图。`scale` 会被限制在 0..=4。
    fn create_map(&self, center_x: i32, center_z: i32, scale: u8) -> MapView;
}

impl WorldMapExt for World {
    fn create_map(&self, center_x: i32, center_z: i32, scale: u8) -> MapView {
        create_map(self, center_x, center_z, scale)
    }
}
