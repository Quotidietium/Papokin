use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::{ANIMATE, SWING_ANIMATION};
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 为实体触发一个客户端可见的特定动画。
///
/// 这主要用于玩家触发的动画，例如挥动手臂
/// 或显示伤害，但它也可以应用于其他实体。
#[java_packet(ANIMATE)]
pub struct CEntityAnimation {
    /// 执行动画的实体 ID。
    pub entity_id: VarInt,
    /// 要播放的动画 ID。
    /// 标准取值见下表。
    pub animation: u8,
}

impl CEntityAnimation {
    #[must_use]
    pub const fn new(entity_id: VarInt, animation: Animation) -> Self {
        Self {
            entity_id,
            animation: animation as u8,
        }
    }
}

impl ClientPacket for CEntityAnimation {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.entity_id)?;
        // 26.3 将挥动动作移入独立数据包并对其余动画重新编号
        let animation = if *version >= JavaMinecraftVersion::V_26_3 {
            match self.animation {
                2 => 0, // 离开床
                4 => 1, // 暴击效果
                5 => 2, // 魔法暴击效果
                other => other,
            }
        } else {
            self.animation
        };
        write.write_u8(animation)?;
        Ok(())
    }
}

/// 播放实体手部的挥动动画。
///
/// 在 26.2 及之前，这是 [`CEntityAnimation`] 的一段动画；自 26.3 起，它是一个独立的数据包，
/// 还携带所持物品的挥动动画。
pub struct CSwingArm {
    pub entity_id: VarInt,
    pub off_hand: bool,
}

impl CSwingArm {
    #[must_use]
    pub const fn new(entity_id: VarInt, off_hand: bool) -> Self {
        Self {
            entity_id,
            off_hand,
        }
    }
}

impl crate::packet::MultiVersionJavaPacket for CSwingArm {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        if version >= JavaMinecraftVersion::V_26_3 {
            SWING_ANIMATION.to_id(version)
        } else {
            ANIMATE.to_id(version)
        }
    }
}

impl ClientPacket for CSwingArm {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.entity_id)?;
        if *version >= JavaMinecraftVersion::V_26_3 {
            write.write_var_int(&VarInt(i32::from(self.off_hand)))?;
            // 手持物品的挥动动画，持续 6 刻的挥击是默认动画
            write.write_var_int(&VarInt(1))?;
            write.write_var_int(&VarInt(6))?;
        } else if self.off_hand {
            write.write_u8(Animation::SwingOffhand as u8)?;
        } else {
            write.write_u8(Animation::SwingMainArm as u8)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Animation {
    SwingMainArm,
    LeaveBed = 2,
    SwingOffhand,
    CriticalEffect,
    MagicCriticaleffect,
}
