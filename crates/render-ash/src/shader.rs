use std::borrow::Cow;

use pulz_render::shader::ShaderSource;

use crate::Result;

pub fn compie_into_spv<'a>(source: &'a ShaderSource<'a>) -> Result<Cow<'a, [u32]>> {
    match source {
        ShaderSource::SpirV(data) => Ok(Cow::Borrowed(data)),
        ShaderSource::Glsl(_data) => todo!("implement compile GLSL"),
        ShaderSource::Wgsl(_data) => todo!("implement compile WGSL"),

        _ => panic!("unsupported shader source"),
    }
}
