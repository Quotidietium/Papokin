use papokin_data::packet::clientbound::play::GAME_EVENT;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 更新游戏状态或触发特定的环境变化。
///
/// 此数据包是服务器传达全局或
/// 特定于上下文的转换，例如改变天气、
/// 更改玩家的游戏模式，或显示制作人员名单。
#[java_packet(GAME_EVENT)]
pub struct CGameEvent {
    /// 事件类型的 ID。
    pub event: u8,
    /// 与事件关联的一个值（用途取决于事件 ID）。
    pub value: f32,
}

/// 那些随机逻辑总得在某个地方实现，对吧？
impl CGameEvent {
    #[must_use]
    pub const fn new(event: GameEvent, value: f32) -> Self {
        Self {
            event: event as u8,
            value,
        }
    }
}

pub enum GameEvent {
    NoRespawnBlockAvailable,
    BeginRaining,
    EndRaining,
    ChangeGameMode,
    WinGame,
    DemoEvent,
    ArrowHitPlayer,
    RainLevelChange,
    ThunderLevelChange,
    PlayPufferfishStringSound,
    PlayElderGuardianMobAppearance,
    EnabledRespawnScreen,
    LimitedCrafting,
    StartWaitingChunks,
}

impl ClientPacket for CGameEvent {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_u8(self.event)?;
        write.write_f32_be(self.value)?;
        Ok(())
    }
}
