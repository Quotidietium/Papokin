use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::PLAYER_COMBAT_KILL;
use papokin_macros::java_packet;
use papokin_util::text::TextComponent;
use papokin_util::version::JavaMinecraftVersion;

/// 通知客户端有玩家死亡。
///
/// 此数据包负责在客户端上触发死亡界面
/// 并在聊天中为死亡玩家显示死亡消息。
#[java_packet(PLAYER_COMBAT_KILL)]
pub struct CCombatDeath<'a> {
    /// 死亡的玩家实体 ID。
    pub player_id: VarInt,
    /// 要显示的死亡消息（例如 "Player was pricked to death by a Cactus"）。
    pub message: &'a TextComponent,
}

impl<'a> CCombatDeath<'a> {
    #[must_use]
    pub const fn new(player_id: VarInt, message: &'a TextComponent) -> Self {
        Self { player_id, message }
    }
}

impl ClientPacket for CCombatDeath<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.player_id)?;
        write.write_component(self.message, version)?;
        Ok(())
    }
}
