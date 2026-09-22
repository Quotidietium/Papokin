use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, InventoryMoveItemEventData};

/// 物品在物品栏之间移动时触发的事件。
pub struct InventoryMoveItemEvent;
impl FromIntoEvent for InventoryMoveItemEvent {
    const EVENT_TYPE: EventType = EventType::InventoryMoveItemEvent;
    type Data = InventoryMoveItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::InventoryMoveItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::InventoryMoveItemEvent(data)
    }
}
