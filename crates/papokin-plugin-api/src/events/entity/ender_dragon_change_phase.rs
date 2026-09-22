use crate::wit::papokin::plugin::event::{EnderDragonChangePhaseEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 末影龙切换阶段时触发的事件。
pub struct EnderDragonChangePhaseEvent;
impl FromIntoEvent for EnderDragonChangePhaseEvent {
    const EVENT_TYPE: EventType = EventType::EnderDragonChangePhaseEvent;
    type Data = EnderDragonChangePhaseEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EnderDragonChangePhaseEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EnderDragonChangePhaseEvent(data)
    }
}
