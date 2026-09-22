use crate::wit::papokin::plugin::event::{Event, EventType, PlayerEditBookEventData};

use super::super::FromIntoEvent;

/// 玩家编辑或署名书本时触发的事件。
pub struct PlayerEditBookEvent;
impl FromIntoEvent for PlayerEditBookEvent {
    const EVENT_TYPE: EventType = EventType::PlayerEditBookEvent;
    type Data = PlayerEditBookEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerEditBookEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerEditBookEvent(data)
    }
}
