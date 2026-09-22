use crate::wit::papokin::plugin::event::{Event, EventType, PreLookupProfileEventData};

use super::super::FromIntoEvent;

/// 按名称查询玩家档案前触发的事件。
pub struct PreLookupProfileEvent;
impl FromIntoEvent for PreLookupProfileEvent {
    const EVENT_TYPE: EventType = EventType::PreLookupProfileEvent;
    type Data = PreLookupProfileEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PreLookupProfileEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PreLookupProfileEvent(data)
    }
}
