/// 创建由示例组成的 [`Vec<String>`]，取自
/// 给定的字符串字面量。
#[macro_export]
macro_rules! examples {
    ( $( $example:literal ),* ) => {
        vec! [
            $( $example.to_string(), )*
        ]
    };
}

pub use papokin_command::argument_types::*;

pub mod entity;
pub mod entity_anchor;
pub mod entity_selector;
pub mod game_profile;
pub mod objective;
pub mod pool;
pub mod resource_key;
pub mod team;
pub mod template;
