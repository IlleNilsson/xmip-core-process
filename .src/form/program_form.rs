//! A [`Program`]'s binary form: what a Work Process unit carries.

use assign::{Assignment, AssignmentValue};
use codec::cursor::Cursor;
use codec::writer::ByteWriter;
use codec::{CodecError, field};
use path::expression::Value;
use xcore::ScalarValue;

use super::{Form, number, numbers, read_number, read_numbers, read_texts, texts, unknown};
use crate::program::{Branch, Call, Catch, Due, Ending, Node, Program, Scope, Step, Timeout, Work};

impl Form for Program {
    fn write(&self, out: &mut Vec<u8>) {
        field::text(out, &self.name);
        field::many(out, &self.nodes, |out, node| node.write(out));
        field::many(out, &self.scopes, |out, scope| scope.write(out));
        numbers(out, &self.starts);
        texts(out, &self.sets);
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            name: field::read_text(cursor)?,
            nodes: field::read_many(cursor, Node::read)?,
            scopes: field::read_many(cursor, Scope::read)?,
            starts: read_numbers(cursor)?,
            sets: read_texts(cursor)?,
        })
    }
}

impl Form for Node {
    fn write(&self, out: &mut Vec<u8>) {
        field::text(out, &self.id);
        field::optional(out, self.scope, number);
        field::optional(out, self.set, number);
        self.step.write(out);
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            id: field::read_text(cursor)?,
            scope: field::read_optional(cursor, read_number)?,
            set: field::read_optional(cursor, read_number)?,
            step: Step::read(cursor)?,
        })
    }
}

impl Form for Step {
    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Start { next } => one(out, 0, *next),
            Self::Call { work, next } => {
                out.byte(1);
                work.write(out);
                number(out, *next);
            }
            Self::Publish { next } => one(out, 2, *next),
            Self::Wait { next } => one(out, 3, *next),
            Self::Batch { most, within, next } => {
                out.byte(4);
                number(out, *most);
                field::optional(out, within.as_ref(), |out, due| due.write(out));
                number(out, *next);
            }
            Self::Timer { due, next } => {
                out.byte(5);
                due.write(out);
                number(out, *next);
            }
            Self::Decide {
                branches,
                otherwise,
            } => {
                out.byte(6);
                field::many(out, branches, |out, branch| {
                    field::text(out, &branch.condition);
                    number(out, branch.next);
                });
                field::optional(out, *otherwise, number);
            }
            Self::Fork { next } => {
                out.byte(7);
                numbers(out, next);
            }
            Self::Join { from, next } => {
                out.byte(8);
                number(out, *from);
                number(out, *next);
            }
            Self::End { ending } => {
                out.byte(9);
                ending.write(out);
            }
        }
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(match cursor.byte()? {
            0 => Self::Start {
                next: read_number(cursor)?,
            },
            1 => Self::Call {
                work: Work::read(cursor)?,
                next: read_number(cursor)?,
            },
            2 => Self::Publish {
                next: read_number(cursor)?,
            },
            3 => Self::Wait {
                next: read_number(cursor)?,
            },
            4 => Self::Batch {
                most: read_number(cursor)?,
                within: field::read_optional(cursor, Due::read)?,
                next: read_number(cursor)?,
            },
            5 => Self::Timer {
                due: Due::read(cursor)?,
                next: read_number(cursor)?,
            },
            6 => Self::Decide {
                branches: field::read_many(cursor, |cursor| {
                    Ok(Branch {
                        condition: field::read_text(cursor)?,
                        next: read_number(cursor)?,
                    })
                })?,
                otherwise: field::read_optional(cursor, read_number)?,
            },
            7 => Self::Fork {
                next: read_numbers(cursor)?,
            },
            8 => Self::Join {
                from: read_number(cursor)?,
                next: read_number(cursor)?,
            },
            9 => Self::End {
                ending: Ending::read(cursor)?,
            },
            tag => return Err(unknown("step", tag)),
        })
    }
}

fn one(out: &mut Vec<u8>, tag: u8, next: u32) {
    out.byte(tag);
    number(out, next);
}

impl Form for Work {
    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Unit(call) => {
                out.byte(0);
                call.write(out);
            }
            Self::Assign(assignments) => {
                out.byte(1);
                field::many(out, assignments, |out, assignment| {
                    field::text(out, &assignment.target_key);
                    match &assignment.value {
                        AssignmentValue::Literal(value) => {
                            out.byte(0);
                            scalar(out, value);
                        }
                        AssignmentValue::Context(name) => {
                            out.byte(1);
                            field::text(out, name);
                        }
                    }
                });
            }
            Self::Debatch(call) => {
                out.byte(2);
                call.write(out);
            }
        }
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(match cursor.byte()? {
            0 => Self::Unit(Call::read(cursor)?),
            1 => Self::Assign(field::read_many(cursor, |cursor| {
                let target_key = field::read_text(cursor)?;
                let value = match cursor.byte()? {
                    0 => AssignmentValue::Literal(read_scalar(cursor)?),
                    1 => AssignmentValue::Context(field::read_text(cursor)?),
                    tag => return Err(unknown("assignment", tag)),
                };
                Ok(Assignment { target_key, value })
            })?),
            2 => Self::Debatch(Call::read(cursor)?),
            tag => return Err(unknown("work", tag)),
        })
    }
}

