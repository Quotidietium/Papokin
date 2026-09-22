use crate::wit::papokin::plugin::event::{AreaEffectCloudApplyEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 区域效果云对实体施加效果时触发的事件。
pub struct AreaEffectCloudApplyEvent;
impl FromIntoEvent for AreaEffectCloudApplyEvent {
    const EVENT_TYPE: EventType = EventType::AreaEffectCloudApplyEvent;
    type Data = AreaEffectCloudApplyEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AreaEffectCloudApplyEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AreaEffectCloudApplyEvent(data)
    }
}
