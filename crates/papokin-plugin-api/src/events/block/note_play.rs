use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, NotePlayEventData};

/// 音符盒播放时触发的事件。
pub struct NotePlayEvent;
impl FromIntoEvent for NotePlayEvent {
    const EVENT_TYPE: EventType = EventType::NotePlayEvent;
    type Data = NotePlayEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::NotePlayEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::NotePlayEvent(data)
    }
}
