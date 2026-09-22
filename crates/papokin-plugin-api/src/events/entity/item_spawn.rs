use crate::wit::papokin::plugin::event::{Event, EventType, ItemSpawnEventData};

use super::super::FromIntoEvent;

/// 物品实体在世界生成时触发的事件。
pub struct ItemSpawnEvent;
impl FromIntoEvent for ItemSpawnEvent {
    const EVENT_TYPE: EventType = EventType::ItemSpawnEvent;
    type Data = ItemSpawnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ItemSpawnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ItemSpawnEvent(data)
    }
}
