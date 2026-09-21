use pumpkin_macros::Event;

/// An event that occurs when a `GS4` (`GameSpy` 4) query is received.
///
/// This is a pure notification; `data` carries the key/value pairs already
/// collected for the response (read-only in the current implementation).
/// There is no player object involved, so this event does not implement
/// `PlayerEvent`.
#[derive(Event, Clone)]
pub struct Gs4QueryEvent {
    /// The type of the query (`basic` or `full`).
    pub query_type: String,

    /// The address of the querier.
    pub querier_address: String,

    /// The response data as key/value pairs.
    pub data: Vec<(String, String)>,
}

impl Gs4QueryEvent {
    /// Creates a new instance of `Gs4QueryEvent`.
    pub fn new(
        query_type: impl Into<String>,
        querier_address: impl Into<String>,
        data: Vec<(String, String)>,
    ) -> Self {
        Self {
            query_type: query_type.into(),
            querier_address: querier_address.into(),
            data,
        }
    }
}
