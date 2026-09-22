use crate::wit::papokin::plugin::event::{
    Event, EventType, PlayerConnectionInitialConfigureEventData,
};

use super::super::FromIntoEvent;

/// 玩家连接初始配置时触发的事件。
pub struct PlayerConnectionInitialConfigureEvent;
impl FromIntoEvent for PlayerConnectionInitialConfigureEvent {
    const EVENT_TYPE: EventType = EventType::PlayerConnectionInitialConfigureEvent;
    type Data = PlayerConnectionInitialConfigureEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerConnectionInitialConfigureEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerConnectionInitialConfigureEvent(data)
    }
}
