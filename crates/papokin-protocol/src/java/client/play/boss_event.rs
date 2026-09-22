use std::io::Write;

use crate::java::client::play::BosseventAction;
use crate::ser::NetworkWriteExt;
use crate::{ClientPacket, WritingError};
use papokin_data::packet::clientbound::play::BOSS_EVENT;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 更新显示在玩家屏幕顶部的“Boss 血条”。
///
/// 此数据包用于管理末影龙等实体的血条
/// 或凋灵，以及用于服务器事件或袭击的自定义进度条。
#[java_packet(BOSS_EVENT)]
pub struct CBossEvent<'a> {
    /// 此特定 Boss 血条实例的唯一标识符。
    pub uuid: &'a uuid::Uuid,
    /// 要执行的操作（Add、Remove、Update Health 等）。
    pub action: BosseventAction,
}

impl<'a> CBossEvent<'a> {
    #[must_use]
    pub const fn new(uuid: &'a uuid::Uuid, action: BosseventAction) -> Self {
        Self { uuid, action }
    }
}

impl ClientPacket for CBossEvent<'_> {
    fn write_packet_data(
        &self,
        write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;

        write.write_uuid(self.uuid)?;
        let action = &self.action;
        match action {
            BosseventAction::Add {
                title,
                health,
                color,
                division,
                flags,
            } => {
                write.write_var_int(&0.into())?;
                write.write_component(title, version)?;
                write.write_f32_be(*health)?;
                write.write_var_int(color)?;
                write.write_var_int(division)?;
                write.write_u8(*flags)
            }
            BosseventAction::Remove => write.write_var_int(&1.into()),
            BosseventAction::UpdateHealth(health) => {
                write.write_var_int(&2.into())?;
                write.write_f32_be(*health)
            }
            BosseventAction::UpdateTile(title) => {
                write.write_var_int(&3.into())?;
                write.write_component(title, version)
            }
            BosseventAction::UpdateStyle { color, dividers } => {
                write.write_var_int(&4.into())?;
                write.write_var_int(color)?;
                write.write_var_int(dividers)
            }
            BosseventAction::UpdateFlags(flags) => {
                write.write_var_int(&5.into())?;
                write.write_u8(*flags)
            }
        }
    }
}
