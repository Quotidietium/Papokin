use crate::wit::papokin::plugin::event::{Event, EventType, PlayerTrackEntityEventData};

use super::super::FromIntoEvent;

/// 实体开始被追踪（发送）给
/// 玩家。
pub struct PlayerTrackEntityEvent;
impl FromIntoEvent for PlayerTrackEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerTrackEntityEvent;
    type Data = PlayerTrackEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerTrackEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerTrackEntityEvent(data)
    }
}
