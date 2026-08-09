use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{Error, LitBool, Result, spanned::Spanned};

pub trait ParseAttributes {
    const IDENT: &'static str;

    fn parse_nested_meta(&mut self, meta: syn::meta::ParseNestedMeta) -> Result<()>;

    fn parse_attribute(&mut self, attr: &syn::Attribute) -> Result<bool> {
        if attr.path().is_ident(Self::IDENT) {
            if !matches!(attr.meta, syn::Meta::Path(_)) {
                attr.parse_nested_meta(|meta| self.parse_nested_meta(meta))?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn parse_attributes(&mut self, attrs: &[syn::Attribute]) -> Result<bool> {
        let mut found = false;
        for attr in attrs {
            found |= self.parse_attribute(attr)?;
        }
        Ok(found)
    }

    fn remove_from_attributes(&mut self, attrs: &mut Vec<syn::Attribute>) -> Result<bool> {
        let found = self.parse_attributes(attrs)?;
        if found {
            attrs.retain(|attr| !attr.path().is_ident(Self::IDENT));
        }
        Ok(found)
    }

    fn parser(&mut self) -> impl syn::parse::Parser<Output = ()> {
        syn::meta::parser(|meta| self.parse_nested_meta(meta))
    }
}

pub trait ParseNestedMetaExt {
    fn get_flag(&self) -> Result<LitBool>;

    fn is_attr(&self, name: &str) -> bool;
}

impl ParseNestedMetaExt for syn::meta::ParseNestedMeta<'_> {
    fn get_flag(&self) -> Result<LitBool> {
        if self.input.is_empty() {
            Ok(LitBool::new(true, self.path.span()))
        } else {
            self.value()?.parse()
        }
    }

    fn is_attr(&self, name: &str) -> bool {
        self.path.is_ident(name)
    }
}

#[derive(Debug, Default)]
pub struct Diagnostics(Option<Error>);

impl Diagnostics {
    #[inline]
    pub const fn new() -> Self {
        Self(None)
    }

    #[inline]
    pub fn add(&mut self, err: Error) {
        match self.0.as_mut() {
            Some(e) => e.combine(err),
            None => self.0 = Some(err),
        }
    }

    pub fn add_if_err<T>(&mut self, result: Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(err) => {
                self.add(err);
                None
            }
        }
    }

    pub fn wrap_result<T>(&mut self, result: Result<T>) -> Result<T> {
        match result {
            Ok(value) => Ok(value),
            Err(err) => {
                self.add(err);
                Err(self.take_error().unwrap())
            }
        }
    }

    #[inline]
    pub fn is_ok(&self) -> bool {
        self.0.is_none()
    }

    #[inline]
    pub fn is_err(&self) -> bool {
        !self.0.is_some()
    }

    pub fn result<T>(&mut self, success: T) -> Result<T> {
        self.take_result()?;
        Ok(success)
    }

    pub fn take_error(&mut self) -> Option<Error> {
        self.0.take()
    }

    pub fn take_result(&mut self) -> Result<()> {
        if let Some(err) = self.0.take() {
            Err(err)
        } else {
            Ok(())
        }
    }

    pub fn take_compile_errors(&mut self) -> Option<TokenStream> {
        self.0.take().map(|err| err.into_compile_error())
    }
}

impl From<Diagnostics> for Result<()> {
    #[inline]
    fn from(mut d: Diagnostics) -> Self {
        d.take_result()
    }
}

impl From<Diagnostics> for Option<Error> {
    #[inline]
    fn from(mut d: Diagnostics) -> Option<Error> {
        d.take_error()
    }
}

impl ToTokens for Diagnostics {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(err) = &self.0 {
            tokens.extend(err.to_compile_error());
        }
    }
}
