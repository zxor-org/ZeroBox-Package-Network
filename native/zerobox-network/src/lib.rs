mod dhcp;
mod meter;
mod session;
mod tun;

use crate::session::{NetworkConfig, NetworkSession, WakeCallback};
use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{CStr, CString, c_char},
    path::PathBuf,
    ptr, slice,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::runtime::{Builder, Runtime};

pub const ABI_VERSION: u32 = 1;

const ZB_OK: i32 = 0;
const ZB_NO_EVENT: i32 = 1;
const ZB_INVALID_ARGUMENT: i32 = -1;
const ZB_NOT_FOUND: i32 = -2;
const ZB_BUFFER_TOO_SMALL: i32 = -3;
const ZB_INTERNAL: i32 = -4;

static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Builder::new_multi_thread()
        .thread_name("zerobox-network")
        .enable_all()
        .build()
        .expect("create ZeroBox network runtime")
});
static SESSIONS: Lazy<Mutex<HashMap<u64, Arc<NetworkSession>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

#[repr(C)]
pub struct ZbNetworkConfig {
    pub abi_version: u32,
    pub mtu: u16,
    pub reserved: u16,
    pub ingress_capacity: u32,
    pub stack_capacity: u32,
    pub outbound_capacity: u32,
    pub meter_window_ms: u32,
    pub statistics_interval_ms: u32,
    pub capture_path: *const c_char,
}

#[repr(C)]
pub struct ZbNetworkSnapshot {
    pub abi_version: u32,
    pub active: u8,
    pub reserved: [u8; 3],
    pub active_sessions: u32,
    pub bytes_from_device: u64,
    pub bytes_to_device: u64,
    pub dropped_packets: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn zb_network_abi_version() -> u32 {
    ABI_VERSION
}

#[unsafe(no_mangle)]
/// Opens a native network session.
///
/// # Safety
///
/// `config` and `out_handle` must point to valid values for the duration of
/// this call. A non-null `capture_path` must point to a valid NUL-terminated
/// string. `callback` must remain callable until `zb_network_close` returns.
pub unsafe extern "C" fn zb_network_open(
    config: *const ZbNetworkConfig,
    callback: Option<WakeCallback>,
    out_handle: *mut u64,
) -> i32 {
    ffi_result(|| {
        let config =
            unsafe { config.as_ref() }.ok_or_else(|| StatusError::invalid("config is null"))?;
        let callback = callback.ok_or_else(|| StatusError::invalid("wake callback is null"))?;
        let out_handle = unsafe { out_handle.as_mut() }
            .ok_or_else(|| StatusError::invalid("out_handle is null"))?;
        if config.abi_version != ABI_VERSION {
            return Err(StatusError::invalid(format!(
                "unsupported ABI version {}",
                config.abi_version
            ))
            .into());
        }

        let capture_path = if config.capture_path.is_null() {
            None
        } else {
            let value = unsafe { CStr::from_ptr(config.capture_path) };
            let value = value
                .to_str()
                .map_err(|_| StatusError::invalid("capture_path is not UTF-8"))?;
            Some(PathBuf::from(value))
        };
        let native_config = NetworkConfig {
            mtu: config.mtu.max(576),
            ingress_capacity: config.ingress_capacity.max(1) as usize,
            stack_capacity: config.stack_capacity.max(1) as usize,
            outbound_capacity: config.outbound_capacity.max(1) as usize,
            meter_window: Duration::from_millis(u64::from(config.meter_window_ms.max(100))),
            stats_interval: Duration::from_millis(u64::from(
                config.statistics_interval_ms.max(100),
            )),
            capture_path,
        };
        let handle = next_handle();
        let session = NetworkSession::start(native_config, callback, RUNTIME.handle())?;
        session.set_handle(handle);
        SESSIONS.lock().insert(handle, session);
        *out_handle = handle;
        Ok(())
    })
}

#[unsafe(no_mangle)]
/// Copies one raw IPv4 packet into a network session.
///
/// # Safety
///
/// When `length` is non-zero, `data` must point to at least `length` readable
/// bytes for the duration of this call.
pub unsafe extern "C" fn zb_network_push(handle: u64, data: *const u8, length: usize) -> i32 {
    ffi_result(|| {
        if data.is_null() && length != 0 {
            return Err(StatusError::invalid("data is null").into());
        }
        let packet = if length == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(data, length) }
        };
        with_session(handle, |session| session.push_inbound(packet))
    })
}

#[unsafe(no_mangle)]
/// Reads the metadata of the next queued event without removing it.
///
/// # Safety
///
/// `out_kind` and `out_length` must point to writable values.
pub unsafe extern "C" fn zb_network_event_peek(
    handle: u64,
    out_kind: *mut u32,
    out_length: *mut usize,
) -> i32 {
    ffi_result(|| {
        let out_kind =
            unsafe { out_kind.as_mut() }.ok_or_else(|| StatusError::invalid("out_kind is null"))?;
        let out_length = unsafe { out_length.as_mut() }
            .ok_or_else(|| StatusError::invalid("out_length is null"))?;
        with_session(handle, |session| {
            if let Some((kind, length)) = session.peek_event() {
                *out_kind = kind as u32;
                *out_length = length;
                Ok(())
            } else {
                Err(StatusError::new(ZB_NO_EVENT, "event queue is empty").into())
            }
        })
    })
}

