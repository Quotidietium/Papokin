use crate::wit::papokin::plugin::event::{CampfireStartEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 营火开始烧炼物品时触发的事件。
pub struct CampfireStartEvent;
impl FromIntoEvent for CampfireStartEvent {
    const EVENT_TYPE: EventType = EventType::CampfireStartEvent;
    type Data = CampfireStartEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CampfireStartEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CampfireStartEvent(data)
    }
}
