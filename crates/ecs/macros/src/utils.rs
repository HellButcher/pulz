use proc_macro2::Span;
use proc_macro_crate::FoundCrate;
use syn::{parse_quote, Error, Path, Result};

pub fn resolve_crate(name: &str) -> Result<Path> {
    match proc_macro_crate::crate_name(name).map_err(|e| Error::new(Span::call_site(), e))? {
        FoundCrate::Itself => Ok(parse_quote!(crate)),
        FoundCrate::Name(name) => Ok(parse_quote!(::#name)),
    }
}
