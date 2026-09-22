use crate::wit::papokin::plugin::event::{Event, EventType, HorseJumpEventData};

use super::super::FromIntoEvent;

/// 马跳跃时触发的事件。
pub struct HorseJumpEvent;
impl FromIntoEvent for HorseJumpEvent {
    const EVENT_TYPE: EventType = EventType::HorseJumpEvent;
    type Data = HorseJumpEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::HorseJumpEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::HorseJumpEvent(data)
    }
}
