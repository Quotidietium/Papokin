use crate::wit::papokin::plugin::event::{
    Event, EventType, ItemTransportingEntityValidateTargetEventData,
};

use super::super::FromIntoEvent;

/// 运输物品的实体校验其目标方块时触发的事件。
pub struct ItemTransportingEntityValidateTargetEvent;
impl FromIntoEvent for ItemTransportingEntityValidateTargetEvent {
    const EVENT_TYPE: EventType = EventType::ItemTransportingEntityValidateTargetEvent;
    type Data = ItemTransportingEntityValidateTargetEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ItemTransportingEntityValidateTargetEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ItemTransportingEntityValidateTargetEvent(data)
    }
}
