use crate::wit::papokin::plugin::event::{EntityAttemptSmashAttackEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体尝试猛击时触发的事件。
pub struct EntityAttemptSmashAttackEvent;
impl FromIntoEvent for EntityAttemptSmashAttackEvent {
    const EVENT_TYPE: EventType = EventType::EntityAttemptSmashAttackEvent;
    type Data = EntityAttemptSmashAttackEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityAttemptSmashAttackEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityAttemptSmashAttackEvent(data)
    }
}
