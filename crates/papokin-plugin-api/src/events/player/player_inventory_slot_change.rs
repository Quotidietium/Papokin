use crate::wit::papokin::plugin::event::{Event, EventType, PlayerInventorySlotChangeEventData};

use super::super::FromIntoEvent;

/// 玩家物品栏槽位中的物品变化时触发的事件。
pub struct PlayerInventorySlotChangeEvent;
impl FromIntoEvent for PlayerInventorySlotChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerInventorySlotChangeEvent;
    type Data = PlayerInventorySlotChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerInventorySlotChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerInventorySlotChangeEvent(data)
    }
}
