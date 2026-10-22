//! An [`Instance`]'s binary form, as the Ledger keeps it, and a turn's —
//! the Instance and its event in, the Instance and the acts out — as they
//! cross the C ABI.

use codec::cursor::Cursor;
use codec::writer::ByteWriter;
use codec::{CodecError, field};

use super::{
    FORM_VERSION, Form, number, pairs, read_number, read_pairs, read_texts, texts, unknown, written,
};
use crate::flow::{Act, Arrival, Declared, Event, Refused};
use crate::instance::{Instance, Outcome, Owner, Standing, Timer, Token};
use crate::program::Work;

fn keys(out: &mut Vec<u8>, keys: &[(u32, String)]) {
    field::many(out, keys, |out, (set, key)| {
        number(out, *set);
        field::text(out, key);
    });
}

fn read_keys(cursor: &mut Cursor<'_>) -> Result<Vec<(u32, String)>, CodecError> {
    field::read_many(cursor, |cursor| {
        Ok((read_number(cursor)?, field::read_text(cursor)?))
    })
}

impl Form for Instance {
    fn write(&self, out: &mut Vec<u8>) {
        keys(out, &self.keys);
        field::many(out, &self.tokens, |out, token| {
            number(out, token.at);
            out.byte(match token.standing {
                Standing::Ready => 0,
                Standing::Message => 1,
                Standing::Timer => 2,
                Standing::Work => 3,
            });
        });
        field::many(out, &self.joins, |out, (join, arrived)| {
            number(out, *join);
            number(out, *arrived);
        });
        field::many(out, &self.timers, |out, timer| {
            out.u64_be(timer.id);
            let (tag, at) = match timer.owner {
                Owner::Node(node) => (0, node),
                Owner::Scope(scope) => (1, scope),
            };
            out.byte(tag);
            number(out, at);
            out.i128_be(timer.due_unix_nanos);
        });
        field::many(out, &self.batch, |out, message| {
            out.u128_be(*message);
        });
        field::optional(out, self.message, |out, message| {
            out.u128_be(message);
        });
        pairs(out, &self.values);
        field::optional(out, self.ended.as_ref(), |out, ended| ended.write(out));
        out.u64_be(self.next_timer);
        out.u64_be(self.turns);
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            keys: read_keys(cursor)?,
            tokens: field::read_many(cursor, |cursor| {
                let at = read_number(cursor)?;
                let standing = match cursor.byte()? {
                    0 => Standing::Ready,
                    1 => Standing::Message,
                    2 => Standing::Timer,
                    3 => Standing::Work,
                    tag => return Err(unknown("standing", tag)),
                };
                Ok(Token { at, standing })
            })?,
            joins: field::read_many(cursor, |cursor| {
                Ok((read_number(cursor)?, read_number(cursor)?))
            })?,
            timers: field::read_many(cursor, |cursor| {
                let id = cursor.u64_be()?;
                let owner = match cursor.byte()? {
                    0 => Owner::Node(read_number(cursor)?),
                    1 => Owner::Scope(read_number(cursor)?),
                    tag => return Err(unknown("timer owner", tag)),
                };
                Ok(Timer {
                    id,
                    owner,
                    due_unix_nanos: cursor.i128_be()?,
                })
            })?,
            batch: field::read_many(cursor, Cursor::u128_be)?,
            message: field::read_optional(cursor, Cursor::u128_be)?,
            values: read_pairs(cursor)?,
            ended: field::read_optional(cursor, Outcome::read)?,
            next_timer: cursor.u64_be()?,
            turns: cursor.u64_be()?,
        })
    }
}

impl Form for Outcome {
    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Completed => {
                out.byte(0);
            }
            Self::Failed { code, reason } => {
                out.byte(1);
                field::text(out, code);
                field::text(out, reason);
            }
        }
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(match cursor.byte()? {
            0 => Self::Completed,
            1 => Self::Failed {
                code: field::read_text(cursor)?,
                reason: field::read_text(cursor)?,
            },
            tag => return Err(unknown("outcome", tag)),
        })
    }
}

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
    let version = cursor.byte()?;
    if version != FORM_VERSION {
        return Err(other_version(version));
    }
    let instance = Instance::read(&mut cursor)?;
    let event = Event::read(&mut cursor)?;
    let now = cursor.i128_be()?;
    if !cursor.is_empty() {
        return Err(CodecError::new("bytes left after a turn"));
    }
    Ok((instance, event, now))
}

fn other_version(version: u8) -> CodecError {
    CodecError::new(format!(
        "a Work Process form of version {version}, where this Xmip reads {FORM_VERSION}"
    ))
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
    let version = cursor.byte()?;
    if version != FORM_VERSION {
        return Err(other_version(version));
    }
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
