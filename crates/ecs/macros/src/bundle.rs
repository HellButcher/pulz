use darling::Result;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::utils::resolve_crate;

pub fn derive_bundle(input: DeriveInput) -> Result<TokenStream> {
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let crate_ecs = resolve_crate("pulz-ecs")?;

    Ok(quote! {
        impl #impl_generics #crate_ecs::component::Bundle for #ident #ty_generics #where_clause {
            // TODO: implement
        }
    })
}