impl Form for Call {
    fn write(&self, out: &mut Vec<u8>) {
        field::text(out, &self.module);
        field::text(out, &self.unit);
        field::many(out, &self.settings, |out, (name, value)| {
            field::text(out, name);
            match value {
                Value::Text(text) => {
                    out.byte(0);
                    field::text(out, text);
                }
                Value::Integer(number) => {
                    out.byte(1);
                    out.i64_be(*number);
                }
                Value::Boolean(flag) => {
                    out.byte(2);
                    out.byte(u8::from(*flag));
                }
            }
        });
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            module: field::read_text(cursor)?,
            unit: field::read_text(cursor)?,
            settings: field::read_many(cursor, |cursor| {
                let name = field::read_text(cursor)?;
                let value = match cursor.byte()? {
                    0 => Value::Text(field::read_text(cursor)?),
                    1 => Value::Integer(cursor.i64_be()?),
                    2 => Value::Boolean(cursor.byte()? != 0),
                    tag => return Err(unknown("setting value", tag)),
                };
                Ok((name, value))
            })?,
        })
    }
}

/// A literal an assignment sets: text, a whole number or a truth, as the
/// expression language writes them; a design gives no other.
fn scalar(out: &mut Vec<u8>, value: &ScalarValue) {
    match value {
        ScalarValue::Bool(flag) => {
            out.byte(1);
            out.byte(u8::from(*flag));
        }
        ScalarValue::Integer(number) => {
            out.byte(2);
            out.i64_be(*number);
        }
        ScalarValue::Text(text) => {
            out.byte(3);
            field::text(out, text);
        }
        ScalarValue::Null | ScalarValue::Decimal(_) | ScalarValue::Binary(_) => {
            out.byte(0);
        }
    }
}

fn read_scalar(cursor: &mut Cursor<'_>) -> Result<ScalarValue, CodecError> {
    Ok(match cursor.byte()? {
        0 => ScalarValue::Null,
        1 => ScalarValue::Bool(cursor.byte()? != 0),
        2 => ScalarValue::Integer(cursor.i64_be()?),
        3 => ScalarValue::Text(field::read_text(cursor)?),
        tag => return Err(unknown("literal", tag)),
    })
}

impl Form for Due {
    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::After { nanos } => {
                out.byte(0);
                out.u64_be(*nanos);
            }
            Self::At { unix_nanos } => {
                out.byte(1);
                out.i128_be(*unix_nanos);
            }
        }
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(match cursor.byte()? {
            0 => Self::After {
                nanos: cursor.u64_be()?,
            },
            1 => Self::At {
                unix_nanos: cursor.i128_be()?,
            },
            tag => return Err(unknown("due time", tag)),
        })
    }
}

impl Form for Ending {
    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Flow => {
                out.byte(0);
            }
            Self::Terminate => {
                out.byte(1);
            }
            Self::Error { code } => {
                out.byte(2);
                field::text(out, code);
            }
        }
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(match cursor.byte()? {
            0 => Self::Flow,
            1 => Self::Terminate,
            2 => Self::Error {
                code: field::read_text(cursor)?,
            },
            tag => return Err(unknown("ending", tag)),
        })
    }
}

impl Form for Scope {
    fn write(&self, out: &mut Vec<u8>) {
        field::text(out, &self.id);
        field::optional(out, self.parent, number);
        field::many(out, &self.catches, |out, catch| {
            field::optional(out, catch.code.as_deref(), field::text);
            number(out, catch.next);
        });
        field::optional(out, self.timeout.as_ref(), |out, timeout| {
            timeout.due.write(out);
            number(out, timeout.next);
        });
    }

    fn read(cursor: &mut Cursor<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            id: field::read_text(cursor)?,
            parent: field::read_optional(cursor, read_number)?,
            catches: field::read_many(cursor, |cursor| {
                Ok(Catch {
                    code: field::read_optional(cursor, field::read_text)?,
                    next: read_number(cursor)?,
                })
            })?,
            timeout: field::read_optional(cursor, |cursor| {
                Ok(Timeout {
                    due: Due::read(cursor)?,
                    next: read_number(cursor)?,
                })
            })?,
        })
    }
}
