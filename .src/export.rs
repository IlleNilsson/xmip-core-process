//! A Work Process unit, loadable: the entry point and the work process
//! table `include/xmip_module.h` section 13 declares, over any [`Flow`].
//!
//! A unit generated from a design names its program's bytes
//! ([`crate::generate`]); a unit written by hand names its flow:
//!
//! ```ignore
//! process::export_work_process!(PROGRAM, unit = "OrderApproval", version = (0, 1, 0));
//! process::export_work_process!(flow = MyFlow, unit = "Mine", version = (1, 0, 0));
//! ```
//!
//! and is built as a `cdylib`. A node opens it, resolves
//! `xmip_create_module_v1` and calls `advance` once a turn; the unit's crate
//! writes no `unsafe`. A panic is caught and answered as `PANIC`, never
//! unwound into the host; a program that does not hold together is refused
//! as the module is created, `MALFORMED` with why in `last_error`.

#![allow(unsafe_code)]

use core::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use xmip_core_abi::XMIP_ABI_VERSION;
use xmip_core_abi::ffi::{
    Host, Module, Slice, Str, VtableHeader, WireDescriptor, WorkProcessVtable, status,
};

use crate::flow::{Compiled, Flow};
use crate::form::{self, read_turn, write_acts};

/// The work process table's version this export implements.
pub const TRAIT_MAJOR: u32 = 1;
/// See [`TRAIT_MAJOR`].
pub const TRAIT_MINOR: u32 = 0;

/// What the descriptor names the trait by.
pub const MODULE: &str = "work-process";

/// The unit's name and its module's version, as the descriptor carries
/// them.
#[derive(Clone, Copy, Debug)]
pub struct Identity {
    pub unit: &'static str,
    pub version: (u32, u32, u32),
}

struct State {
    flow: Box<dyn Flow>,
    declared: Vec<u8>,
    table: WorkProcessVtable,
    error: String,
}

/// The flow a generated unit's program bytes make.
///
/// # Errors
/// The bytes are no program, or the program does not hold together.
pub fn compiled(program: &[u8]) -> Result<Box<dyn Flow>, String> {
    let program = form::read(program).map_err(|why| why.to_string())?;
    Compiled::new(program)
        .map(|compiled| Box::new(compiled) as Box<dyn Flow>)
        .map_err(|problems| problems.join("; "))
}

/// The entry point's body. [`export_work_process!`](crate::export_work_process)
/// calls it from the one exported symbol.
///
/// # Safety
/// `host` and `out` must be null or valid for the call, as the header says of
/// `xmip_create_module_v1`. A null pointer or a foreign ABI version is
/// refused, never dereferenced into.
pub unsafe fn create(
    host: *const c_void,
    out: *mut c_void,
    flow: Result<Box<dyn Flow>, String>,
    identity: Identity,
) -> i32 {
    let (host, out) = (host.cast::<Host>(), out.cast::<Module>());
    if host.is_null() || out.is_null() {
        return status::INVALID;
    }
    // SAFETY: host is non-null and the caller vouches for it for the call.
    if unsafe { (*host).abi_version } != XMIP_ABI_VERSION {
        return status::UNSUPPORTED;
    }
    let flow = match flow {
        Ok(flow) => flow,
        Err(_) => return status::MALFORMED,
    };
    let declared = form::written(flow.declared());
    let state = Box::into_raw(Box::new(State {
        flow,
        declared,
        table: table(),
        error: String::new(),
    }));
    let (major, minor, patch) = identity.version;
    // SAFETY: out is non-null and the caller vouches for it; state was just
    // made, and the table lives in it until `destroy`.
    unsafe {
        *out = Module {
            descriptor: WireDescriptor {
                abi_version: XMIP_ABI_VERSION,
                provider: Str::from_static(identity.unit),
                module: Str::from_static(MODULE),
                standard: Str::from_static(identity.unit),
                trait_major: TRAIT_MAJOR,
                trait_minor: TRAIT_MINOR,
                module_major: major,
                module_minor: minor,
                module_patch: patch,
            },
            state: state.cast(),
            vtable: (&raw const (*state).table).cast(),
            last_error: Some(last_error),
            destroy: Some(destroy),
        };
    }
    status::OK
}

