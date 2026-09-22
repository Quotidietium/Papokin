use crate::wit::papokin::plugin::event::{Event, EventType, SpawnChangeEventData};

use super::super::FromIntoEvent;

/// 世界出生点变化时触发的事件。
///
/// 关联的 [`SpawnChangeEventData`] 包含世界、先前的生成点
/// 位置、偏航角和俯仰角，以及新的重生位置、偏航角和俯仰角。
pub struct SpawnChangeEvent;
impl FromIntoEvent for SpawnChangeEvent {
    const EVENT_TYPE: EventType = EventType::SpawnChangeEvent;
    type Data = SpawnChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::SpawnChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::SpawnChangeEvent(data)
    }
}
