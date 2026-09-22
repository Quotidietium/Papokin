use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, InventoryInteractEventData};

/// 玩家与物品栏交互时触发的事件。
pub struct InventoryInteractEvent;
impl FromIntoEvent for InventoryInteractEvent {
    const EVENT_TYPE: EventType = EventType::InventoryInteractEvent;
    type Data = InventoryInteractEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::InventoryInteractEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::InventoryInteractEvent(data)
    }
}
