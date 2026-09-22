use crate::wit::papokin::plugin::event::{BlockCanBuildEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 玩家尝试放置方块、检查能否建造时触发的事件。
///
/// 关联的 [`BlockCanBuildEventData`] 包含玩家、正在被
/// 放置位置、所贴靠的方块，以及一个 `buildable` 标志，该标志可
/// 覆盖。此事件可取消。
pub struct BlockCanBuildEvent;
impl FromIntoEvent for BlockCanBuildEvent {
    const EVENT_TYPE: EventType = EventType::BlockCanBuildEvent;
    type Data = BlockCanBuildEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockCanBuildEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockCanBuildEvent(data)
    }
}
