use crate::wit::papokin::plugin::event::{Event, EventType, InventoryCloseEventData};

use super::super::FromIntoEvent;

/// 玩家关闭物品栏时触发的事件。
pub struct InventoryCloseEvent;

impl FromIntoEvent for InventoryCloseEvent {
    const EVENT_TYPE: EventType = EventType::InventoryCloseEvent;
    type Data = InventoryCloseEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::InventoryCloseEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::InventoryCloseEvent(data)
    }
}
