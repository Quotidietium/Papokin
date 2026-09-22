use crate::wit::papokin::plugin::event::{Event, EventType, PlayerRecipeBookClickEventData};

use super::super::FromIntoEvent;

/// 玩家在配方书中点击配方时触发的事件。
pub struct PlayerRecipeBookClickEvent;
impl FromIntoEvent for PlayerRecipeBookClickEvent {
    const EVENT_TYPE: EventType = EventType::PlayerRecipeBookClickEvent;
    type Data = PlayerRecipeBookClickEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerRecipeBookClickEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerRecipeBookClickEvent(data)
    }
}
