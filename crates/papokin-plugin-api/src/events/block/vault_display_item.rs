use crate::wit::papokin::plugin::event::{Event, EventType, VaultDisplayItemEventData};

use super::super::FromIntoEvent;

/// 宝库展示物品时触发的事件。
pub struct VaultDisplayItemEvent;
impl FromIntoEvent for VaultDisplayItemEvent {
    const EVENT_TYPE: EventType = EventType::VaultDisplayItemEvent;
    type Data = VaultDisplayItemEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VaultDisplayItemEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VaultDisplayItemEvent(data)
    }
}
