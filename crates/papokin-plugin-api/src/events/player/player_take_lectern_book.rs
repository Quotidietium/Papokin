use crate::wit::papokin::plugin::event::{Event, EventType, PlayerTakeLecternBookEventData};

use super::super::FromIntoEvent;

/// 玩家从讲台取书时触发的事件。
pub struct PlayerTakeLecternBookEvent;
impl FromIntoEvent for PlayerTakeLecternBookEvent {
    const EVENT_TYPE: EventType = EventType::PlayerTakeLecternBookEvent;
    type Data = PlayerTakeLecternBookEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerTakeLecternBookEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerTakeLecternBookEvent(data)
    }
}
