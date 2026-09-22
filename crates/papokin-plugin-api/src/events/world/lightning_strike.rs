use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, LightningStrikeEventData};

/// 闪电击中世界时触发的事件。
pub struct LightningStrikeEvent;
impl FromIntoEvent for LightningStrikeEvent {
    const EVENT_TYPE: EventType = EventType::LightningStrikeEvent;
    type Data = LightningStrikeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::LightningStrikeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::LightningStrikeEvent(data)
    }
}
