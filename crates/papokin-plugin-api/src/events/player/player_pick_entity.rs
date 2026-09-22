use crate::wit::papokin::plugin::event::{Event, EventType, PlayerPickEntityEventData};

use super::super::FromIntoEvent;

/// 玩家选取实体（中键点击）并
/// 收到物品。此事件可取消。
pub struct PlayerPickEntityEvent;
impl FromIntoEvent for PlayerPickEntityEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPickEntityEvent;
    type Data = PlayerPickEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPickEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPickEntityEvent(data)
    }
}
