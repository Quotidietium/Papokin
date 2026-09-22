use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{
    Event, EventType, PlayerBucketEmptyEventData, PlayerBucketFillEventData,
};

/// 玩家倒空桶时触发的事件。
pub struct PlayerBucketEmptyEvent;
impl FromIntoEvent for PlayerBucketEmptyEvent {
    const EVENT_TYPE: EventType = EventType::PlayerBucketEmptyEvent;
    type Data = PlayerBucketEmptyEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerBucketEmptyEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerBucketEmptyEvent(data)
    }
}

/// 玩家装填桶时触发的事件。
pub struct PlayerBucketFillEvent;
impl FromIntoEvent for PlayerBucketFillEvent {
    const EVENT_TYPE: EventType = EventType::PlayerBucketFillEvent;
    type Data = PlayerBucketFillEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerBucketFillEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerBucketFillEvent(data)
    }
}
