use crate::wit::papokin::plugin::event::{Event, EventType, InventoryBlockStartEventData};

use super::super::FromIntoEvent;

/// 物品栏方块开始一次操作时触发的事件。
pub struct InventoryBlockStartEvent;
impl FromIntoEvent for InventoryBlockStartEvent {
    const EVENT_TYPE: EventType = EventType::InventoryBlockStartEvent;
    type Data = InventoryBlockStartEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::InventoryBlockStartEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::InventoryBlockStartEvent(data)
    }
}
