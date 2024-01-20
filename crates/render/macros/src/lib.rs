#![cfg_attr(feature = "unstable", feature(proc_macro_span))]

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};
use utils::resolve_render_crate;

#[macro_use]
mod utils;
mod binding_layout;

#[cfg(feature = "unstable")]
mod compile_shader;

#[proc_macro_derive(AsBindingLayout, attributes(uniform, texture, sampler))]
pub fn derive_as_binding_layout(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    binding_layout::derive_as_binding_layout(input)
        .unwrap_or_else(|err| err.write_errors())
        .into()
}

encase_derive_impl::implement! {{
    let crate_render = resolve_render_crate().unwrap();
    encase_derive_impl::syn::parse_quote!(#crate_render::shader)
}}

/// requires #![feature(proc_macro_span)]
#[cfg(feature = "unstable")]
#[proc_macro]
pub fn compile_shader_int(input: TokenStream) -> TokenStream {
    compile_shader::CompileShaderArgs::parse(input.into())
        .and_then(|args| args.compile())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
