use crate::wit::papokin::plugin::event::{
    EntityDeathEventData, Event, EventType, PlayerDeathEventData,
};

use super::super::FromIntoEvent;

/// 实体死亡时触发的事件。
pub struct EntityDeathEvent;
impl FromIntoEvent for EntityDeathEvent {
    const EVENT_TYPE: EventType = EventType::EntityDeathEvent;
    type Data = EntityDeathEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityDeathEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityDeathEvent(data)
    }
}

/// 玩家死亡时触发的事件。
pub struct PlayerDeathEvent;
impl FromIntoEvent for PlayerDeathEvent {
    const EVENT_TYPE: EventType = EventType::PlayerDeathEvent;
    type Data = PlayerDeathEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerDeathEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerDeathEvent(data)
    }
}
