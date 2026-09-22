use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, InventoryOpenEventData};

/// 物品栏容器被打开时触发的事件。
pub struct InventoryOpenEvent;
impl FromIntoEvent for InventoryOpenEvent {
    const EVENT_TYPE: EventType = EventType::InventoryOpenEvent;
    type Data = InventoryOpenEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::InventoryOpenEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::InventoryOpenEvent(data)
    }
}
