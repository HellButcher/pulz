use std::borrow::Cow;

mod preprocessor;

use serde::{Deserialize, Serialize};

mod encase {
    pub use ::encase::{private, ShaderSize, ShaderType};
}
pub use ::pulz_render_macros::ShaderType;

#[doc(hidden)]
pub use self::encase::*;

crate::backend::define_gpu_resource!(ShaderModule, ShaderModuleDescriptor<'l>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShaderModuleDescriptor<'a> {
    pub label: Option<&'a str>,
    pub source: ShaderSource<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ShaderSource<'a> {
    Wgsl(Cow<'a, str>),
    Glsl(Cow<'a, str>),
    SpirV(Cow<'a, [u32]>),
}

// impl<'a> ShaderSource<'a> {
//     pub fn from_spirv_bytes(data: &'a [u8]) -> Self {
//         Self::SpirV(spirv_raw_from_bytes(data))
//     }
// }

// fn spirv_raw_from_bytes(data: &[u8]) -> Cow<'_, [u32]> {
//     const MAGIC_NUMBER: u32 = 0x0723_0203;

//     assert!(data.len() % 4 == 0, "data size is not a multiple of 4");

//     let (pre, words, post) = unsafe { data.align_to::<u32>() };
//     let words = if pre.is_empty() {
//         // is already aligned
//         debug_assert!(post.is_empty());
//         Cow::Borrowed(words)
//     } else {
//         // copy into aligned Vec
//         let mut words = vec![0u32; data.len() / 4];
//         unsafe {
//             std::ptr::copy_nonoverlapping(data.as_ptr(), words.as_mut_ptr() as *mut u8, data.len());
//         }
//         Cow::from(words)
//     };

//     assert_eq!(words[0], MAGIC_NUMBER, "wrong magic word {:x}. Not a SPIRV file.", words[0]);

//     words
// }

// #[macro_export]
// macro_rules! compile_shader {
//     ($filename:literal) => {
//         $crate::shader::ShaderModuleDescriptor {
//             label: Some($filename),
//             #[cfg(not(target_arch = "wasm32"))]
//             source: $crate::shader::ShaderSource::SpirV(::std::borrow::Cow::Borrowed($crate::shader::__compile_shader_int!(
//                 target_format = "SpirV",
//                 source = $filename,
//             ))),
//             #[cfg(target_arch = "wasm32")]
//             source: $crate::shader::ShaderSource::Wgsl(::std::borrow::Cow::Borrowed($crate::shader::__compile_shader_int!(
//                 target_format = "Wgsl",
//                 source = $filename,
//             ))),
//         }
//     };
// }

/// Macro to load a WGSL shader module statically.
#[macro_export]
macro_rules! include_wgsl {
    ($file:literal) => {
        &$crate::shader::ShaderModuleDescriptor {
            label: Some($file),
            source: $crate::shader::ShaderSource::Wgsl(::std::borrow::Cow::Borrowed(include_str!(
                $file
            ))),
        }
    };
}

// #[macro_export]
// macro_rules! include_glsl {
//     ($file:literal) => {
//         &$crate::shader::ShaderModuleDescriptor {
//             label: Some($file),
//             source: $crate::shader::ShaderSource::Glsl(::std::borrow::Cow::Borrowed(include_str!($file))),
//         }
//     };
// }

// #[macro_export]
// macro_rules! include_spirv {
//     ($file:literal) => {
//         &$crate::shader::ShaderModuleDescriptor {
//             label: Some($file),
//             source: $crate::shader::ShaderSource::from_spirv_bytes(include_bytes!($file)),
//         }
//     };
// }
