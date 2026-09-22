use crate::wit::papokin::plugin::event::{Event, EventType, PlayerRiptideEventData};

use super::super::FromIntoEvent;

/// 玩家激活激流附魔时触发的事件。
pub struct PlayerRiptideEvent;
impl FromIntoEvent for PlayerRiptideEvent {
    const EVENT_TYPE: EventType = EventType::PlayerRiptideEvent;
    type Data = PlayerRiptideEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerRiptideEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerRiptideEvent(data)
    }
}
