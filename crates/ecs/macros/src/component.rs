use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_quote, Attribute, DeriveInput, Ident, Path, Result, Token,
};

use crate::utils::resolve_crate;

pub fn derive_component(input: DeriveInput) -> Result<TokenStream> {
    let args = ComponentStructArgs::parse_attributes(&input.attrs)?;
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let crate_ecs = resolve_crate("pulz-ecs")?;

    // TODO: dynamic package path
    let storage = if let Some(storage) = args.storage {
        storage
    } else if args.sparse {
        parse_quote!(#crate_ecs::storage::HashMapStorage)
    } else {
        parse_quote!(#crate_ecs::storage::DenseStorage)
    };
    Ok(quote! {
        impl #impl_generics #crate_ecs::component::Component for #ident #ty_generics #where_clause  {
            type Storage = #storage<Self>;
        }
    })
}

#[derive(Default)]
struct ComponentStructArgs {
    sparse: bool,
    storage: Option<Path>,
}

fn parse_attr<P: Parse>(input: ParseStream) -> Result<P> {
    input.parse::<Token![=]>()?;
    P::parse(input)
}

impl ComponentStructArgs {
    fn parse_attributes(input: &[Attribute]) -> Result<Self> {
        let mut result = Self::default();
        for attrib in input {
            if attrib.path.is_ident("component") {
                attrib.parse_args_with(|input: ParseStream| result.parse_into(input))?;
            }
        }
        Ok(result)
    }
    fn parse_into(&mut self, input: ParseStream) -> Result<()> {
        while !input.is_empty() {
            let ident = Ident::parse(input)?;
            if ident == "sparse" {
                self.sparse = true;
            } else if ident == "storage" {
                self.storage = Some(parse_attr(input)?);
            }
            if input.is_empty() {
                break;
            }
            <Token![,]>::parse(input)?;
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
