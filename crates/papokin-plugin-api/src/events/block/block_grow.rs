use crate::wit::papokin::plugin::event::{BlockGrowEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块自然生长或改变状态时触发的事件（如作物、树苗）。
///
/// 关联的 [`BlockGrowEventData`] 包含世界、旧方块与新方块
/// 标识符及其状态 ID，以及方块位置。此事件可取消。
pub struct BlockGrowEvent;
impl FromIntoEvent for BlockGrowEvent {
    const EVENT_TYPE: EventType = EventType::BlockGrowEvent;
    type Data = BlockGrowEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockGrowEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockGrowEvent(data)
    }
}
