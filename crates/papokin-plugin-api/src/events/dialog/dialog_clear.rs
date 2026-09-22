use crate::wit::papokin::plugin::event::{DialogClearEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 玩家的对话框被清除时触发的事件。
pub struct DialogClearEvent;

impl FromIntoEvent for DialogClearEvent {
    const EVENT_TYPE: EventType = EventType::DialogClearEvent;
    type Data = DialogClearEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::DialogClearEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::DialogClearEvent(data)
    }
}
