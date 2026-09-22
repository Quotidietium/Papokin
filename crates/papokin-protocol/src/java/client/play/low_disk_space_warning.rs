use papokin_data::packet::clientbound::play::LOW_DISK_SPACE_WARNING;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::ClientPacket;

/// 警告客户端服务器磁盘空间不足。
///
/// 于 26.1 加入。此数据包通知客户端显示警告
/// 有关存储空间不足的覆盖层/通知。
#[java_packet(LOW_DISK_SPACE_WARNING)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CLowDiskSpaceWarning;

impl CLowDiskSpaceWarning {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ClientPacket for CLowDiskSpaceWarning {
    fn write_packet_data(
        &self,
        _write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        Ok(())
    }
}
