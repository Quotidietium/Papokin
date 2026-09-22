use crate::wit::papokin::plugin::event::{Event, EventType, PlayerBucketEntityEventData};

use super::super::FromIntoEvent;

/// 玩家用桶捕捉实体时触发的事件。
pub struct PlayerBucketEntityEvent;
impl FromIntoEvent for PlayerBucketEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerBucketEntityEvent;
    type Data = PlayerBucketEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerBucketEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerBucketEntityEvent(data)
    }
}
