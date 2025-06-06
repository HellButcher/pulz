use proc_macro_crate::FoundCrate;
use proc_macro2::{Ident, Span};
use syn::{Error, Path, Result, Token};

pub fn resolve_crate(name: &str) -> Result<Path> {
    match proc_macro_crate::crate_name(name).map_err(|e| Error::new(Span::call_site(), e))? {
        FoundCrate::Itself => Ok(Path::from(Ident::new("crate", Span::call_site()))),
        FoundCrate::Name(name) => {
            let mut path: Path = Ident::new(&name, Span::call_site()).into();
            path.leading_colon = Some(Token![::](Span::call_site()));
            Ok(path)
        }
    }
}
