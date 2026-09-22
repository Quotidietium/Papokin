use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, SmithItemEventData};

/// 物品在锻造台合成时触发的事件。
pub struct SmithItemEvent;
impl FromIntoEvent for SmithItemEvent {
    const EVENT_TYPE: EventType = EventType::SmithItemEvent;
    type Data = SmithItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::SmithItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::SmithItemEvent(data)
    }
}
