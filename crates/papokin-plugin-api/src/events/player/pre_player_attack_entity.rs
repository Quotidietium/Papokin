use crate::wit::papokin::plugin::event::{Event, EventType, PrePlayerAttackEntityEventData};

use super::super::FromIntoEvent;

/// 玩家攻击实体之前触发的事件，允许
/// 攻击可以被重定向或取消。此事件可取消。
pub struct PrePlayerAttackEntityEvent;
impl FromIntoEvent for PrePlayerAttackEntityEvent {
    const EVENT_TYPE: EventType = EventType::PrePlayerAttackEntityEvent;
    type Data = PrePlayerAttackEntityEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PrePlayerAttackEntityEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PrePlayerAttackEntityEvent(data)
    }
}
