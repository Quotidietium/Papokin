use crate::wit::papokin::plugin::event::{BeaconEffectEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 信标对玩家施加状态效果时触发的事件。
pub struct BeaconEffectEvent;
impl FromIntoEvent for BeaconEffectEvent {
    const EVENT_TYPE: EventType = EventType::BeaconEffectEvent;
    type Data = BeaconEffectEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BeaconEffectEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BeaconEffectEvent(data)
    }
}
