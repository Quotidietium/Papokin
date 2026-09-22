use papokin_data::packet::clientbound::play::OPEN_SCREEN;
use papokin_util::text::TextComponent;

use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 指示客户端打开特定类型的 GUI（物品栏、箱子等）。
///
/// 当玩家与方块（如箱子）交互时，会发送此数据包
/// 或当命令/插件强制打开界面时。它会建立
/// `sync_id`，后续所有的 "Set Slot" 或 "Click Slot"
/// 数据包，以确保服务器与客户端讨论的是同一个窗口。
#[java_packet(OPEN_SCREEN)]
pub struct COpenScreen<'a> {
    /// 当前窗口会话的唯一标识符。
    /// 通常每打开一个新窗口就递增 1。
    pub sync_id: VarInt,
    /// 要打开的窗口类型的 ID（例如 Generic 9x3、工作台）。
    /// 标准 ID 见下表。
    pub window_type: VarInt,
    /// 显示在 GUI 顶部的标题。
    /// 支持完整 JSON 格式化（颜色、粗体等）。
    pub window_title: &'a TextComponent,
}

impl<'a> COpenScreen<'a> {
    #[must_use]
    pub const fn new(
        window_id: VarInt,
        window_type: VarInt,
        window_title: &'a TextComponent,
    ) -> Self {
        Self {
            sync_id: window_id,
            window_type,
            window_title,
        }
    }
}

impl ClientPacket for COpenScreen<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.sync_id)?;
        let window_type = papokin_data::menu_id_remap::remap_menu_id_for_version(
            self.window_type.0 as u8,
            *version,
        );
        write.write_var_int(&VarInt(i32::from(window_type)))?;
        write.write_component(self.window_title, version)
    }
}
