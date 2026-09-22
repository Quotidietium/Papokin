use crate::wit::papokin::plugin::event::{Event, EventType, PlayerPortalEventData};

use super::super::FromIntoEvent;

/// 玩家使用传送门时触发的事件。
pub struct PlayerPortalEvent;
impl FromIntoEvent for PlayerPortalEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPortalEvent;
    type Data = PlayerPortalEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPortalEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPortalEvent(data)
    }
}
