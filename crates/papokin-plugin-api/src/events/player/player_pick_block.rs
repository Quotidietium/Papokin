use crate::wit::papokin::plugin::event::{Event, EventType, PlayerPickBlockEventData};

use super::super::FromIntoEvent;

/// 玩家选取方块（中键点击）并
/// 收到物品。此事件可取消。
pub struct PlayerPickBlockEvent;
impl FromIntoEvent for PlayerPickBlockEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPickBlockEvent;
    type Data = PlayerPickBlockEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPickBlockEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPickBlockEvent(data)
    }
}
