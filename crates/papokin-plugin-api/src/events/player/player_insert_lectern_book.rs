use crate::wit::papokin::plugin::event::{Event, EventType, PlayerInsertLecternBookEventData};

use super::super::FromIntoEvent;

/// 玩家将书本放入讲台时触发的事件。此
/// 事件可取消。
pub struct PlayerInsertLecternBookEvent;
impl FromIntoEvent for PlayerInsertLecternBookEvent {
    const EVENT_TYPE: EventType = EventType::PlayerInsertLecternBookEvent;
    type Data = PlayerInsertLecternBookEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerInsertLecternBookEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerInsertLecternBookEvent(data)
    }
}
