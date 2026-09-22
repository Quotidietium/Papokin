use crate::wit::papokin::plugin::event::{EnderDragonFlameEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 末影龙吐息时触发的事件。
pub struct EnderDragonFlameEvent;
impl FromIntoEvent for EnderDragonFlameEvent {
    const EVENT_TYPE: EventType = EventType::EnderDragonFlameEvent;
    type Data = EnderDragonFlameEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EnderDragonFlameEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EnderDragonFlameEvent(data)
    }
}
