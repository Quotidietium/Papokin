use crate::wit::papokin::plugin::event::{Event, EventType, VaultChangeStateEventData};

use super::super::FromIntoEvent;

/// 宝库方块改变状态时触发的事件。
pub struct VaultChangeStateEvent;
impl FromIntoEvent for VaultChangeStateEvent {
    const EVENT_TYPE: EventType = EventType::VaultChangeStateEvent;
    type Data = VaultChangeStateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::VaultChangeStateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::VaultChangeStateEvent(data)
    }
}
