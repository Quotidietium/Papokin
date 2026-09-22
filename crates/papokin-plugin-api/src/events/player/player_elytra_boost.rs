use crate::wit::papokin::plugin::event::{Event, EventType, PlayerElytraBoostEventData};

use super::super::FromIntoEvent;

/// 玩家用烟花火箭助推鞘翅飞行时触发的事件。
pub struct PlayerElytraBoostEvent;
impl FromIntoEvent for PlayerElytraBoostEvent {
    const EVENT_TYPE: EventType = EventType::PlayerElytraBoostEvent;
    type Data = PlayerElytraBoostEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerElytraBoostEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerElytraBoostEvent(data)
    }
}
