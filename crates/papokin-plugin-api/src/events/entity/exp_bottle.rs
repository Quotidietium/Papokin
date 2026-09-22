use crate::wit::papokin::plugin::event::{Event, EventType, ExpBottleEventData};

use super::super::FromIntoEvent;

/// 经验瓶破裂时触发的事件。
pub struct ExpBottleEvent;
impl FromIntoEvent for ExpBottleEvent {
    const EVENT_TYPE: EventType = EventType::ExpBottleEvent;
    type Data = ExpBottleEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ExpBottleEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ExpBottleEvent(data)
    }
}
