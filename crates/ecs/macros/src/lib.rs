use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod bundle;
mod component;
mod utils;

#[proc_macro_derive(Component, attributes(component))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    component::derive_component(input).into()
}

#[proc_macro_derive(Bundle, attributes(bundle))]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    bundle::derive_bundle(input).into()
}
