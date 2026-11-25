//! A turn's binary form — the Instance and its event in, the Instance and
//! the acts out — as it crosses the C ABI, and what a unit declares.

use codec::cursor::Cursor;
use codec::form::{Form, opened};
use codec::writer::ByteWriter;
use codec::{CodecError, field};

use super::instance_form::{keys, read_keys};
use super::{
    FORM_VERSION, FORMS, number, pairs, read_number, read_pairs, read_texts, texts, unknown,
    written,
};
use crate::flow::{Act, Arrival, Declared, Event, Refused};
use crate::instance::{Instance, Outcome};
use crate::program::Work;

impl Form for Declared {
    fn write(&self, out: &mut Vec<u8>) {
        field::text(out, &self.name);
        texts(out, &self.sets);
        texts(out, &self.reads);
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            name: field::read_text(cursor)?,
            sets: read_texts(cursor)?,
            reads: read_texts(cursor)?,
        })
    }
}

impl Form for Arrival {
    fn write(&self, out: &mut Vec<u8>) {
        out.u128_be(self.message);
        pairs(out, &self.values);
        keys(out, &self.keys);
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            message: cursor.u128_be()?,
            values: read_pairs(cursor)?,
            keys: read_keys(cursor)?,
        })
    }
}

impl Form for Event {
    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Arrived(arrival) => {
                out.byte(0);
                arrival.write(out);
            }
            Self::Done { node, made } => {
                out.byte(1);
                number(out, *node);
                field::many(out, made, |out, arrival| arrival.write(out));
            }
            Self::Threw { node, code, reason } => {
                out.byte(2);
                number(out, *node);
                field::text(out, code);
                field::text(out, reason);
            }
            Self::Due => {
                out.byte(3);
            }
        }
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(match cursor.byte()? {
            0 => Self::Arrived(Arrival::read(cursor)?),
            1 => Self::Done {
                node: read_number(cursor)?,
                made: field::read_many(cursor, Arrival::read)?,
            },
            2 => Self::Threw {
                node: read_number(cursor)?,
                code: field::read_text(cursor)?,
                reason: field::read_text(cursor)?,
            },
            3 => Self::Due,
            tag => return Err(unknown("event", tag)),
        })
    }
}

impl Form for Act {
    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Work {
                node,
                work,
                message,
            } => {
                out.byte(0);
                number(out, *node);
                work.write(out);
                out.u128_be(*message);
            }
            Self::Combine { node, messages } => {
                out.byte(1);
                number(out, *node);
                field::many(out, messages, |out, message| {
                    out.u128_be(*message);
                });
            }
            Self::Publish { message } => {
                out.byte(2);
                out.u128_be(*message);
            }
            Self::Ended(outcome) => {
                out.byte(3);
                outcome.write(out);
            }
        }
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(match cursor.byte()? {
            0 => Self::Work {
                node: read_number(cursor)?,
                work: Work::read(cursor)?,
                message: cursor.u128_be()?,
            },
            1 => Self::Combine {
                node: read_number(cursor)?,
                messages: field::read_many(cursor, Cursor::u128_be)?,
            },
            2 => Self::Publish {
                message: cursor.u128_be()?,
            },
            3 => Self::Ended(Outcome::read(cursor)?),
            tag => return Err(unknown("act", tag)),
        })
    }
}

/// What a turn hands a unit across the C ABI: the Instance, the event and
/// the time.
#[must_use]
pub fn write_turn(instance: &Instance, event: &Event, now_unix_nanos: i128) -> Vec<u8> {
    let mut out = written(instance);
    event.write(&mut out);
    out.i128_be(now_unix_nanos);
    out
}

/// A turn [`write_turn`] wrote.
///
/// # Errors
/// As [`super::read`], for any of its parts.
pub fn read_turn(bytes: &[u8]) -> Result<(Instance, Event, i128), CodecError> {
    let mut cursor = Cursor::new(bytes);
    opened(&mut cursor, FORM_VERSION, FORMS)?;
    let instance = Instance::read(&mut cursor)?;
    let event = Event::read(&mut cursor)?;
    let now = cursor.i128_be()?;
    if !cursor.is_empty() {
        return Err(CodecError::new("bytes left after a turn"));
    }
    Ok((instance, event, now))
}

/// What a unit answers a turn with across the C ABI: the Instance as the
/// turn left it and its acts, or why it refused.
#[must_use]
pub fn write_acts(answer: &Result<(Instance, Vec<Act>), Refused>) -> Vec<u8> {
    let mut out = vec![FORM_VERSION];
    match answer {
        Ok((instance, acts)) => {
            out.byte(0);
            instance.write(&mut out);
            field::many(&mut out, acts, |out, act| act.write(out));
        }
        Err(Refused::NotWaiting) => {
            out.byte(1);
        }
        Err(Refused::Ended) => {
            out.byte(2);
        }
        Err(Refused::Unfit(why)) => {
            out.byte(3);
            field::text(&mut out, why);
        }
    }
    out
}

/// An answer [`write_acts`] wrote.
///
/// # Errors
/// As [`super::read`].
#[allow(clippy::type_complexity)]
pub fn read_acts(bytes: &[u8]) -> Result<Result<(Instance, Vec<Act>), Refused>, CodecError> {
    let mut cursor = Cursor::new(bytes);
    opened(&mut cursor, FORM_VERSION, FORMS)?;
    let answer = match cursor.byte()? {
        0 => Ok((
            Instance::read(&mut cursor)?,
            field::read_many(&mut cursor, Act::read)?,
        )),
        1 => Err(Refused::NotWaiting),
        2 => Err(Refused::Ended),
        3 => Err(Refused::Unfit(field::read_text(&mut cursor)?)),
        tag => return Err(unknown("answer", tag)),
    };
    if !cursor.is_empty() {
        return Err(CodecError::new("bytes left after a turn's answer"));
    }
    Ok(answer)
}
