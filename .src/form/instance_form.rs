//! An [`Instance`]'s binary form, as the Ledger keeps it and a turn hands
//! it across the C ABI: where its flows stand, its timers, the current
//! Message and what was read of it, its own values, how it ended and its
//! history of Messages.

use codec::cursor::Cursor;
use codec::form::Form;
use codec::writer::ByteWriter;
use codec::{CodecError, field};

use super::{number, pairs, read_number, read_pairs, unknown};
use crate::instance::{
    HANDLINGS, Handled, Handling, Instance, Outcome, Owner, Standing, Timer, Token,
};

pub(super) fn keys(out: &mut Vec<u8>, keys: &[(u32, String)]) {
    field::many(out, keys, |out, (set, key)| {
        number(out, *set);
        field::text(out, key);
    });
}

pub(super) fn read_keys(cursor: &mut Cursor<'_>) -> Result<Vec<(u32, String)>, CodecError> {
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
        pairs(out, &self.read);
        pairs(out, &self.values);
        field::optional(out, self.ended.as_ref(), |out, ended| ended.write(out));
        out.u64_be(self.next_timer);
        out.u64_be(self.turns);
        field::many(out, &self.history, |out, entry| entry.write(out));
        out.u64_be(self.earlier);
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
            read: read_pairs(cursor)?,
            values: read_pairs(cursor)?,
            ended: field::read_optional(cursor, Outcome::read)?,
            next_timer: cursor.u64_be()?,
            turns: cursor.u64_be()?,
            history: field::read_many(cursor, Handled::read)?,
            earlier: cursor.u64_be()?,
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

impl Form for Handled {
    fn write(&self, out: &mut Vec<u8>) {
        out.i128_be(self.at_unix_nanos);
        let tag = HANDLINGS
            .iter()
            .position(|what| *what == self.what)
            .unwrap_or_default();
        out.byte(u8::try_from(tag).unwrap_or_default());
        out.u128_be(self.message);
        out.u128_be(self.journey);
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        let at_unix_nanos = cursor.i128_be()?;
        let tag = cursor.byte()?;
        let what: Handling = *HANDLINGS
            .get(usize::from(tag))
            .ok_or_else(|| unknown("handling", tag))?;
        Ok(Self {
            at_unix_nanos,
            what,
            message: cursor.u128_be()?,
            journey: cursor.u128_be()?,
        })
    }
}
