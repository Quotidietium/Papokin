use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{BlockFadeEventData, Event, EventType};

/// 方块消融或融化时触发的事件。
pub struct BlockFadeEvent;
impl FromIntoEvent for BlockFadeEvent {
    const EVENT_TYPE: EventType = EventType::BlockFadeEvent;
    type Data = BlockFadeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockFadeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockFadeEvent(data)
    }
}
