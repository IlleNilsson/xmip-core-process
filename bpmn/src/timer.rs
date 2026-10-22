//! A BPMN timer: a `timeDuration` or a `timeDate`, read by the estate's one
//! calendar (`codec::civil`). A `timeCycle` repeats and is refused in this
//! version.

use codec::civil::{read_duration, read_rfc3339};
use process::Refusal;
use process::program::Due;

use crate::document::Element;
use crate::schema::MODEL;

/// When the timer `definition` is due.
///
/// # Errors
/// It is a cycle, gives no time, or one that does not read.
pub fn due(at: &Element, definition: &Element) -> Result<Due, Refusal> {
    if let Some(duration) = definition.child(MODEL, "timeDuration") {
        return self::duration(at, duration.text.trim());
    }
    if let Some(date) = definition.child(MODEL, "timeDate") {
        let text = date.text.trim();
        return read_rfc3339(text)
            .map(|unix_nanos| Due::At { unix_nanos })
            .ok_or_else(|| {
                Refusal::new(
                    at.label(),
                    format!("its timeDate '{text}' is no RFC 3339 time"),
                )
            });
    }
    if definition.child(MODEL, "timeCycle").is_some() {
        return Err(Refusal::new(
            at.label(),
            "a timer that repeats is not supported in this version",
        ));
    }
    Err(Refusal::new(at.label(), "its timer gives no time"))
}

/// An ISO 8601 duration.
///
/// # Errors
/// It does not read as one, or counts years or months.
pub fn duration(at: &Element, text: &str) -> Result<Due, Refusal> {
    read_duration(text)
        .map(|nanos| Due::After { nanos })
        .ok_or_else(|| {
            Refusal::new(
                at.label(),
                format!(
                    "'{text}' is no ISO 8601 duration of weeks, days, hours, minutes and \
                     seconds"
                ),
            )
        })
}
