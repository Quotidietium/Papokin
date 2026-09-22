use crate::wit::papokin::plugin::event::{BlockMultiPlaceEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 一次放置多个方块时触发的事件。
pub struct BlockMultiPlaceEvent;
impl FromIntoEvent for BlockMultiPlaceEvent {
    const EVENT_TYPE: EventType = EventType::BlockMultiPlaceEvent;
    type Data = BlockMultiPlaceEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockMultiPlaceEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockMultiPlaceEvent(data)
    }
}
