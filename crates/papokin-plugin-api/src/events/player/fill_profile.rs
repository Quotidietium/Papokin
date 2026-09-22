use crate::wit::papokin::plugin::event::{Event, EventType, FillProfileEventData};

use super::super::FromIntoEvent;

/// 玩家档案已填充好其
/// 属性。
pub struct FillProfileEvent;
impl FromIntoEvent for FillProfileEvent {
    const EVENT_TYPE: EventType = EventType::FillProfileEvent;
    type Data = FillProfileEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::FillProfileEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::FillProfileEvent(data)
    }
}
