use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    DeriveInput, Error, LitBool, Path, Result, meta::ParseNestedMeta, parse_quote,
    parse_quote_spanned,
};

use crate::utils::{Diagnostics, ParseAttributes, ParseNestedMetaExt};

pub fn derive_component(input: DeriveInput) -> TokenStream {
    let mut diagnostics = Diagnostics::new();
    let mut params = ComponentStructParams::default();
    diagnostics.add_if_err(params.parse_attributes(&input.attrs));
    diagnostics.add_if_err(params.validate());

    if let Some(errors) = diagnostics.take_compile_errors() {
        return errors;
    }

    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let storage = params.storage();
    let mut storage: syn::Type = parse_quote!(#storage<Self>);
    if params.tracked.value {
        storage = parse_quote!(::pulz_ecs::storage::Tracked<#storage>);
    }

    quote! {
        impl #impl_generics ::pulz_ecs::component::Component for #ident #ty_generics #where_clause {
            type Storage = #storage;
        }
    }
}

pub struct ComponentStructParams {
    sparse: Option<LitBool>,
    tracked: LitBool,
    storage: Option<Path>,
}

impl Default for ComponentStructParams {
    fn default() -> Self {
        let span = Span::call_site();
        Self {
            sparse: None,
            tracked: LitBool::new(false, span),
            storage: None,
        }
    }
}

impl ParseAttributes for ComponentStructParams {
    const IDENT: &'static str = "component";
    fn parse_nested_meta(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.is_attr("sparse") {
            self.sparse = Some(meta.get_flag()?);
        } else if meta.is_attr("tracked") {
            self.tracked = meta.get_flag()?;
        } else {
            return Err(meta.error("Unknown attribute"));
        }
        Ok(())
    }
}

impl ComponentStructParams {
    fn storage(&self) -> Path {
        if let Some(storage) = &self.storage {
            if let Some(single_ident) = storage.get_ident() {
                parse_quote_spanned!(single_ident.span() => ::pulz_ecs::storage::#single_ident)
            } else {
                storage.clone()
            }
        } else if let Some(sparse) = &self.sparse
            && sparse.value
        {
            parse_quote_spanned!(sparse.span() => ::pulz_ecs::storage::SparseStorage)
        } else {
            parse_quote!(::pulz_ecs::storage::ArchetypeStorage)
        }
    }

    fn validate(&self) -> Result<()> {
        if self.sparse.is_some() && self.storage.is_some() {
            const MSG: &str = "either provide `sparse` or `storage`, but not both!";
            return Err(Error::new(Span::call_site(), MSG));
        }
        Ok(())
    }
}
