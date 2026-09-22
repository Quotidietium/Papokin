use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, LootGenerateEventData};

/// 战利品生成时触发的事件。
pub struct LootGenerateEvent;
impl FromIntoEvent for LootGenerateEvent {
    const EVENT_TYPE: EventType = EventType::LootGenerateEvent;
    type Data = LootGenerateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::LootGenerateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::LootGenerateEvent(data)
    }
}
