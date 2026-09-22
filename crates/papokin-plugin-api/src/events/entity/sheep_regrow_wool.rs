use crate::wit::papokin::plugin::event::{Event, EventType, SheepRegrowWoolEventData};

use super::super::FromIntoEvent;

/// 羊重新长出羊毛时触发的事件。
pub struct SheepRegrowWoolEvent;
impl FromIntoEvent for SheepRegrowWoolEvent {
    const EVENT_TYPE: EventType = EventType::SheepRegrowWoolEvent;
    type Data = SheepRegrowWoolEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::SheepRegrowWoolEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::SheepRegrowWoolEvent(data)
    }
}
