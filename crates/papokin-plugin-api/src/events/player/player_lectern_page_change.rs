use crate::wit::papokin::plugin::event::{Event, EventType, PlayerLecternPageChangeEventData};

use super::super::FromIntoEvent;

/// 玩家翻动物台上书本页面时触发的事件（在
/// 讲台。此事件可取消；新页码可以被修改。
pub struct PlayerLecternPageChangeEvent;
impl FromIntoEvent for PlayerLecternPageChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerLecternPageChangeEvent;
    type Data = PlayerLecternPageChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerLecternPageChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerLecternPageChangeEvent(data)
    }
}
