use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod bundle;
mod component;
mod utils;

#[proc_macro_derive(Component, attributes(component))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    component::derive_component(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(Bundle)]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    bundle::derive_bundle(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
