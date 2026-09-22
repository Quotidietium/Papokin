//! 结构模板注册与放置。
//!
//! 本模块让插件在运行时注册原版 `.nbt` 结构模板
//! （gzip 压缩的 NBT），并按名称放入世界。已注册的
//! 模板存活在 `/place template`
//! 命令所解析的同一个服务器端模板缓存中，因此在此注册的模板可立即
//! 同时被本 API 与该命令放置——`/place template` 也能
//! 放置插件注册的模板。
//!
//! 放置流程与
//! `/place template` 命令经过相同的方块变更管线。
//!
//! # Examples
//!
//! ## Registering and placing a template
//! ```rust,ignore
//! use papokin_plugin_api::structure::{
//!     self, BlockPos, Rotation, WorldStructureExt,
//! };
//!
//! fn build(world: &papokin_plugin_api::wit::papokin::plugin::world::World, nbt: &[u8]) {
//!     structure::register_structure("my_plugin:hall", nbt).expect("valid template");
//!     assert!(structure::has_structure("my_plugin:hall"));
//!
//!     let origin = BlockPos { x: 0, y: 64, z: 0 };
//!     world
//!         .place_structure("my_plugin:hall", origin, Some(Rotation::Clockwise90), None)
//!         .expect("template exists");
//! }
//! ```

pub use crate::wit::papokin::plugin::common::BlockPos;
pub use crate::wit::papokin::plugin::structure::{
    Mirror, Rotation, has_structure, list_structures, register_structure,
};

use crate::wit::papokin::plugin::structure::place_structure;
use crate::wit::papokin::plugin::world::World;

/// 在 [`World`] 上放置结构模板的扩展 trait。
pub trait WorldStructureExt {
    /// 放置一个已注册或内嵌的结构模板，其原点在
    /// `pos`。
    ///
    /// `rotation` 和 `mirror` 默认为 [`Rotation::None`] /
    /// 传入 `None` 时得到 [`Mirror::None`]。当没有
    /// 名为 `name` 的模板存在。放置采用方块缓冲，并且
    /// 通过服务器的常规方块更新管线刷入，因此每个
    /// 客户端版本都能观察到该变更。
    ///
    /// # Errors
    ///
    ///当模板名称无法解析时，返回错误字符串。
    fn place_structure(
        &self,
        name: &str,
        pos: BlockPos,
        rotation: Option<Rotation>,
        mirror: Option<Mirror>,
    ) -> Result<bool, String>;
}

impl WorldStructureExt for World {
    fn place_structure(
        &self,
        name: &str,
        pos: BlockPos,
        rotation: Option<Rotation>,
        mirror: Option<Mirror>,
    ) -> Result<bool, String> {
        place_structure(self, name, pos, rotation, mirror)
    }
}
