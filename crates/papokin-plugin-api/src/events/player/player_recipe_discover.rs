use crate::wit::papokin::plugin::event::{Event, EventType, PlayerRecipeDiscoverEventData};

use super::super::FromIntoEvent;

/// 玩家发现配方时触发的事件。
pub struct PlayerRecipeDiscoverEvent;
impl FromIntoEvent for PlayerRecipeDiscoverEvent {
    const EVENT_TYPE: EventType = EventType::PlayerRecipeDiscoverEvent;
    type Data = PlayerRecipeDiscoverEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerRecipeDiscoverEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerRecipeDiscoverEvent(data)
    }
}
