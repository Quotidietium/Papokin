use crate::wit::papokin::plugin::event::{Event, EventType, PlayerCustomPayloadEventData};

use super::super::FromIntoEvent;

/// 玩家发送自定义插件通道负载时触发的事件。
///
/// 关联的 [`PlayerCustomPayloadEventData`] 包含玩家、通道
/// 标识符以及原始载荷字节。
pub struct PlayerCustomPayloadEvent;
impl FromIntoEvent for PlayerCustomPayloadEvent {
    const EVENT_TYPE: EventType = EventType::PlayerCustomPayloadEvent;
    type Data = PlayerCustomPayloadEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerCustomPayloadEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerCustomPayloadEvent(data)
    }
}
