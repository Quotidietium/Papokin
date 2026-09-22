use papokin_data::translation;
use papokin_nbt::{nbt_ops::NbtOps, tag::NbtTag};
use papokin_util::uuid::parse_uuid_vec;

use crate::{errors::error_types::CommandErrorType, parser::Parser, snbt::SnbtParser};
use papokin_codecs::DynamicOps;

pub const EXPECTED_NUMBER_OR_BOOLEAN: CommandErrorType<0> =
    CommandErrorType::new(translation::java::SNBT_PARSER_EXPECTED_NUMBER_OR_BOOLEAN);

pub const EXPECTED_STRING_UUID: CommandErrorType<0> =
    CommandErrorType::new(translation::java::SNBT_PARSER_EXPECTED_STRING_UUID);

/// 表示一种*操作*，它可接受*操作数*并返回所需的*结果*。
pub type SnbtOperation = fn(parser: &mut SnbtParser, args: &[NbtTag]) -> Option<NbtTag>;

/// 一个管理编译期固化的 SNBT 操作的管理器。
pub struct SnbtOperations;

impl SnbtOperations {
    pub const BUILTIN_IDS: &[&str] = &["true", "false", "bool", "uuid"];

    /// 搜索要运行的操作，来源为
    /// 给定的标识符与参数数量。
    pub fn search(id: &str, arg_count: usize) -> Option<SnbtOperation> {
        match (id, arg_count) {
            ("bool", 1) => Some(Self::bool),
            ("uuid", 1) => Some(Self::uuid),
            _ => None,
        }
    }

    /// 表示 SNBT 中的 `bool` 一元运算符。
    ///
    /// 对布尔值而言如同恒等操作，
    /// 并且对非零数字返回 `true`。
    fn bool(parser: &mut SnbtParser, args: &[NbtTag]) -> Option<NbtTag> {
        NbtOps.get_bool(&args[0]).into_result().map_or_else(
            || {
                parser.store_simple_error(&EXPECTED_NUMBER_OR_BOOLEAN);
                None
            },
            |result| Some(NbtTag::Byte(result as i8)),
        )
    }

    /// 表示 SNBT 中的 `uuid` 一元运算符。
    ///
    /// 将字符串中的 UUID 解析为包含 4 个整数的数组。
    fn uuid(parser: &mut SnbtParser, args: &[NbtTag]) -> Option<NbtTag> {
        if let NbtTag::String(string) = &args[0]
            && let Some(ints) = parse_uuid_vec(string)
        {
            Some(NbtTag::IntArray(ints))
        } else {
            parser.store_simple_error(&EXPECTED_STRING_UUID);
            None
        }
    }
}
