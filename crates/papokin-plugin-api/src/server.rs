//! 服务器级查询工具：构建/版本信息与离线玩家查询。
//!
//! [`Server`](crate::Server) 资源直接以
//! `get_build_info`、`get_offline_player_by_uuid` 和
//! `get_offline_player_by_name` 的形式暴露这些功能；本模块再导出记录类型，并
//! 为便于使用在 [`Context`] 上也提供同样的查询。

pub use crate::wit::papokin::plugin::server::ServerBuildInfo as BuildInfo;
pub use crate::wit::papokin::plugin::server::*;

use crate::Context;

impl Context {
    ///返回运行中服务器的构建和版本信息。
    #[must_use]
    pub fn get_build_info(&self) -> BuildInfo {
        self.get_server().get_build_info()
    }

    /// 按 UUID 字符串查找玩家，在线离线均可。
    ///
    ///当玩家对服务器完全未知时，返回 `None`
    /// （不在线、无玩家数据文件、无用户缓存条目）。
    #[must_use]
    pub fn get_offline_player_by_uuid(&self, uuid: &str) -> Option<OfflinePlayerInfo> {
        self.get_server().get_offline_player_by_uuid(uuid)
    }

    /// 通过用户缓存按名称查找玩家，在线离线均可。
    ///
    ///当服务器不知道该名称时，返回 `None`。
    #[must_use]
    pub fn get_offline_player_by_name(&self, name: &str) -> Option<OfflinePlayerInfo> {
        self.get_server().get_offline_player_by_name(name)
    }
}
