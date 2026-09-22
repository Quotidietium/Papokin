use crate::wit::papokin::plugin::event::{BlockBurnEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 方块被火烧毁时触发的事件。
///
/// 关联的 [`BlockBurnEventData`] 包含着火的方块以及
/// 引燃它使其燃烧的方块。该事件可取消。
pub struct BlockBurnEvent;
impl FromIntoEvent for BlockBurnEvent {
    const EVENT_TYPE: EventType = EventType::BlockBurnEvent;
    type Data = BlockBurnEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockBurnEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockBurnEvent(data)
    }
}
