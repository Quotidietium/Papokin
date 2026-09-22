use crate::wit::papokin::plugin::event::{BlockRedstoneEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 红石元件充能等级变化时触发的事件。
///
/// 关联的 [`BlockRedstoneEventData`] 包含世界、方块状态 ID、
/// 方块位置，以及新旧当前值。该事件可取消。
pub struct BlockRedstoneEvent;
impl FromIntoEvent for BlockRedstoneEvent {
    const EVENT_TYPE: EventType = EventType::BlockRedstoneEvent;
    type Data = BlockRedstoneEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockRedstoneEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockRedstoneEvent(data)
    }
}
