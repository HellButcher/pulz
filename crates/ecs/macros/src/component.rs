use darling::{util::SpannedValue, Error, FromDeriveInput, Result};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, DeriveInput, Path};

use crate::utils::resolve_crate;

pub fn derive_component(input: DeriveInput) -> Result<TokenStream> {
    let args = ComponentStructArgs::from_derive_input(&input)?;
    args.validate()?;

    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let crate_ecs = resolve_crate("pulz-ecs")?;

    let storage: Path = if let Some(storage) = &*args.storage {
        if let Some(single_ident) = storage.get_ident() {
            parse_quote!(#crate_ecs::storage::#single_ident)
        } else {
            storage.clone()
        }
    } else if *args.sparse {
        parse_quote!(#crate_ecs::storage::HashMapStorage)
    } else {
        parse_quote!(#crate_ecs::storage::DenseStorage)
    };
    Ok(quote! {
        impl #impl_generics #crate_ecs::component::Component for #ident #ty_generics #where_clause {
            type Storage = #storage<Self>;
        }
    })
}

#[derive(Default, FromDeriveInput)]
#[darling(default, attributes(component), forward_attrs(allow, doc, cfg))]
pub struct ComponentStructArgs {
    sparse: SpannedValue<bool>,
    storage: SpannedValue<Option<Path>>,
}

impl ComponentStructArgs {
    fn validate(&self) -> Result<()> {
        if *self.sparse && self.storage.is_some() {
            const MSG: &str = "either provide `sparse` or `storage`, but not both!";
            return Err(Error::multiple(vec![
                Error::custom(MSG).with_span(&self.sparse),
                Error::custom(MSG).with_span(&self.storage),
            ]));
        }
        Ok(())
    }
}
