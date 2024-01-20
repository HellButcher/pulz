use darling::FromMeta;
use proc_macro2::TokenStream;
use syn::{LitStr, Result};

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

impl CompileShaderArgs {
    pub fn parse(input: TokenStream) -> Result<Self> {
        let meta_list = darling::ast::NestedMeta::parse_meta_list(input)?;
        let args = Self::from_list(&meta_list)?;
        Ok(args)
    }
    pub fn compile(&self) -> Result<TokenStream> {
        panic!("TODO: implement: {:#?}", self);
    }
}
