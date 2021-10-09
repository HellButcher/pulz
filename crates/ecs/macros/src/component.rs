use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_quote, Attribute, DeriveInput, Error, Ident, Path, Result, Token,
};

use crate::utils::{resolve_crate, Attr, AttributeKeyword};

pub fn derive_component(input: DeriveInput) -> Result<TokenStream> {
    let args = ComponentStructArgs::parse_attributes(&input.attrs)?;
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let crate_ecs = resolve_crate("pulz-ecs")?;

    let s: Path = if let Some(storage) = args.storage {
        storage.arg
    } else if args.sparse.is_some() {
        parse_quote!(#crate_ecs::storage::HashMapStorage)
    } else {
        parse_quote!(#crate_ecs::storage::DenseStorage)
    };
    Ok(quote! {
        impl #impl_generics #crate_ecs::component::Component for #ident #ty_generics #where_clause  {
            type Storage = #s<Self>;
        }
    })
}

#[derive(Default)]
struct ComponentStructArgs {
    sparse: Option<attr::sparse>,
    storage: Option<Attr<attr::storage>>,
}

mod attr {
    attribute_kw!(sparse);
    attribute_kw!(storage: syn::Path);
}

impl ComponentStructArgs {
    fn parse_attributes(input: &[Attribute]) -> Result<Self> {
        let mut result = Self::default();
        for attrib in input {
            if attrib.path.is_ident("component") {
                attrib.parse_args_with(|input: ParseStream| result.parse_into(input))?;
            }
        }
        result.validate()?;
        Ok(result)
    }
    fn parse_into(&mut self, input: ParseStream) -> Result<()> {
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if let Some(sparse) = attr::sparse::from_ident(&ident) {
                self.sparse = Some(sparse);
            } else if let Some(storage) = attr::storage::parse_if(&ident, &input)? {
                self.storage = Some(storage);
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(())
    }
    fn validate(&self) -> Result<()> {
        if self.sparse.is_some() && self.storage.is_some() {
            const MSG: &str = "either provide `sparse` or `storage`, but not both!";
            let mut err = Error::new_spanned(&self.sparse, MSG);
            err.combine(Error::new_spanned(&self.storage, MSG));
            return Err(err);
        }
        Ok(())
    }
}
impl Parse for ComponentStructArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut result = Self::default();
        result.parse_into(input)?;
        Ok(result)
    }
}