/// Export a Work Process unit: the one symbol a node resolves,
/// `xmip_create_module_v1`, over a generated program's bytes or a flow
/// written by hand, named by its unit and its module's version.
#[macro_export]
macro_rules! export_work_process {
    (flow = $flow:expr, unit = $unit:literal,
     version = ($major:literal, $minor:literal, $patch:literal)) => {
        $crate::export_work_process!(@export
            Ok(Box::new($flow) as Box<dyn $crate::flow::Flow>), $unit, $major, $minor, $patch);
    };
    ($program:expr, unit = $unit:literal,
     version = ($major:literal, $minor:literal, $patch:literal)) => {
        $crate::export_work_process!(@export
            $crate::export::compiled($program), $unit, $major, $minor, $patch);
    };
    (@export $flow:expr, $unit:literal, $major:literal, $minor:literal, $patch:literal) => {
        /// The module's entry point, header section 1.
        ///
        /// # Safety
        /// Called by a host across the C boundary; see
        /// `process::export::create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn xmip_create_module_v1(
            host: *const ::core::ffi::c_void,
            out: *mut ::core::ffi::c_void,
        ) -> i32 {
            // Evaluated here, outside the unsafe block, so a caller's
            // expression is never run with unsafe permitted.
            let flow = $flow;
            let identity = $crate::export::Identity {
                unit: $unit,
                version: ($major, $minor, $patch),
            };
            // SAFETY: the host's pointers are passed through unchanged.
            unsafe { $crate::export::create(host, out, flow, identity) }
        }
    };
}

const fn table() -> WorkProcessVtable {
    WorkProcessVtable {
        header: VtableHeader {
            trait_major: TRAIT_MAJOR,
            trait_minor: TRAIT_MINOR,
            configure: Some(configure),
            start: Some(lifecycle),
            stop: Some(lifecycle),
        },
        declared: Some(declared),
        advance: Some(advance),
    }
}

/// Run `call`, answering `PANIC` rather than unwinding into the host.
fn guarded(call: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(call)).unwrap_or(status::PANIC)
}

unsafe extern "C" fn configure(_: *mut c_void, _: Str) -> i32 {
    status::OK
}

unsafe extern "C" fn lifecycle(_: *mut c_void) -> i32 {
    status::OK
}

unsafe extern "C" fn declared(state: *mut c_void, out: *mut Slice) -> i32 {
    if state.is_null() || out.is_null() {
        return status::INVALID;
    }
    guarded(|| {
        // SAFETY: state is the one `create` made, alive until `destroy`.
        let state = unsafe { &*state.cast::<State>() };
        // SAFETY: out is non-null and the caller's for the call.
        unsafe {
            *out = Slice {
                ptr: state.declared.as_ptr(),
                len: state.declared.len(),
            };
        }
        status::OK
    })
}

unsafe extern "C" fn advance(
    state: *mut c_void,
    turn: Slice,
    out: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if state.is_null() || out_len.is_null() || (turn.ptr.is_null() && turn.len > 0) {
        return status::INVALID;
    }
    guarded(|| {
        // SAFETY: state is the one `create` made; it is only read here.
        let state = unsafe { &*state.cast::<State>() };
        let bytes = if turn.len == 0 {
            &[][..]
        } else {
            // SAFETY: the header guarantees ptr..ptr+len for the call.
            unsafe { core::slice::from_raw_parts(turn.ptr, turn.len) }
        };
        let Ok((mut instance, event, now)) = read_turn(bytes) else {
            return status::MALFORMED;
        };
        let answer = state
            .flow
            .advance(&mut instance, &event, now)
            .map(|acts| (instance, acts));
        let written = write_acts(&answer);
        // SAFETY: out_len is non-null and the caller's for the call.
        unsafe { *out_len = written.len() };
        if written.len() > cap || out.is_null() {
            return status::CAPACITY;
        }
        // SAFETY: out holds cap bytes, and written fits in them.
        unsafe { core::ptr::copy_nonoverlapping(written.as_ptr(), out, written.len()) };
        status::OK
    })
}

unsafe extern "C" fn last_error(state: *mut c_void) -> Str {
    if state.is_null() {
        return Str::empty();
    }
    // SAFETY: state is the one `create` made, alive until `destroy`.
    let state = unsafe { &*state.cast::<State>() };
    Str {
        ptr: state.error.as_ptr(),
        len: state.error.len(),
    }
}

unsafe extern "C" fn destroy(state: *mut c_void) {
    if !state.is_null() {
        // SAFETY: state is the one `create` made, given back once.
        drop(unsafe { Box::from_raw(state.cast::<State>()) });
    }
}
