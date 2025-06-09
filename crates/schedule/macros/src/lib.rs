use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{DeriveInput, ImplItemFn, ItemImpl, Path, parse_macro_input};

mod attrib_system;
mod attrib_system_module;
mod derive_label;
mod derive_system_data;
mod utils;

#[proc_macro_attribute]
pub fn system_module(attribute: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemImpl);
    attrib_system_module::attrib_system_module(attribute.into(), input).into()
}

#[proc_macro_attribute]
pub fn system(attribute: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ImplItemFn);
    attrib_system::attrib_system(attribute.into(), input).into()
}

#[proc_macro]
pub fn into_system(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Path);
    attrib_system::into_system_path(input)
        .into_token_stream()
        .into()
}

#[proc_macro_derive(SystemData, attributes(system_data, __crate_path))]
pub fn system_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_system_data::derive_system_data(input).into()
}
