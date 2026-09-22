use crate::wit::papokin::plugin::event::{Event, EventType, PlayerAdvancementDoneEventData};

use super::super::FromIntoEvent;

/// 玩家完成进度时触发的事件。
pub struct PlayerAdvancementDoneEvent;
impl FromIntoEvent for PlayerAdvancementDoneEvent {
    const EVENT_TYPE: EventType = EventType::PlayerAdvancementDoneEvent;
    type Data = PlayerAdvancementDoneEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerAdvancementDoneEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerAdvancementDoneEvent(data)
    }
}
