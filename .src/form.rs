//! The binary forms a Work Process crosses in: its [`Program`] inside the
//! unit, its [`Instance`] in the Ledger, and a turn's event and acts over
//! the C ABI (`include/xmip_module.h` section 13). Each opens with its
//! version, so a form written by a later Xmip is refused in words, never
//! misread. Written with `xmip-core-library-codec`'s fields, as a Message's
//! and a Journey's forms are.

mod program_form;
mod turn_form;

use codec::cursor::Cursor;
use codec::writer::ByteWriter;
use codec::{CodecError, field};

pub use turn_form::{read_acts, read_turn, write_acts, write_turn};

use crate::instance::Instance;
use crate::program::Program;

/// The version every form here opens with.
pub const FORM_VERSION: u8 = 1;

/// A value written and read back in its binary form.
pub trait Form: Sized {
    fn write(&self, out: &mut Vec<u8>);

    /// # Errors
    /// The bytes are cut short or say what no value of it is.
    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError>;
}

/// `value` in its form, after the version.
#[must_use]
pub fn written<T: Form>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    out.byte(FORM_VERSION);
    value.write(&mut out);
    out
}

/// A value [`written`] wrote, every byte of it read.
///
/// # Errors
/// Another version, bytes cut short or left over, or a value no `T` is.
pub fn read<T: Form>(bytes: &[u8]) -> Result<T, CodecError> {
    let mut cursor = Cursor::new(bytes);
    let version = cursor.byte()?;
    if version != FORM_VERSION {
        return Err(CodecError::new(format!(
            "a Work Process form of version {version}, where this Xmip reads {FORM_VERSION}"
        )));
    }
    let value = T::read(&mut cursor)?;
    if !cursor.is_empty() {
        return Err(CodecError::new("bytes left after a Work Process form"));
    }
    Ok(value)
}

/// A program's form.
#[must_use]
pub fn program(program: &Program) -> Vec<u8> {
    written(program)
}

/// An Instance's form.
#[must_use]
pub fn instance(instance: &Instance) -> Vec<u8> {
    written(instance)
}

pub(crate) fn number(out: &mut Vec<u8>, value: u32) {
    out.varint(u64::from(value));
}

pub(crate) fn read_number(cursor: &mut Cursor<'_>) -> Result<u32, CodecError> {
    u32::try_from(cursor.varint()?).map_err(|_| CodecError::new("a number past 32 bits"))
}

pub(crate) fn numbers(out: &mut Vec<u8>, values: &[u32]) {
    field::many(out, values, |out, value| number(out, *value));
}

pub(crate) fn read_numbers(cursor: &mut Cursor<'_>) -> Result<Vec<u32>, CodecError> {
    field::read_many(cursor, read_number)
}

pub(crate) fn texts(out: &mut Vec<u8>, values: &[String]) {
    field::many(out, values, |out, value| field::text(out, value));
}

pub(crate) fn read_texts(cursor: &mut Cursor<'_>) -> Result<Vec<String>, CodecError> {
    field::read_many(cursor, field::read_text)
}

pub(crate) fn pairs(out: &mut Vec<u8>, values: &[(String, String)]) {
    field::many(out, values, |out, (name, value)| {
        field::text(out, name);
        field::text(out, value);
    });
}

pub(crate) fn read_pairs(cursor: &mut Cursor<'_>) -> Result<Vec<(String, String)>, CodecError> {
    field::read_many(cursor, |cursor| {
        Ok((field::read_text(cursor)?, field::read_text(cursor)?))
    })
}

pub(crate) fn unknown(what: &str, tag: u8) -> CodecError {
    CodecError::new(format!("no {what} is numbered {tag}"))
}
