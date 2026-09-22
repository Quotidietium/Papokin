use crate::wit::papokin::plugin::event::{BlockCookEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块烧炼物品时触发的事件。
pub struct BlockCookEvent;
impl FromIntoEvent for BlockCookEvent {
    const EVENT_TYPE: EventType = EventType::BlockCookEvent;
    type Data = BlockCookEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockCookEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockCookEvent(data)
    }
}
