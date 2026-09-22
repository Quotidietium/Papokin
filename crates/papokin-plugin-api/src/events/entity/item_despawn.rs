use crate::wit::papokin::plugin::event::{Event, EventType, ItemDespawnEventData};

use super::super::FromIntoEvent;

/// 物品实体老化消失时触发的事件。
pub struct ItemDespawnEvent;
impl FromIntoEvent for ItemDespawnEvent {
    const EVENT_TYPE: EventType = EventType::ItemDespawnEvent;
    type Data = ItemDespawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ItemDespawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ItemDespawnEvent(data)
    }
}
