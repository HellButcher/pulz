use proc_macro::TokenStream;

extern crate proc_macro;

mod generate_variadic_array;
use generate_variadic_array::VariadicTupleGenerator;
use quote::ToTokens;
use syn::parse_macro_input;

#[proc_macro]
pub fn generate_variadic_array(input: TokenStream) -> TokenStream {
    let generator = parse_macro_input!(input as VariadicTupleGenerator);
    TokenStream::from(generator.into_token_stream())
}
