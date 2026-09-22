use crate::wit::papokin::plugin::event::{Event, EventType, InventoryClickEventData};

use super::super::FromIntoEvent;

/// 玩家点击物品栏槽位时触发的事件。
pub struct InventoryClickEvent;

impl FromIntoEvent for InventoryClickEvent {
    const EVENT_TYPE: EventType = EventType::InventoryClickEvent;
    type Data = InventoryClickEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::InventoryClickEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::InventoryClickEvent(data)
    }
}
