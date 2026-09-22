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
        let java_string = cesu8::to_java_cesu8(value);
        let len = java_string.len();
        if len > u16::MAX as usize {
            return Err(Error::LargeLength(len));
        }

        self.write_string_len(len as u16)?;
        self.writer
            .write_all(&java_string)
            .map_err(Error::Incomplete)?;
        Ok(())
    }
}