#[unsafe(no_mangle)]
/// Copies and removes the next queued event.
///
/// # Safety
///
/// `out_kind` and `out_length` must point to writable values. When the event
/// payload is non-empty, `buffer` must point to at least `capacity` writable
/// bytes.
pub unsafe extern "C" fn zb_network_event_read(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
    out_kind: *mut u32,
    out_length: *mut usize,
) -> i32 {
    ffi_result(|| {
        let out_kind =
            unsafe { out_kind.as_mut() }.ok_or_else(|| StatusError::invalid("out_kind is null"))?;
        let out_length = unsafe { out_length.as_mut() }
            .ok_or_else(|| StatusError::invalid("out_length is null"))?;
        with_session(handle, |session| {
            let Some((kind, length)) = session.peek_event() else {
                return Err(StatusError::new(ZB_NO_EVENT, "event queue is empty").into());
            };
            *out_kind = kind as u32;
            *out_length = length;
            if capacity < length || (length != 0 && buffer.is_null()) {
                return Err(
                    StatusError::new(ZB_BUFFER_TOO_SMALL, "event buffer is too small").into(),
                );
            }
            let event = session
                .pop_event()
                .context("event queue changed while reading")?;
            if length != 0 {
                unsafe { ptr::copy_nonoverlapping(event.payload.as_ptr(), buffer, length) };
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
/// Copies the current session counters into `out_snapshot`.
///
/// # Safety
///
/// `out_snapshot` must point to a writable `ZbNetworkSnapshot`.
pub unsafe extern "C" fn zb_network_get_snapshot(
    handle: u64,
    out_snapshot: *mut ZbNetworkSnapshot,
) -> i32 {
    ffi_result(|| {
        let output = unsafe { out_snapshot.as_mut() }
            .ok_or_else(|| StatusError::invalid("out_snapshot is null"))?;
        with_session(handle, |session| {
            let counters = &session.counters;
            *output = ZbNetworkSnapshot {
                abi_version: ABI_VERSION,
                active: 1,
                reserved: [0; 3],
                active_sessions: counters.active_sessions.load(Ordering::Relaxed) as u32,
                bytes_from_device: counters.bytes_from_device.load(Ordering::Relaxed),
                bytes_to_device: counters.bytes_to_device.load(Ordering::Relaxed),
                dropped_packets: counters.dropped_packets.load(Ordering::Relaxed),
            };
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn zb_network_close(handle: u64) -> i32 {
    ffi_result(|| {
        let session = SESSIONS
            .lock()
            .remove(&handle)
            .ok_or_else(|| StatusError::not_found(handle))?;
        session.close();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn zb_network_last_error() -> *const c_char {
    LAST_ERROR.with(|value| value.borrow().as_ptr())
}

fn with_session<T>(
    handle: u64,
    operation: impl FnOnce(&Arc<NetworkSession>) -> Result<T>,
) -> Result<T> {
    let session = SESSIONS
        .lock()
        .get(&handle)
        .cloned()
        .ok_or_else(|| StatusError::not_found(handle))?;
    operation(&session)
}

fn next_handle() -> u64 {
    loop {
        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        if handle != 0 && !SESSIONS.lock().contains_key(&handle) {
            return handle;
        }
    }
}

fn ffi_result(operation: impl FnOnce() -> Result<()>) -> i32 {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(Ok(())) => ZB_OK,
        Ok(Err(error)) => {
            let status = error
                .downcast_ref::<StatusError>()
                .map_or(ZB_INTERNAL, |error| error.status);
            set_last_error(format!("{error:#}"));
            status
        }
        Err(_) => {
            set_last_error("native panic".to_owned());
            ZB_INTERNAL
        }
    }
}

fn set_last_error(message: String) {
    let message = message.replace('\0', "�");
    LAST_ERROR.with(|value| *value.borrow_mut() = CString::new(message).unwrap_or_default());
}

#[derive(Debug)]
struct StatusError {
    status: i32,
    message: String,
}

impl StatusError {
    fn new(status: i32, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(ZB_INVALID_ARGUMENT, message)
    }

    fn not_found(handle: u64) -> Self {
        Self::new(ZB_NOT_FOUND, format!("network session {handle} not found"))
    }
}

impl std::fmt::Display for StatusError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for StatusError {}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn wake(_: u64) {}

    fn config() -> ZbNetworkConfig {
        ZbNetworkConfig {
            abi_version: ABI_VERSION,
            mtu: 800,
            reserved: 0,
            ingress_capacity: 8,
            stack_capacity: 8,
            outbound_capacity: 8,
            meter_window_ms: 1000,
            statistics_interval_ms: 1000,
            capture_path: ptr::null(),
        }
    }

    #[test]
    fn exports_current_abi_version() {
        assert_eq!(zb_network_abi_version(), ABI_VERSION);
    }

    #[test]
    fn opens_snapshots_and_closes_a_session() {
        let mut handle = 0;
        assert_eq!(
            unsafe { zb_network_open(&config(), Some(wake), &mut handle) },
            ZB_OK
        );
        assert_ne!(handle, 0);

        let mut snapshot = ZbNetworkSnapshot {
            abi_version: 0,
            active: 0,
            reserved: [0; 3],
            active_sessions: 0,
            bytes_from_device: 0,
            bytes_to_device: 0,
            dropped_packets: 0,
        };
        assert_eq!(
            unsafe { zb_network_get_snapshot(handle, &mut snapshot) },
            ZB_OK
        );
        assert_eq!(snapshot.abi_version, ABI_VERSION);
        assert_eq!(snapshot.active, 1);
        assert_eq!(zb_network_close(handle), ZB_OK);
        assert_eq!(zb_network_close(handle), ZB_NOT_FOUND);
    }

    #[test]
    fn rejects_an_incompatible_abi() {
        let mut config = config();
        config.abi_version += 1;
        let mut handle = 0;
        assert_eq!(
            unsafe { zb_network_open(&config, Some(wake), &mut handle) },
            ZB_INVALID_ARGUMENT
        );
        assert_eq!(handle, 0);
    }
}
