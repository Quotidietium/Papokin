use crate::wit::papokin::plugin::event::{Event, EventType, PlayerServerFullCheckEventData};

use super::super::FromIntoEvent;

/// 玩家尝试加入已满的服务器时触发的事件。将
/// 结果设为 `allowed` 可让玩家照常加入。
pub struct PlayerServerFullCheckEvent;
impl FromIntoEvent for PlayerServerFullCheckEvent {
    const EVENT_TYPE: EventType = EventType::PlayerServerFullCheckEvent;
    type Data = PlayerServerFullCheckEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerServerFullCheckEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerServerFullCheckEvent(data)
    }
}
