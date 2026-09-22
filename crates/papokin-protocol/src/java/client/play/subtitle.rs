use papokin_data::packet::clientbound::play::SET_SUBTITLE_TEXT;
use papokin_util::text::TextComponent;

use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(SET_SUBTITLE_TEXT)]
pub struct CSubtitle<'a> {
    pub subtitle: &'a TextComponent,
}

impl<'a> CSubtitle<'a> {
    #[must_use]
    pub const fn new(subtitle: &'a TextComponent) -> Self {
        Self { subtitle }
    }
}

impl ClientPacket for CSubtitle<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_component(self.subtitle, version)
    }
}
