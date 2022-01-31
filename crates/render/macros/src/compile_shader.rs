use darling::FromMeta;
use proc_macro2::TokenStream;
use syn::{AttributeArgs, LitStr, Result};

pub fn compile_shader_int(args: AttributeArgs) -> Result<TokenStream> {
    let args = CompileShaderArgs::from_list(&args)?;
    compile_shader_with_args(args)
}

pub fn compile_shader_with_args(args: CompileShaderArgs) -> Result<TokenStream> {
    panic!("TODO: implement: {:#?}", args);
}

#[derive(FromMeta, PartialEq, Eq, Debug)]
pub enum TargetFormat {
    Wgsl,
    SpirV,
}

#[derive(FromMeta, PartialEq, Eq, Debug)]
pub struct CompileShaderArgs {
    pub target_format: TargetFormat,
    pub source: LitStr,
}
