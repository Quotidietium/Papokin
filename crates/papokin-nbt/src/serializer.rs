use std::io::Write;

use crate::Error;

/// NBT 序列化操作返回的结果类型。
pub type Result<T> = std::result::Result<T, Error>;

macro_rules! define_write_number_be {
    ($name:ident, $type:ty) => {
        fn $name(&mut self, value: $type) -> Result<()> {
            let buf = value.to_be_bytes();
            self.writer.write_all(&buf).map_err(Error::Incomplete)?;
            Ok(())
        }
    };
}

/// NBT 序列化器使用的特定格式原始类型写入器。
pub trait NbtWriteHelper {
    /// 底层输出写入器。
    type Writer: Write;

    /// 返回底层的输出写入器。
    fn writer(&mut self) -> &mut Self::Writer;
    /// 写入一个无符号字节。
    fn write_u8(&mut self, value: u8) -> Result<()>;
    /// 写入一个有符号字节。
    fn write_i8(&mut self, value: i8) -> Result<()>;
    /// 写入一个 16 位有符号整数。
    fn write_i16(&mut self, value: i16) -> Result<()>;
    /// 写入一个 32 位有符号整数。
    fn write_i32(&mut self, value: i32) -> Result<()>;
    /// 写入一个 64 位有符号整数。
    fn write_i64(&mut self, value: i64) -> Result<()>;
    /// 写入一个 32 位浮点数。
    fn write_f32(&mut self, value: f32) -> Result<()>;
    /// 写入一个 64 位浮点数。
    fn write_f64(&mut self, value: f64) -> Result<()>;
    /// 写入一个带长度前缀的字符串。
    fn write_string(&mut self, value: &str) -> Result<()>;

    /// 原样写入字节切片，不做修改。
    fn write_slice(&mut self, value: &[u8]) -> Result<()> {
        self.writer().write_all(value).map_err(Error::Incomplete)?;
        Ok(())
    }
}

/// 使用大端数字编码写入 Java 版 NBT 基本类型。
pub struct NbtWriteHelperJava<W: Write> {
    writer: W,
}

impl<W: Write> NbtWriteHelperJava<W> {
    /// 基于 `w` 创建 Java 版写入器。
    pub const fn new(w: W) -> Self {
        Self { writer: w }
    }
}

impl<W: Write> NbtWriteHelperJava<W> {
    define_write_number_be!(write_string_len, u16);
}

impl<W: Write> NbtWriteHelper for NbtWriteHelperJava<W> {
    type Writer = W;

    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.writer
    }

    define_write_number_be!(write_u8, u8);
    define_write_number_be!(write_i8, i8);
    define_write_number_be!(write_i16, i16);
    define_write_number_be!(write_i32, i32);
    define_write_number_be!(write_i64, i64);
    define_write_number_be!(write_f32, f32);
    define_write_number_be!(write_f64, f64);

    fn write_string(&mut self, value: &str) -> Result<()> {
        let mut java_string = cesu8::to_java_cesu8(value);
        if java_string.len() > u16::MAX as usize {
            // 拼接产物（如物品 CustomName 的拍平文本）可能超出 NBT
            // 的 u16 长度上限。此处若向上报错，`Nbt::write` 会吞掉
            // 错误并把截断的字节流当作完整文档写盘——下游无法解析，
            // 整区块数据会被判定损坏并重新生成。宁可按 CESU-8 边界
            // 截断这一个字符串（文档仍合法、仅该文本受损），也绝不
            // 损坏整份文档。
            let mut end = u16::MAX as usize;
            // 回退到多字节序列的起始边界
            while end > 0 && (java_string[end] & 0xC0) == 0x80 {
                end -= 1;
            }
            // 不得把增补字符的代理对劈成两半：高代理（ED A0-AF 起
            // 头的 3 字节序列）单独遗留会构成非法 CESU-8，整体丢弃。
            if end >= 3
                && java_string[end - 3] == 0xED
                && (0xA0..=0xAF).contains(&java_string[end - 2])
            {
                end -= 3;
            }
            java_string.to_mut().truncate(end);
        }

        let len = java_string.len();
        debug_assert!(u16::try_from(len).is_ok());

        self.write_string_len(len as u16)?;
        self.writer
            .write_all(&java_string)
            .map_err(Error::Incomplete)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::Nbt;
    use crate::deserializer::NbtReadHelperJava;

    /// 超长字符串必须按 CESU-8 边界截断而不是报错：
    /// `Nbt::write` 会吞掉序列化错误并把半截流写盘，
    /// 整份文档将无法解析（下游按损坏区块重新生成）。
    #[test]
    fn oversized_string_is_truncated_but_document_stays_parseable() {
        let mut compound = crate::NbtCompound::new();
        compound.put_string("text", "a".repeat(70_000));
        compound.put_int("after", 42);
        let nbt = Nbt {
            name: String::new(),
            root_tag: compound,
        };

        let bytes = nbt.write_unnamed();
        let mut reader = NbtReadHelperJava::new(Cursor::new(bytes.as_ref()));
        let parsed = Nbt::read_unnamed(&mut reader).expect("截断后仍应可解析");

        let text = parsed.get_string("text").expect("字段应存在");
        assert!(u16::try_from(text.len()).is_ok());
        assert_eq!(parsed.get_int("after"), Some(42), "后续字段不得丢失");
    }

    /// 截断点落在增补字符（6 字节 CESU-8 代理对）中间时，
    /// 必须整体丢弃高代理，不得产出非法 CESU-8。
    #[test]
    fn truncation_never_splits_surrogate_pair() {
        // '😀' 的 CESU-8 表示是 6 字节（高、低代理各 3 字节）。
        // 填充 65531 字节后，截断点恰落在第一个代理对内部，
        // 此时高代理的 3 字节必须被整体丢弃而不是单独遗留。
        let filler = "b".repeat(u16::MAX as usize - 4);
        let value = format!("{filler}😀😀😀😀");

        let mut compound = crate::NbtCompound::new();
        compound.put_string("text", value);
        let nbt = Nbt {
            name: String::new(),
            root_tag: compound,
        };

        let bytes = nbt.write_unnamed();
        let mut reader = NbtReadHelperJava::new(Cursor::new(bytes.as_ref()));
        let parsed = Nbt::read_unnamed(&mut reader).expect("不得留下劈开的代理对（否则无法解析）");
        let text = parsed.get_string("text").expect("字段应存在");
        assert!(u16::try_from(text.len()).is_ok());
        // 代理对之前被完整截断：不得出现替换符或残缺字符
        assert!(text.chars().all(|c| c != '\u{FFFD}'));
        assert!(text.chars().all(|c| c == 'b' || c == '😀'));
    }
}
