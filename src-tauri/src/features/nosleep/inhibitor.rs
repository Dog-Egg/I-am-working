//! Platform sleep-inhibition support for the nosleep feature.

use std::sync::{Mutex, OnceLock};

static SLEEP_INHIBITOR: OnceLock<Mutex<Option<SleepInhibitor>>> = OnceLock::new();

fn sleep_inhibitor() -> &'static Mutex<Option<SleepInhibitor>> {
    SLEEP_INHIBITOR.get_or_init(|| Mutex::new(None))
}

pub(crate) fn sync_sleep_inhibitor(should_keep_awake: bool) {
    if should_keep_awake {
        start_sleep_inhibitor();
    } else {
        stop_sleep_inhibitor();
    }
}

fn start_sleep_inhibitor() {
    let mut inhibitor = sleep_inhibitor().lock().unwrap();
    if inhibitor.is_some() {
        return;
    }

    match SleepInhibitor::new() {
        Ok(sleep_inhibitor) => {
            *inhibitor = Some(sleep_inhibitor);
        }
        Err(err) => eprintln!("failed to start sleep inhibitor: {err}"),
    }
}

pub(crate) fn stop_sleep_inhibitor() {
    let mut inhibitor = sleep_inhibitor().lock().unwrap();
    *inhibitor = None;
}

struct SleepInhibitor {
    assertion_id: PlatformAssertionId,
}

impl SleepInhibitor {
    fn new() -> Result<Self, String> {
        Ok(Self {
            assertion_id: create_platform_assertion()?,
        })
    }
}

impl Drop for SleepInhibitor {
    fn drop(&mut self) {
        release_platform_assertion(self.assertion_id);
    }
}

#[cfg(target_os = "macos")]
type PlatformAssertionId = u32;

#[cfg(target_os = "macos")]
fn create_platform_assertion() -> Result<PlatformAssertionId, String> {
    macos::create_assertion()
}

#[cfg(target_os = "macos")]
fn release_platform_assertion(assertion_id: PlatformAssertionId) {
    macos::release_assertion(assertion_id);
}

#[cfg(not(target_os = "macos"))]
type PlatformAssertionId = ();

#[cfg(not(target_os = "macos"))]
fn create_platform_assertion() -> Result<PlatformAssertionId, String> {
    Err("sleep inhibitor is only implemented on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
fn release_platform_assertion(_assertion_id: PlatformAssertionId) {}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_char, c_void, CString};
    use std::ptr;

    type CFAllocatorRef = *const c_void;
    type CFStringRef = *const c_void;
    type IOReturn = i32;
    type IOPMAssertionID = u32;
    type IOPMAssertionLevel = u32;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_IOPM_ASSERTION_LEVEL_ON: IOPMAssertionLevel = 255;
    const K_IO_RETURN_SUCCESS: IOReturn = 0;
    const PREVENT_USER_IDLE_SYSTEM_SLEEP: &str = "PreventUserIdleSystemSleep";
    const ASSERTION_NAME: &str = "nosleep";

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: CFAllocatorRef,
            c_str: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: CFStringRef,
            assertion_level: IOPMAssertionLevel,
            assertion_name: CFStringRef,
            assertion_id: *mut IOPMAssertionID,
        ) -> IOReturn;
        fn IOPMAssertionRelease(assertion_id: IOPMAssertionID) -> IOReturn;
    }

    pub(super) fn create_assertion() -> Result<IOPMAssertionID, String> {
        let assertion_type = cf_string(PREVENT_USER_IDLE_SYSTEM_SLEEP)?;
        let assertion_name = cf_string(ASSERTION_NAME)?;
        let mut assertion_id = 0;

        let result = unsafe {
            IOPMAssertionCreateWithName(
                assertion_type.as_ptr(),
                K_IOPM_ASSERTION_LEVEL_ON,
                assertion_name.as_ptr(),
                &mut assertion_id,
            )
        };

        if result == K_IO_RETURN_SUCCESS {
            Ok(assertion_id)
        } else {
            Err(format!(
                "IOPMAssertionCreateWithName failed with code {result}"
            ))
        }
    }

    pub(super) fn release_assertion(assertion_id: IOPMAssertionID) {
        let result = unsafe { IOPMAssertionRelease(assertion_id) };
        if result != K_IO_RETURN_SUCCESS {
            eprintln!("failed to release sleep inhibitor assertion: {result}");
        }
    }

    struct CfString {
        ptr: CFStringRef,
    }

    impl CfString {
        fn as_ptr(&self) -> CFStringRef {
            self.ptr
        }
    }

    impl Drop for CfString {
        fn drop(&mut self) {
            unsafe {
                CFRelease(self.ptr);
            }
        }
    }

    fn cf_string(value: &str) -> Result<CfString, String> {
        let value = CString::new(value).map_err(|err| err.to_string())?;
        let ptr = unsafe {
            CFStringCreateWithCString(ptr::null(), value.as_ptr(), K_CF_STRING_ENCODING_UTF8)
        };

        if ptr.is_null() {
            Err("CFStringCreateWithCString returned null".to_string())
        } else {
            Ok(CfString { ptr })
        }
    }
}
