use std::{ffi::CStr, ops::Deref, os::raw::c_char, sync::Arc};

use ash::{
    extensions::{ext, khr},
    vk,
};
use tracing::{debug, warn};

use crate::{debug_utils::DebugUtils, AshRendererFlags, ErrorNoExtension, Result};

pub const ENGINE_NAME: &[u8] = concat!(env!("CARGO_PKG_NAME"), "\0").as_bytes();
pub const ENGINE_VERSION: u32 = parse_version(env!("CARGO_PKG_VERSION"));
pub const VK_API_VERSION: u32 = vk::API_VERSION_1_1;

pub struct AshInstance {
    instance_raw: ash::Instance,
    entry: ash::Entry,
    instance_extensions: Vec<&'static CStr>,
    ext_debug_utils: Option<DebugUtils>,
    ext_surface: Option<khr::Surface>,
    flags: AshRendererFlags,
}

impl Deref for AshInstance {
    type Target = ash::Instance;
    #[inline]
    fn deref(&self) -> &ash::Instance {
        &self.instance_raw
    }
}

impl AshInstance {
    pub(crate) fn new(flags: AshRendererFlags) -> Result<Arc<Self>> {
        let entry = unsafe { ash::Entry::load()? };
        let instance_extensions = get_instance_extensions(&entry, flags)?;
        let instance_raw = create_instance(&entry, instance_extensions.iter().copied())?;

        let mut instance = Self {
            entry,
            instance_raw,
            instance_extensions,
            ext_debug_utils: None,
            ext_surface: None,
            flags,
        };

        if instance.has_instance_extension(DebugUtils::name()) {
            instance.ext_debug_utils = Some(DebugUtils::new(
                instance.entry(),
                &instance,
                // TODO: filter activated severities
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                    | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
            ));
        }

        if instance.has_instance_extension(khr::Surface::name()) {
            instance.ext_surface = Some(khr::Surface::new(instance.entry(), &instance));
        }

        Ok(Arc::new(instance))
    }

    #[inline]
    pub fn entry(&self) -> &ash::Entry {
        &self.entry
    }

    #[inline]
    pub fn has_instance_extension(&self, name: &CStr) -> bool {
        self.instance_extensions.contains(&name)
    }

    #[inline]
    pub(crate) fn ext_surface(&self) -> Result<&khr::Surface, ErrorNoExtension> {
        self.ext_surface
            .as_ref()
            .ok_or(ErrorNoExtension(khr::Surface::name()))
    }

    #[inline]
    pub(crate) fn ext_debug_utils(&self) -> Result<&DebugUtils, ErrorNoExtension> {
        self.ext_debug_utils
            .as_ref()
            .ok_or(ErrorNoExtension(ext::DebugUtils::name()))
    }

    #[inline]
    pub fn flags(&self) -> AshRendererFlags {
        self.flags
    }
}

impl Drop for AshInstance {
    fn drop(&mut self) {
        self.ext_debug_utils = None;
        unsafe { self.instance_raw.destroy_instance(None) }
    }
}

fn get_instance_extensions(
    entry: &ash::Entry,
    flags: AshRendererFlags,
) -> Result<Vec<&'static CStr>> {
    let available_extensions = entry.enumerate_instance_extension_properties(None)?;

    let mut extensions = Vec::with_capacity(5);
    extensions.push(khr::Surface::name());

    if cfg!(target_os = "windows") {
        extensions.push(khr::Win32Surface::name());
    } else if cfg!(target_os = "android") {
        extensions.push(khr::AndroidSurface::name());
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        extensions.push(ext::MetalSurface::name());
    } else if cfg!(unix) {
        extensions.push(khr::XlibSurface::name());
        extensions.push(khr::XcbSurface::name());
        extensions.push(khr::WaylandSurface::name());
    }

    if flags.contains(AshRendererFlags::DEBUG) {
        extensions.push(DebugUtils::name());
    }

    // Only keep available extensions.
    extensions.retain(|&ext| {
        if available_extensions
            .iter()
            .any(|avail_ext| unsafe { CStr::from_ptr(avail_ext.extension_name.as_ptr()) == ext })
        {
            debug!("Instance extension ✅ YES {:?}", ext);
            true
        } else {
            warn!("Instance extension ❌ NO  {:?}", ext);
            false
        }
    });

    Ok(extensions)
}

#[inline]
fn create_instance<'a>(
    entry: &ash::Entry,
    extensions: impl IntoIterator<Item = &'a CStr>,
) -> Result<ash::Instance> {
    let extensions_ptr: Vec<_> = extensions.into_iter().map(CStr::as_ptr).collect();
    _create_instance(entry, &extensions_ptr)
}

fn _create_instance(entry: &ash::Entry, extensions_ptr: &[*const c_char]) -> Result<ash::Instance> {
    let engine_name = unsafe { CStr::from_bytes_with_nul_unchecked(ENGINE_NAME) };

    let instance = unsafe {
        entry.create_instance(
            &vk::InstanceCreateInfo::builder()
                .application_info(
                    &vk::ApplicationInfo::builder()
                        .application_name(engine_name)
                        .application_version(ENGINE_VERSION)
                        .engine_name(engine_name)
                        .engine_version(ENGINE_VERSION)
                        .api_version(VK_API_VERSION),
                )
                .enabled_extension_names(extensions_ptr),
            None,
        )?
    };
    Ok(instance)
}

macro_rules! parse_int_iteration {
    ($value:ident += $input:ident[$pos:expr]) => {
        if $input.len() <= $pos {
            return ($value, $pos);
        }
        $value *= 10;
        let c = $input[$pos];
        if c < '0' as u8 || c > '9' as u8 {
            if c != '.' as u8 && c != '-' as u8 {
                panic!("invalid character in version");
            }
            return ($value, $pos + 1);
        }
        $value += c as u32 - '0' as u32;
    };
}

#[inline]
const fn const_parse_decimal_u32(input: &[u8], offset: usize) -> (u32, usize) {
    let mut value = 0;
    // manual unroll of loop for const compability
    parse_int_iteration!(value += input[offset]);
    parse_int_iteration!(value += input[offset + 1]);
    parse_int_iteration!(value += input[offset + 2]);
    parse_int_iteration!(value += input[offset + 3]);
    parse_int_iteration!(value += input[offset + 4]);
    parse_int_iteration!(value += input[offset + 5]);
    (value, offset + 6)
}

#[inline]
const fn parse_version(version: &str) -> u32 {
    let version = version.as_bytes();
    let (major, i) = const_parse_decimal_u32(version, 0);
    let (minor, j) = const_parse_decimal_u32(version, i);
    let (patch, _) = const_parse_decimal_u32(version, j);
    vk::make_api_version(0, major, minor, patch)
}
