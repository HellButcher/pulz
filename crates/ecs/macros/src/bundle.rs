use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Result};

pub fn derive_bundle(input: DeriveInput) -> Result<TokenStream> {
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics Bundle for #ident #ty_generics #where_clause  {
            // TODO: implement
        }
    })
}
