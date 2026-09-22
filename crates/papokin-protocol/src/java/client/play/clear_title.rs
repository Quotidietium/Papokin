use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::CLEAR_TITLES;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;
/// 移除玩家屏幕上当前显示的任何标题或副标题。
///
/// 此数据包用于立即隐藏当前正处于其显示阶段的标题
/// “停留”或“淡出”阶段。
#[java_packet(CLEAR_TITLES)]
pub struct CClearTitle {
    /// 如果为 true，客户端还会重置标题计时（淡入、停留、淡出）
    /// 重置为它们的默认值（10、70、20 刻）。
    ///
    /// 如果想清除文本但保留自定义计时，请将其设为 false
    /// 将应用于你发送的下一个标题。
    pub reset: bool,
}

impl CClearTitle {
    #[must_use]
    pub const fn new(reset: bool) -> Self {
        Self { reset }
    }
}

impl ClientPacket for CClearTitle {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_bool(self.reset)?;
        Ok(())
    }
}
