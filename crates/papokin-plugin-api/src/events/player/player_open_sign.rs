use crate::wit::papokin::plugin::event::{Event, EventType, PlayerOpenSignEventData};

use super::super::FromIntoEvent;

/// 玩家打开告示牌编辑器时触发的事件。
pub struct PlayerOpenSignEvent;
impl FromIntoEvent for PlayerOpenSignEvent {
    const EVENT_TYPE: EventType = EventType::PlayerOpenSignEvent;
    type Data = PlayerOpenSignEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerOpenSignEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerOpenSignEvent(data)
    }
}
