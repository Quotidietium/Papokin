use papokin_data::packet::clientbound::play::ENTITY_EVENT;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 发送特定实体的状态更新。
///
/// 此数据包是各种实体触发器的“兜底”处理，用于那些没有
/// warrant a complex packet of their own. It primarily handles visual
/// 以及逻辑状态触发器，例如工具损坏、使用图腾，
/// 或剪羊毛。
#[java_packet(ENTITY_EVENT)]
pub struct CEntityStatus {
    /// 受状态变化影响的实体 ID。
    pub entity_id: i32,
    /// 要触发的状态/事件的 ID。
    /// 常见实体状态见下表。
    pub entity_status: i8,
}

impl CEntityStatus {
    #[must_use]
    pub const fn new(entity_id: i32, entity_status: i8) -> Self {
        Self {
            entity_id,
            entity_status,
        }
    }
}

impl ClientPacket for CEntityStatus {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_i32_be(self.entity_id)?;
        write.write_i8(self.entity_status)?;
        Ok(())
    }
}
