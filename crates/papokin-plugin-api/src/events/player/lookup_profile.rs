use crate::wit::papokin::plugin::event::{Event, EventType, LookupProfileEventData};

use super::super::FromIntoEvent;

/// 按名称查询到玩家档案后触发的事件。
pub struct LookupProfileEvent;
impl FromIntoEvent for LookupProfileEvent {
    const EVENT_TYPE: EventType = EventType::LookupProfileEvent;
    type Data = LookupProfileEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::LookupProfileEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::LookupProfileEvent(data)
    }
}
