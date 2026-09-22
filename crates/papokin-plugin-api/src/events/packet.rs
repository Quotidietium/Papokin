use super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{
    Event, EventType, PacketReceivedEventData, PacketSentEventData,
};

/// 从客户端收到数据包时触发的事件
pub struct PacketReceivedEvent;

impl FromIntoEvent for PacketReceivedEvent {
    const EVENT_TYPE: EventType = EventType::PacketReceivedEvent;
    type Data = PacketReceivedEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PacketReceivedEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PacketReceivedEvent(data)
    }
}

/// 向客户端发送数据包时触发的事件
pub struct PacketSentEvent;

impl FromIntoEvent for PacketSentEvent {
    const EVENT_TYPE: EventType = EventType::PacketSentEvent;
    type Data = PacketSentEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PacketSentEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PacketSentEvent(data)
    }
}
