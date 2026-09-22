pub mod configured_features;
// 因此我们首先遍历所有已放置的地物，检查是否应该放置某个地物
// 的某处并使用 `placed_features`。之后如果想放置某个地物，我们就把它放到
// 使用 `configured_features`，其中包含我们打算如何放置
// 特性。
pub mod placed_features;

pub mod features;
mod size;
