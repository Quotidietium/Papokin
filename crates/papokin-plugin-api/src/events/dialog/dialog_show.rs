use crate::wit::papokin::plugin::event::{DialogShowEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 向玩家展示对话框时触发的事件。
pub struct DialogShowEvent;

impl FromIntoEvent for DialogShowEvent {
    const EVENT_TYPE: EventType = EventType::DialogShowEvent;
    type Data = DialogShowEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::DialogShowEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::DialogShowEvent(data)
    }
}
