use crate::wit::papokin::plugin::event::{Event, EventType, ProjectileHitEventData};

use super::super::FromIntoEvent;

/// 弹射物击中实体或方块时触发的事件。
pub struct ProjectileHitEvent;
impl FromIntoEvent for ProjectileHitEvent {
    const EVENT_TYPE: EventType = EventType::ProjectileHitEvent;
    type Data = ProjectileHitEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ProjectileHitEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ProjectileHitEvent(data)
    }
}
