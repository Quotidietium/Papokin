use crate::wit::papokin::plugin::event::{EntityPlaceEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体放置方块时触发的事件。
pub struct EntityPlaceEvent;
impl FromIntoEvent for EntityPlaceEvent {
    const EVENT_TYPE: EventType = EventType::EntityPlaceEvent;
    type Data = EntityPlaceEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityPlaceEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityPlaceEvent(data)
    }
}
