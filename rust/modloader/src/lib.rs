use std::ffi::{c_char, c_void, CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(not(windows))]
use anyhow::anyhow;
use anyhow::{bail, Context, Result};
use tracing::{debug, error, info, trace, warn};
use vienna_vma::{VmaGetModDefinition, VmaHostApi, VmaLogLevel, VmaResultCode, VMA_ABI_VERSION};

#[derive(Debug, Clone)]
pub struct LoadedModInfo {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
}

pub struct ModLoader {
    loaded_mods: Vec<LoadedMod>,
}

impl ModLoader {
    pub fn load_from_directory(directory: &Path) -> Result<Self> {
        if !directory.exists() {
            fs::create_dir_all(directory).with_context(|| {
                format!("failed to create mod directory {}", directory.display())
            })?;
            info!(mods_dir = %directory.display(), "created mod directory");
        }

        let mut entries = fs::read_dir(directory)
            .with_context(|| format!("failed to read mod directory {}", directory.display()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("failed to enumerate mod directory {}", directory.display()))?;

        entries.sort_by_key(|entry| entry.path());

        let mut loaded_mods = Vec::new();

        for entry in entries {
            let path = entry.path();
            if !is_mod_file(&path) {
                continue;
            }

            match unsafe { LoadedMod::load(&path) } {
                Ok(loaded_mod) => {
                    info!(
                        mod_name = %loaded_mod.info.name,
                        mod_version = %loaded_mod.info.version,
                        path = %loaded_mod.info.path.display(),
                        "loaded VMA mod"
                    );
                    loaded_mods.push(loaded_mod);
                }
                Err(error) => {
                    warn!(path = %path.display(), error = %error, "failed to load VMA mod");
                }
            }
        }

        info!(
            mods_dir = %directory.display(),
            loaded_mods = loaded_mods.len(),
            "finished VMA mod scan"
        );

        Ok(Self { loaded_mods })
    }

    pub fn infos(&self) -> Vec<LoadedModInfo> {
        self.loaded_mods.iter().map(|loaded_mod| loaded_mod.info.clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.loaded_mods.len()
    }

    pub fn dispatch_server_start(&self) {
        for loaded_mod in &self.loaded_mods {
            loaded_mod.call_server_start();
        }
    }

    pub fn dispatch_server_stop(&self) {
        for loaded_mod in &self.loaded_mods {
            loaded_mod.call_server_stop();
        }
    }

    pub fn dispatch_player_join(&self, player_name: &str) {
        for loaded_mod in &self.loaded_mods {
            loaded_mod.call_player_join(player_name);
        }
    }

    pub fn dispatch_player_leave(&self, player_name: &str) {
        for loaded_mod in &self.loaded_mods {
            loaded_mod.call_player_leave(player_name);
        }
    }

    pub fn dispatch_command(&self, player_name: &str, command: &str) {
        for loaded_mod in &self.loaded_mods {
            loaded_mod.call_command(player_name, command);
        }
    }
}

struct LoadedMod {
    info: LoadedModInfo,
    host_api: VmaHostApi,
    on_unload: unsafe extern "C" fn(host: *const VmaHostApi),
    on_server_start: unsafe extern "C" fn(host: *const VmaHostApi),
    on_server_stop: unsafe extern "C" fn(host: *const VmaHostApi),
    on_player_join: unsafe extern "C" fn(host: *const VmaHostApi, player_name: *const c_char),
    on_player_leave: unsafe extern "C" fn(host: *const VmaHostApi, player_name: *const c_char),
    on_command: unsafe extern "C" fn(
        host: *const VmaHostApi,
        player_name: *const c_char,
        command: *const c_char,
    ),
    _library: DynamicLibrary,
}

impl LoadedMod {
    unsafe fn load(path: &Path) -> Result<Self> {
        let library = DynamicLibrary::open(path)?;
        let symbol_name = CString::new("vma_get_mod_definition").expect("literal is valid");
        let get_definition =
            library.get::<VmaGetModDefinition>(symbol_name.as_c_str())?;
        let definition_ptr = get_definition();

        if definition_ptr.is_null() {
            bail!("VMA mod returned a null definition");
        }

        let definition = &*definition_ptr;
        if definition.abi_version != VMA_ABI_VERSION {
            bail!(
                "VMA ABI mismatch: host={} mod={}",
                VMA_ABI_VERSION,
                definition.abi_version
            );
        }

        let name = read_c_string(definition.name).context("mod name is invalid")?;
        let version = read_c_string(definition.version).context("mod version is invalid")?;

        let host_api = VmaHostApi {
            abi_version: VMA_ABI_VERSION,
            log: host_log_callback,
        };

        let result = (definition.on_load)(&host_api as *const _);
        if result != VmaResultCode::Ok {
            bail!("mod on_load returned an error");
        }

        Ok(Self {
            info: LoadedModInfo {
                name,
                version,
                path: path.to_path_buf(),
            },
            host_api,
            on_unload: definition.on_unload,
            on_server_start: definition.on_server_start,
            on_server_stop: definition.on_server_stop,
            on_player_join: definition.on_player_join,
            on_player_leave: definition.on_player_leave,
            on_command: definition.on_command,
            _library: library,
        })
    }

    fn call_server_start(&self) {
        trace!(mod_name = %self.info.name, "dispatching on_server_start");
        unsafe {
            (self.on_server_start)(&self.host_api as *const _);
        }
    }

    fn call_server_stop(&self) {
        trace!(mod_name = %self.info.name, "dispatching on_server_stop");
        unsafe {
            (self.on_server_stop)(&self.host_api as *const _);
        }
    }

    fn call_player_join(&self, player_name: &str) {
        let player_name_log = player_name.to_owned();
        let player_name = sanitize_for_c(player_name);
        trace!(mod_name = %self.info.name, player_name = %player_name_log, "dispatching on_player_join");
        unsafe {
            (self.on_player_join)(&self.host_api as *const _, player_name.as_ptr());
        }
    }

    fn call_player_leave(&self, player_name: &str) {
        let player_name_log = player_name.to_owned();
        let player_name = sanitize_for_c(player_name);
        trace!(mod_name = %self.info.name, player_name = %player_name_log, "dispatching on_player_leave");
        unsafe {
            (self.on_player_leave)(&self.host_api as *const _, player_name.as_ptr());
        }
    }

    fn call_command(&self, player_name: &str, command: &str) {
        let player_name_log = player_name.to_owned();
        let command_log = command.to_owned();
        let player_name = sanitize_for_c(player_name);
        let command = sanitize_for_c(command);
        trace!(mod_name = %self.info.name, player_name = %player_name_log, command = %command_log, "dispatching on_command");
        unsafe {
            (self.on_command)(&self.host_api as *const _, player_name.as_ptr(), command.as_ptr());
        }
    }
}

impl Drop for LoadedMod {
    fn drop(&mut self) {
        trace!(mod_name = %self.info.name, "calling VMA mod on_unload");
        unsafe {
            (self.on_unload)(&self.host_api as *const _);
        }
    }
}

fn is_mod_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("mcemod")
                    || extension.eq_ignore_ascii_case("dll")
            })
}

