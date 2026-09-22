use papokin_data::packet::clientbound::play::SET_ACTION_BAR_TEXT;
use papokin_util::text::TextComponent;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;
/// 更新显示在玩家快捷栏上方的文本（Action Bar）。
///
/// 与聊天消息不同，Action Bar 文本是临时性的，通常用于
/// 非关键状态信息，如“Now entering: Wilderness”或
/// 法力/耐力计数器。
#[java_packet(SET_ACTION_BAR_TEXT)]
pub struct CActionBar<'a> {
    /// 要显示的文本组件。
    pub action_bar: &'a TextComponent,
}

impl<'a> CActionBar<'a> {
    #[must_use]
    pub const fn new(action_bar: &'a TextComponent) -> Self {
        Self { action_bar }
    }
}

impl ClientPacket for CActionBar<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_component(self.action_bar, version)
    }
}
