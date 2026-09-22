use crate::wit::papokin::plugin::event::{Event, EventType, FireworkExplodeEventData};

use super::super::FromIntoEvent;

/// 烟花火箭爆炸时触发的事件。
pub struct FireworkExplodeEvent;
impl FromIntoEvent for FireworkExplodeEvent {
    const EVENT_TYPE: EventType = EventType::FireworkExplodeEvent;
    type Data = FireworkExplodeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::FireworkExplodeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::FireworkExplodeEvent(data)
    }
}