fn read_c_string(raw: *const c_char) -> Result<String> {
    if raw.is_null() {
        bail!("null pointer");
    }

    Ok(unsafe { CStr::from_ptr(raw) }.to_string_lossy().into_owned())
}

fn sanitize_for_c(value: &str) -> CString {
    let sanitized = value.replace('\0', " ");
    CString::new(sanitized).unwrap_or_else(|_| CString::new("").expect("literal is valid"))
}

unsafe extern "C" fn host_log_callback(level: VmaLogLevel, message: *const c_char) {
    if message.is_null() {
        warn!("mod emitted a null log message");
        return;
    }

    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    match level {
        VmaLogLevel::Trace => trace!(target: "vienna_vma_mod", "{message}"),
        VmaLogLevel::Debug => debug!(target: "vienna_vma_mod", "{message}"),
        VmaLogLevel::Info => info!(target: "vienna_vma_mod", "{message}"),
        VmaLogLevel::Warn => warn!(target: "vienna_vma_mod", "{message}"),
        VmaLogLevel::Error => error!(target: "vienna_vma_mod", "{message}"),
    }
}

#[cfg(windows)]
struct DynamicLibrary {
    handle: *mut c_void,
}

#[cfg(windows)]
impl DynamicLibrary {
    unsafe fn open(path: &Path) -> Result<Self> {
        use std::os::windows::ffi::OsStrExt;

        #[link(name = "kernel32")]
        extern "system" {
            fn LoadLibraryW(file_name: *const u16) -> *mut c_void;
        }

        let mut wide_path: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide_path.push(0);

        let handle = LoadLibraryW(wide_path.as_ptr());
        if handle.is_null() {
            bail!("failed to load {}", path.display());
        }

        Ok(Self { handle })
    }

    unsafe fn get<T: Copy>(&self, symbol: &CStr) -> Result<T> {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        }

        let raw_symbol = GetProcAddress(self.handle, symbol.as_ptr());
        if raw_symbol.is_null() {
            bail!("failed to find symbol {}", symbol.to_string_lossy());
        }

        Ok(std::mem::transmute_copy::<*mut c_void, T>(&raw_symbol))
    }
}

#[cfg(windows)]
impl Drop for DynamicLibrary {
    fn drop(&mut self) {
        #[link(name = "kernel32")]
        extern "system" {
            fn FreeLibrary(module: *mut c_void) -> i32;
        }

        unsafe {
            let _ = FreeLibrary(self.handle);
        }
    }
}

#[cfg(windows)]
unsafe impl Send for DynamicLibrary {}

#[cfg(windows)]
unsafe impl Sync for DynamicLibrary {}

#[cfg(not(windows))]
struct DynamicLibrary;

#[cfg(not(windows))]
impl DynamicLibrary {
    unsafe fn open(path: &Path) -> Result<Self> {
        Err(anyhow!(
            "VMA mod loading is currently implemented only for Windows: {}",
            path.display()
        ))
    }

    unsafe fn get<T: Copy>(&self, _symbol: &CStr) -> Result<T> {
        Err(anyhow!("dynamic symbol lookup is not available on this platform"))
    }
}
