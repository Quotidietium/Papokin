use crate::wit::papokin::plugin::event::{
    Event, EventType, PlayerRecipeBookSettingsChangeEventData,
};

use super::super::FromIntoEvent;

/// 玩家更改配方书设置时触发的事件。
pub struct PlayerRecipeBookSettingsChangeEvent;
impl FromIntoEvent for PlayerRecipeBookSettingsChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerRecipeBookSettingsChangeEvent;
    type Data = PlayerRecipeBookSettingsChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerRecipeBookSettingsChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerRecipeBookSettingsChangeEvent(data)
    }
}
