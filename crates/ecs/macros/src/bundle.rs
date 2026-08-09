use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn derive_bundle(input: DeriveInput) -> TokenStream {
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics ::pulz_ecs::component::Bundle for #ident #ty_generics #where_clause {
            // TODO: implement
        }
    }
}
