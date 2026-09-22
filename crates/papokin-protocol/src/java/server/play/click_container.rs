use crate::VarInt;
use crate::codec::item_stack_seralizer::OptionalItemStackHash;
use crate::{
    ServerPacket,
    ser::{NetworkReadExt, ReadingError},
};
use papokin_data::packet::serverbound::play::CONTAINER_CLICK;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;
use std::io::Read;

#[derive(Debug)]
#[java_packet(CONTAINER_CLICK)]
pub struct SClickSlot {
    pub sync_id: VarInt,
    pub revision: VarInt,
    pub slot: i16,
    pub button: i8,
    pub mode: SlotActionType,
    pub length_of_array: VarInt,
    pub array_of_changed_slots: Vec<(i16, OptionalItemStackHash)>,
    pub carried_item: OptionalItemStackHash,
}

impl SClickSlot {
    pub const BUTTON_LEFT: i8 = 0;
    pub const BUTTON_RIGHT: i8 = 1;
    pub const BUTTON_MIDDLE: i8 = 2;
    pub const BUTTON_DROP_SINGLE: i8 = 0;
    pub const BUTTON_DROP_STACK: i8 = 1;
    pub const BUTTON_OFFHAND_SWAP: i8 = 40;
}

impl<'a> ServerPacket<'a> for SClickSlot {
    fn read(
        mut bytebuf: &mut &'a [u8],
        version: &JavaMinecraftVersion,
    ) -> Result<Self, ReadingError> {
        let sync_id = bytebuf.get_container_id(version)?;
        let revision = if version >= &JavaMinecraftVersion::V_1_17_1 {
            bytebuf.get_var_int()?
        } else {
            VarInt(i32::from(bytebuf.get_i16_be()?))
        };
        let slot = bytebuf.get_i16_be()?;
        let button = bytebuf.get_i8()?;
        let mode = SlotActionType::read(&mut bytebuf)?;

        let length_of_array = bytebuf.get_var_int()?;
        if length_of_array.0 < 0 || length_of_array.0 > 256 {
            return Err(ReadingError::Message(
                "Changed slots length out of bounds".into(),
            ));
        }
        let mut array_of_changed_slots = Vec::with_capacity(length_of_array.0 as usize);
        for _ in 0..length_of_array.0 {
            array_of_changed_slots.push((
                bytebuf.get_i16_be()?,
                OptionalItemStackHash::read(&mut bytebuf)?,
            ));
        }

        let carried_item = OptionalItemStackHash::read(&mut bytebuf)?;

        Ok(Self {
            sync_id,
            revision,
            slot,
            button,
            mode,
            length_of_array,
            array_of_changed_slots,
            carried_item,
        })
    }
}

impl crate::ClientPacket for SClickSlot {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_container_id(&self.sync_id, version)?;
        if version >= &JavaMinecraftVersion::V_1_17_1 {
            write.write_var_int(&self.revision)?;
        } else {
            write.write_i16_be(self.revision.0 as i16)?;
        }
        write.write_i16_be(self.slot)?;
        write.write_i8(self.button)?;
        self.mode.write(&mut write)?;
        write.write_var_int(&VarInt(self.array_of_changed_slots.len() as i32))?;
        for (slot, item) in &self.array_of_changed_slots {
            write.write_i16_be(*slot)?;
            item.write(&mut write)?;
        }
        self.carried_item.write(&mut write)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SlotActionType {
    /// 执行普通槽位点击。这会拾取或放置槽位中的物品，可能将光标物品堆合并入槽位；如果无法合并，则将槽位物品堆与光标物品堆交换。
    Pickup,
    /// 执行 Shift 点击。这通常会在玩家物品栏与打开的界面处理器之间快速移动物品。
    QuickMove,
    /// 在槽位与快捷栏槽位之间交换物品。这通常由玩家在悬停于某个槽位上时按下 1-9 数字键触发。
    /// 当操作类型为 swap 时，点击数据是要与之交换的快捷栏槽位（0-8）。
    Swap,
    /// 复制槽位中的物品。通常由创造模式下中键点击物品触发。
    Clone,
    /// 将物品从物品栏中抛出。通常由玩家将光标悬停在槽位上时按下 Q，或点击窗口外部触发。
    /// 当操作类型为 throw 时，点击数据决定是扔出整组物品（1）还是从该组中扔出单个物品（0）。
    Throw,
    /// 在多个槽位之间拖动物品。这通常由玩家在各槽位之间点击并拖动触发。
    /// 此操作分 3 个阶段进行。阶段 0 表示拖拽已开始，阶段 2 表示拖拽已结束。在此之间的多个阶段 1 表示拖拽经过了哪些槽位。
    QuickCraft,
    /// 用屏幕处理器中的物品补充光标上的物品堆。这通常由玩家双击触发。
    PickupAll,
}

impl SlotActionType {
    pub fn read(bytebuf: &mut impl Read) -> Result<Self, ReadingError> {
        let mode = bytebuf.get_var_int()?;
        Self::try_from(mode.0)
            .map_err(|_| ReadingError::Message("Invalid slot action type".to_string()))
    }

    pub fn write(
        &self,
        write: &mut impl crate::ser::NetworkWriteExt,
    ) -> Result<(), crate::ser::WritingError> {
        let mode = match self {
            Self::Pickup => 0,
            Self::QuickMove => 1,
            Self::Swap => 2,
            Self::Clone => 3,
            Self::Throw => 4,
            Self::QuickCraft => 5,
            Self::PickupAll => 6,
        };
        write.write_var_int(&VarInt(mode))
    }
}

#[derive(Debug)]
pub struct InvalidSlotActionType;

impl TryFrom<i32> for SlotActionType {
    type Error = InvalidSlotActionType;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Pickup),
            1 => Ok(Self::QuickMove),
            2 => Ok(Self::Swap),
            3 => Ok(Self::Clone),
            4 => Ok(Self::Throw),
            5 => Ok(Self::QuickCraft),
            6 => Ok(Self::PickupAll),
            _ => Err(InvalidSlotActionType),
        }
    }
}
