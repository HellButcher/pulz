use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

#[macro_use]
mod utils;
mod bundle;
mod component;

#[proc_macro_derive(Component, attributes(component))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    component::derive_component(input)
        .unwrap_or_else(|err| err.write_errors())
        .into()
}

#[proc_macro_derive(Bundle, attributes(bundle))]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    bundle::derive_bundle(input)
        .unwrap_or_else(|err| err.write_errors())
        .into()
}
