use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, PrepareSmithingEventData};

/// 物品在锻造台上备料时触发的事件。
pub struct PrepareSmithingEvent;
impl FromIntoEvent for PrepareSmithingEvent {
    const EVENT_TYPE: EventType = EventType::PrepareSmithingEvent;
    type Data = PrepareSmithingEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PrepareSmithingEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PrepareSmithingEvent(data)
    }
}
