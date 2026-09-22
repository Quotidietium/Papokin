use crate::wit::papokin::plugin::event::{EndermanAttackPlayerEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 末影人攻击玩家时触发的事件。
pub struct EndermanAttackPlayerEvent;
impl FromIntoEvent for EndermanAttackPlayerEvent {
    const EVENT_TYPE: EventType = EventType::EndermanAttackPlayerEvent;
    type Data = EndermanAttackPlayerEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EndermanAttackPlayerEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EndermanAttackPlayerEvent(data)
    }
}
