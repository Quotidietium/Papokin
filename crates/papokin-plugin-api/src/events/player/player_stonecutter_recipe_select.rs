use crate::wit::papokin::plugin::event::{
    Event, EventType, PlayerStonecutterRecipeSelectEventData,
};

use super::super::FromIntoEvent;

/// 玩家在切石机中选择配方时触发的事件。此
/// 事件可取消。
pub struct PlayerStonecutterRecipeSelectEvent;
impl FromIntoEvent for PlayerStonecutterRecipeSelectEvent {
    const EVENT_TYPE: EventType = EventType::PlayerStonecutterRecipeSelectEvent;
    type Data = PlayerStonecutterRecipeSelectEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerStonecutterRecipeSelectEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerStonecutterRecipeSelectEvent(data)
    }
}
