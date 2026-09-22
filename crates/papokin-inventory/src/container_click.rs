//! 容器点击处理。
//!
//! 此模块处理来自客户端的物品栏点击数据包，并将它们
//! 转换为结构化的点击事件。它处理所有点击类型：
//! - 鼠标点击（左键/右键）
//! - Shift 点击
//! - 快捷栏按键
//! - 丢弃操作（按 Q 键或点击界面外侧）
//! - 拖拽操作（按住并跨槽位拖动）
//! - 双击（拿取全部）
//!
//! # Click Types
//!
//! 客户端发送模式与按钮值，二者会被解码为
//! [`ClickType`] 的各个变体。详见 Minecraft 协议文档
//! 以了解数据包格式细节。

use crate::InventoryError;
use papokin_protocol::java::server::play::SlotActionType;

/// 一个已解析的容器点击事件。
///
/// 包含被点击的槽位以及点击动作的类型。
#[derive(Debug)]
pub struct Click {
    /// 被点击的槽位（或在物品栏之外）。
    pub slot: Slot,
    /// 点击操作的类型（鼠标点击、Shift+点击、拖动等）。
    pub click_type: ClickType,
}

/// 普通鼠标点击对应的按钮值。
const BUTTON_CLICK_LEFT: i8 = 0;
const BUTTON_CLICK_RIGHT: i8 = 1;

/// 副手切换的按键代码（默认为 F 键）。
const KEY_CLICK_OFFHAND: i8 = 40;

/// 快捷栏槽位按键范围（1-9 键）。
const KEY_CLICK_HOTBAR_START: i8 = 0;
const KEY_CLICK_HOTBAR_END: i8 = 9;

/// 表示在物品栏外点击的槽位索引。
const SLOT_INDEX_OUTSIDE: i16 = -999;

impl Click {
    /// 将槽位操作解析为点击事件。
    ///
    /// # Arguments
    /// - `mode` - 协议中的操作类型
    /// - `button` - 来自协议的按钮值
    /// - `slot` - 槽位索引（-999 表示在外部）
    ///
    /// # Returns
    /// 解析得到的点击操作，无效时返回错误。
    ///
    /// # Errors
    /// 返回 [`InventoryError::InvalidSlot`] 或 [`InventoryError::InvalidPacket`]
    /// 用于处理格式错误的输入。
    pub fn new(mode: &SlotActionType, button: i8, slot: i16) -> Result<Self, InventoryError> {
        match mode {
            SlotActionType::Pickup => Self::new_normal_click(button, slot),
            // 两个按钮在这里作用相同，因此省略
            SlotActionType::QuickMove => Self::new_shift_click(slot),
            SlotActionType::Swap => Self::new_key_click(button, slot),
            SlotActionType::Clone => Ok(Self {
                click_type: ClickType::CreativePickItem,
                slot: Slot::Normal(slot.try_into().or(Err(InventoryError::InvalidSlot))?),
            }),
            SlotActionType::Throw => Self::new_drop_item(button, slot),
            SlotActionType::QuickCraft => Self::new_drag_item(button, slot),
            SlotActionType::PickupAll => Ok(Self {
                click_type: ClickType::DoubleClick,
                slot: Slot::Normal(slot.try_into().or(Err(InventoryError::InvalidSlot))?),
            }),
        }
    }

    fn new_normal_click(button: i8, slot: i16) -> Result<Self, InventoryError> {
        let slot = if slot == SLOT_INDEX_OUTSIDE {
            Slot::OutsideInventory
        } else {
            let slot = slot.try_into().unwrap_or(0);
            Slot::Normal(slot)
        };
        let button = match button {
            BUTTON_CLICK_LEFT => MouseClick::Left,
            BUTTON_CLICK_RIGHT => MouseClick::Right,
            _ => Err(InventoryError::InvalidPacket)?,
        };
        Ok(Self {
            click_type: ClickType::MouseClick(button),
            slot,
        })
    }

    fn new_shift_click(slot: i16) -> Result<Self, InventoryError> {
        Ok(Self {
            slot: Slot::Normal(slot.try_into().or(Err(InventoryError::InvalidSlot))?),
            click_type: ClickType::ShiftClick,
        })
    }

