use crate::wit::papokin::plugin::event::{BlockPlaceEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 玩家放置方块时触发的事件。
///
/// 关联的 [`BlockPlaceEventData`] 包含玩家、被放置的方块、
/// 放置时依托的方块、位置以及 `can-build` 标志。该事件
/// 此事件可取消。
pub struct BlockPlaceEvent;
impl FromIntoEvent for BlockPlaceEvent {
    const EVENT_TYPE: EventType = EventType::BlockPlaceEvent;
    type Data = BlockPlaceEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockPlaceEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockPlaceEvent(data)
    }
}
