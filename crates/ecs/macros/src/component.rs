use darling::{
    util::{Flag, SpannedValue},
    Error, FromDeriveInput, Result,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, DeriveInput, Path};

use crate::utils::resolve_crate;

pub fn derive_component(input: DeriveInput) -> Result<TokenStream> {
    let args = ComponentStructArgs::from_derive_input(&input)?;

    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let crate_ecs = resolve_crate("pulz-ecs")?;

    let storage: Path = if let Some(storage) = &*args.storage {
        if let Some(single_ident) = storage.get_ident() {
            parse_quote!(#crate_ecs::storage::#single_ident)
        } else {
            storage.clone()
        }
    } else if args.sparse.is_present() {
        parse_quote!(#crate_ecs::storage::SparseStorage)
    } else {
        parse_quote!(#crate_ecs::storage::ArchetypeStorage)
    };
    let mut storage: syn::Type = parse_quote!(#storage<Self>);
    if args.tracked.is_present() {
        storage = parse_quote!(#crate_ecs::storage::Tracked<#storage>);
    }
    Ok(quote! {
        impl #impl_generics #crate_ecs::component::Component for #ident #ty_generics #where_clause {
            type Storage = #storage;
        }
    })
}

#[derive(Default, FromDeriveInput)]
#[darling(
    default,
    attributes(component),
    forward_attrs(allow, doc, cfg),
    and_then = "Self::validate"
)]
pub struct ComponentStructArgs {
    sparse: Flag,
    tracked: Flag,
    storage: SpannedValue<Option<Path>>,
}

impl ComponentStructArgs {
    fn validate(self) -> Result<Self> {
        if self.sparse.is_present() && self.storage.is_some() {
            const MSG: &str = "either provide `sparse` or `storage`, but not both!";
            return Err(Error::custom(MSG));
        }
        Ok(self)
    }
}