    fn new_key_click(button: i8, slot: i16) -> Result<Self, InventoryError> {
        let key = match button {
            KEY_CLICK_HOTBAR_START..KEY_CLICK_HOTBAR_END => {
                KeyClick::Slot(button.try_into().or(Err(InventoryError::InvalidSlot))?)
            }
            KEY_CLICK_OFFHAND => KeyClick::Offhand,
            _ => Err(InventoryError::InvalidSlot)?,
        };

        Ok(Self {
            click_type: ClickType::KeyClick(key),
            slot: Slot::Normal(slot.try_into().or(Err(InventoryError::InvalidSlot))?),
        })
    }

    fn new_drop_item(button: i8, slot: i16) -> Result<Self, InventoryError> {
        let drop_type = DropType::from_i8(button)?;
        let slot = if slot == SLOT_INDEX_OUTSIDE {
            Slot::OutsideInventory
        } else {
            let slot = slot.try_into().unwrap_or(0);
            Slot::Normal(slot)
        };
        Ok(Self {
            click_type: ClickType::DropType(drop_type),
            slot,
        })
    }

    fn new_drag_item(button: i8, slot: i16) -> Result<Self, InventoryError> {
        let state = match button {
            0 => MouseDragState::Start(MouseDragType::Left),
            4 => MouseDragState::Start(MouseDragType::Right),
            8 => MouseDragState::Start(MouseDragType::Middle),
            1 | 5 | 9 => {
                MouseDragState::AddSlot(slot.try_into().or(Err(InventoryError::InvalidSlot))?)
            }
            2 | 6 | 10 => MouseDragState::End,
            _ => Err(InventoryError::InvalidPacket)?,
        };
        Ok(Self {
            slot: match &state {
                MouseDragState::AddSlot(slot) => Slot::Normal(*slot),
                _ => Slot::OutsideInventory,
            },
            click_type: ClickType::MouseDrag { drag_state: state },
        })
    }
}

/// 点击操作的类型。
#[derive(Debug)]
pub enum ClickType {
    /// 普通鼠标点击（左键或右键）。
    MouseClick(MouseClick),
    /// 按 Shift 点击以快速移动物品。
    ShiftClick,
    /// 快捷栏按键（1-9 或副手切换）。
    KeyClick(KeyClick),
    /// 创造模式中键点击（选取方块）。
    CreativePickItem,
    /// 丢弃物品（Q 键或丢弃点击）。
    DropType(DropType),
    /// 跨多个槽位拖动物品。
    MouseDrag { drag_state: MouseDragState },
    /// 双击以收集物品。
    DoubleClick,
}

/// 普通鼠标按键点击。
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MouseClick {
    Left,
    Right,
}

/// 快捷栏按键点击。
#[derive(Debug)]
pub enum KeyClick {
    /// 快捷栏槽位索引（0-8）。
    Slot(u8),
    /// 与副手交换。
    Offhand,
}

/// 一个槽位引用——可以是特定槽位，也可以是物品栏之外。
#[derive(Debug, Copy, Clone)]
pub enum Slot {
    Normal(usize),
    OutsideInventory,
}

/// 丢弃操作类型。
#[derive(Debug)]
pub enum DropType {
    /// 丢弃单个物品（Ctrl+Q）。
    SingleItem,
    /// 丢弃整组物品（Q）。
    FullStack,
}

impl DropType {
    const fn from_i8(value: i8) -> Result<Self, InventoryError> {
        Ok(match value {
            0 => Self::SingleItem,
            1 => Self::FullStack,
            _ => return Err(InventoryError::InvalidPacket),
        })
    }
}

/// 鼠标拖动按键类型。
#[derive(Debug, PartialEq, Eq)]
pub enum MouseDragType {
    /// 左键拖动——均匀分配。
    Left,
    /// 右键拖动 - 每个槽位一个物品。
    Right,
    /// 中键拖动 - 创建整组物品堆（仅创造模式）。
    Middle,
}

/// 拖动操作状态。
#[derive(PartialEq, Eq, Debug)]
pub enum MouseDragState {
    /// 拖拽开始——按键决定拖拽类型。
    Start(MouseDragType),
    /// 将槽位添加到拖拽中。
    AddSlot(usize),
    /// 拖动结束——应用于所有选中的槽位。
    End,
}
