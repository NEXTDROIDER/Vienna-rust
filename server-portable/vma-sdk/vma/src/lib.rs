use std::ffi::{c_char, CString};
use std::panic;

pub const VMA_ABI_VERSION: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmaLogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmaResultCode {
    Ok = 0,
    Failed = 1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VmaHostApi {
    pub abi_version: u32,
    pub log: unsafe extern "C" fn(level: VmaLogLevel, message: *const c_char),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VmaModDefinition {
    pub abi_version: u32,
    pub name: *const c_char,
    pub version: *const c_char,
    pub on_load: unsafe extern "C" fn(host: *const VmaHostApi) -> VmaResultCode,
    pub on_unload: unsafe extern "C" fn(host: *const VmaHostApi),
    pub on_server_start: unsafe extern "C" fn(host: *const VmaHostApi),
    pub on_server_stop: unsafe extern "C" fn(host: *const VmaHostApi),
    pub on_player_join: unsafe extern "C" fn(host: *const VmaHostApi, player_name: *const c_char),
    pub on_player_leave: unsafe extern "C" fn(host: *const VmaHostApi, player_name: *const c_char),
    pub on_command: unsafe extern "C" fn(
        host: *const VmaHostApi,
        player_name: *const c_char,
        command: *const c_char,
    ),
}

unsafe impl Sync for VmaModDefinition {}

pub type VmaGetModDefinition = unsafe extern "C" fn() -> *const VmaModDefinition;
pub type ModResult = Result<(), String>;

#[derive(Clone, Copy)]
pub struct HostApi {
    raw: *const VmaHostApi,
}

impl HostApi {
    pub unsafe fn from_raw(raw: *const VmaHostApi) -> Self {
        Self { raw }
    }

    pub fn abi_version(&self) -> u32 {
        unsafe { (*self.raw).abi_version }
    }

    pub fn log(&self, level: VmaLogLevel, message: &str) {
        let sanitized = message.replace('\0', " ");
        let c_message = CString::new(sanitized)
            .unwrap_or_else(|_| CString::new("invalid VMA log message").expect("literal is valid"));

        unsafe {
            ((*self.raw).log)(level, c_message.as_ptr());
        }
    }

    pub fn log_trace(&self, message: &str) {
        self.log(VmaLogLevel::Trace, message);
    }

    pub fn log_debug(&self, message: &str) {
        self.log(VmaLogLevel::Debug, message);
    }

    pub fn log_info(&self, message: &str) {
        self.log(VmaLogLevel::Info, message);
    }

    pub fn log_warn(&self, message: &str) {
        self.log(VmaLogLevel::Warn, message);
    }

    pub fn log_error(&self, message: &str) {
        self.log(VmaLogLevel::Error, message);
    }
}

pub fn run_load_callback(
    host: *const VmaHostApi,
    callback: fn(&HostApi) -> ModResult,
) -> VmaResultCode {
    match panic::catch_unwind(|| {
        let host = unsafe { HostApi::from_raw(host) };
        callback(&host)
    }) {
        Ok(Ok(())) => VmaResultCode::Ok,
        Ok(Err(message)) => {
            let host = unsafe { HostApi::from_raw(host) };
            host.log_error(&message);
            VmaResultCode::Failed
        }
        Err(_) => {
            let host = unsafe { HostApi::from_raw(host) };
            host.log_error("mod panicked during on_load");
            VmaResultCode::Failed
        }
    }
}

pub fn run_unload_callback(host: *const VmaHostApi, callback: fn(&HostApi)) {
    let _ = panic::catch_unwind(|| {
        let host = unsafe { HostApi::from_raw(host) };
        callback(&host);
    });
}

pub fn default_on_unload(_host: &HostApi) {}
pub fn default_on_server_start(_host: &HostApi) {}
pub fn default_on_server_stop(_host: &HostApi) {}
pub fn default_on_player_join(_host: &HostApi, _player_name: &str) {}
pub fn default_on_player_leave(_host: &HostApi, _player_name: &str) {}
pub fn default_on_command(_host: &HostApi, _player_name: &str, _command: &str) {}

pub fn run_void_callback(host: *const VmaHostApi, callback: fn(&HostApi), callback_name: &str) {
    let _ = panic::catch_unwind(|| {
        let host = unsafe { HostApi::from_raw(host) };
        callback(&host);
    })
    .map_err(|_| {
        let host = unsafe { HostApi::from_raw(host) };
        host.log_error(&format!("mod panicked during {callback_name}"));
    });
}

pub fn run_player_callback(
    host: *const VmaHostApi,
    player_name: *const c_char,
    callback: fn(&HostApi, &str),
    callback_name: &str,
) {
    let player_name = match read_optional_str(player_name) {
        Ok(player_name) => player_name,
        Err(error) => {
            let host = unsafe { HostApi::from_raw(host) };
            host.log_error(&format!("{callback_name} received invalid player name: {error}"));
            return;
        }
    };

    let _ = panic::catch_unwind(|| {
        let host = unsafe { HostApi::from_raw(host) };
        callback(&host, &player_name);
    })
    .map_err(|_| {
        let host = unsafe { HostApi::from_raw(host) };
        host.log_error(&format!("mod panicked during {callback_name}"));
    });
}

pub fn run_command_callback(
    host: *const VmaHostApi,
    player_name: *const c_char,
    command: *const c_char,
    callback: fn(&HostApi, &str, &str),
    callback_name: &str,
) {
    let player_name = match read_optional_str(player_name) {
        Ok(player_name) => player_name,
        Err(error) => {
            let host = unsafe { HostApi::from_raw(host) };
            host.log_error(&format!("{callback_name} received invalid player name: {error}"));
            return;
        }
    };

    let command = match read_optional_str(command) {
        Ok(command) => command,
        Err(error) => {
            let host = unsafe { HostApi::from_raw(host) };
            host.log_error(&format!("{callback_name} received invalid command: {error}"));
            return;
        }
    };

    let _ = panic::catch_unwind(|| {
        let host = unsafe { HostApi::from_raw(host) };
        callback(&host, &player_name, &command);
    })
    .map_err(|_| {
        let host = unsafe { HostApi::from_raw(host) };
        host.log_error(&format!("mod panicked during {callback_name}"));
    });
}

fn read_optional_str(raw: *const c_char) -> Result<String, String> {
    if raw.is_null() {
        return Ok(String::new());
    }

    unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_str()
        .map(str::to_owned)
        .map_err(|error| error.to_string())
}

#[macro_export]
macro_rules! export_mcemod {
    (
        name: $name:literal,
        version: $version:literal,
        on_load: $on_load:path
        $(, on_unload: $on_unload:path)?
        $(, on_server_start: $on_server_start:path)?
        $(, on_server_stop: $on_server_stop:path)?
        $(, on_player_join: $on_player_join:path)?
        $(, on_player_leave: $on_player_leave:path)?
        $(, on_command: $on_command:path)?
        $(,)?
    ) => {
        static __VMA_MOD_NAME: &[u8] = concat!($name, "\0").as_bytes();
        static __VMA_MOD_VERSION: &[u8] = concat!($version, "\0").as_bytes();

        unsafe extern "C" fn __vma_on_load(
            host: *const $crate::VmaHostApi,
        ) -> $crate::VmaResultCode {
            $crate::run_load_callback(host, $on_load)
        }

        unsafe extern "C" fn __vma_on_unload(host: *const $crate::VmaHostApi) {
            $crate::run_unload_callback(
                host,
                $crate::export_mcemod!(@resolve_on_unload $($on_unload)?),
            )
        }

        unsafe extern "C" fn __vma_on_server_start(host: *const $crate::VmaHostApi) {
            $crate::run_void_callback(
                host,
                $crate::export_mcemod!(@resolve_on_server_start $($on_server_start)?),
                "on_server_start",
            )
        }

        unsafe extern "C" fn __vma_on_server_stop(host: *const $crate::VmaHostApi) {
            $crate::run_void_callback(
                host,
                $crate::export_mcemod!(@resolve_on_server_stop $($on_server_stop)?),
                "on_server_stop",
            )
        }

        unsafe extern "C" fn __vma_on_player_join(
            host: *const $crate::VmaHostApi,
            player_name: *const std::ffi::c_char,
        ) {
            $crate::run_player_callback(
                host,
                player_name,
                $crate::export_mcemod!(@resolve_on_player_join $($on_player_join)?),
                "on_player_join",
            )
        }

        unsafe extern "C" fn __vma_on_player_leave(
            host: *const $crate::VmaHostApi,
            player_name: *const std::ffi::c_char,
        ) {
            $crate::run_player_callback(
                host,
                player_name,
                $crate::export_mcemod!(@resolve_on_player_leave $($on_player_leave)?),
                "on_player_leave",
            )
        }

        unsafe extern "C" fn __vma_on_command(
            host: *const $crate::VmaHostApi,
            player_name: *const std::ffi::c_char,
            command: *const std::ffi::c_char,
        ) {
            $crate::run_command_callback(
                host,
                player_name,
                command,
                $crate::export_mcemod!(@resolve_on_command $($on_command)?),
                "on_command",
            )
        }

        static __VMA_MOD_DEFINITION: $crate::VmaModDefinition = $crate::VmaModDefinition {
            abi_version: $crate::VMA_ABI_VERSION,
            name: __VMA_MOD_NAME.as_ptr() as *const std::ffi::c_char,
            version: __VMA_MOD_VERSION.as_ptr() as *const std::ffi::c_char,
            on_load: __vma_on_load,
            on_unload: __vma_on_unload,
            on_server_start: __vma_on_server_start,
            on_server_stop: __vma_on_server_stop,
            on_player_join: __vma_on_player_join,
            on_player_leave: __vma_on_player_leave,
            on_command: __vma_on_command,
        };

        #[no_mangle]
        pub extern "C" fn vma_get_mod_definition() -> *const $crate::VmaModDefinition {
            &__VMA_MOD_DEFINITION
        }
    };
    (@resolve_on_unload $on_unload:path) => {
        $on_unload
    };
    (@resolve_on_unload) => {
        $crate::default_on_unload
    };
    (@resolve_on_server_start $on_server_start:path) => {
        $on_server_start
    };
    (@resolve_on_server_start) => {
        $crate::default_on_server_start
    };
    (@resolve_on_server_stop $on_server_stop:path) => {
        $on_server_stop
    };
    (@resolve_on_server_stop) => {
        $crate::default_on_server_stop
    };
    (@resolve_on_player_join $on_player_join:path) => {
        $on_player_join
    };
    (@resolve_on_player_join) => {
        $crate::default_on_player_join
    };
    (@resolve_on_player_leave $on_player_leave:path) => {
        $on_player_leave
    };
    (@resolve_on_player_leave) => {
        $crate::default_on_player_leave
    };
    (@resolve_on_command $on_command:path) => {
        $on_command
    };
    (@resolve_on_command) => {
        $crate::default_on_command
    };
}
