use crate::wit::papokin::plugin::event::{Event, EventType, SheepDyeWoolEventData};

use super::super::FromIntoEvent;

/// 羊的羊毛被染色时触发的事件。
pub struct SheepDyeWoolEvent;
impl FromIntoEvent for SheepDyeWoolEvent {
    const EVENT_TYPE: EventType = EventType::SheepDyeWoolEvent;
    type Data = SheepDyeWoolEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::SheepDyeWoolEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::SheepDyeWoolEvent(data)
    }
}
