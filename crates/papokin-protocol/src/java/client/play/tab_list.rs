use papokin_data::packet::clientbound::play::TAB_LIST;
use papokin_macros::java_packet;
use papokin_util::text::TextComponent;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 更新玩家列表（Tab 列表）的页眉与页脚。
#[java_packet(TAB_LIST)]
pub struct CTabList<'a> {
    /// 显示在玩家列表顶部的文本。
    pub header: &'a TextComponent,
    /// 显示在玩家列表底部的文本。
    pub footer: &'a TextComponent,
}

impl<'a> CTabList<'a> {
    #[must_use]
    pub const fn new(header: &'a TextComponent, footer: &'a TextComponent) -> Self {
        Self { header, footer }
    }
}

impl ClientPacket for CTabList<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_component(self.header, version)?;
        write.write_component(self.footer, version)?;
        Ok(())
    }
}
