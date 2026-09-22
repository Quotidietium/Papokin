use papokin_data::packet::clientbound::play::INITIALIZE_BORDER;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use crate::{VarInt, codec::var_long::VarLong};
use papokin_util::version::JavaMinecraftVersion;

/// 为客户端完整初始化世界边界。
///
/// 当玩家加入世界或切换维度时，会发送此数据包。
/// 它同步当前位置、大小和所有警告参数
/// 以确保客户端的视觉屏障与服务器的权威状态一致。
#[java_packet(INITIALIZE_BORDER)]
pub struct CInitializeWorldBorder {
    /// 世界边界中心的 X 坐标。
    pub x: f64,
    /// 世界边界中心的 Z 坐标。
    pub z: f64,
    /// 边界开始移动时的直径。
    pub old_diameter: f64,
    /// 边界将要移动到的直径。
    pub new_diameter: f64,
    /// 达到 `new_diameter` 所需的时间（以毫秒为单位）。
    pub speed: VarLong,
    /// 玩家经传送门传送的最大距离
    /// 之前，否则边界会阻止传送。
    pub portal_teleport_boundary: VarInt,
    /// 屏幕开始泛红处与边界的距离（以方块计）。
    pub warning_blocks: VarInt,
    /// 玩家与……处于碰撞航向所需的时间（秒）
    /// 警告色调出现前与边界的距离。
    pub warning_time: VarInt,
}

impl CInitializeWorldBorder {
    #[expect(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        x: f64,
        z: f64,
        old_diameter: f64,
        new_diameter: f64,
        speed: VarLong,
        portal_teleport_boundary: VarInt,
        warning_blocks: VarInt,
        warning_time: VarInt,
    ) -> Self {
        Self {
            x,
            z,
            old_diameter,
            new_diameter,
            speed,
            portal_teleport_boundary,
            warning_blocks,
            warning_time,
        }
    }
}

impl ClientPacket for CInitializeWorldBorder {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_f64_be(self.x)?;
        write.write_f64_be(self.z)?;
        write.write_f64_be(self.old_diameter)?;
        write.write_f64_be(self.new_diameter)?;
        write.write_var_long(&self.speed)?;
        write.write_var_int(&self.portal_teleport_boundary)?;
        write.write_var_int(&self.warning_blocks)?;
        write.write_var_int(&self.warning_time)?;
        Ok(())
    }
}
