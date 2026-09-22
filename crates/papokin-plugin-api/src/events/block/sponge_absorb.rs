use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, SpongeAbsorbEventData};

/// 海绵吸水时触发的事件。
pub struct SpongeAbsorbEvent;
impl FromIntoEvent for SpongeAbsorbEvent {
    const EVENT_TYPE: EventType = EventType::SpongeAbsorbEvent;
    type Data = SpongeAbsorbEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::SpongeAbsorbEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::SpongeAbsorbEvent(data)
    }
}
