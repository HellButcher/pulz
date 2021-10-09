use std::ops::{Deref, DerefMut};

use proc_macro2::{Span, TokenStream};
use proc_macro_crate::FoundCrate;
use quote::ToTokens;
use syn::{Error, Path, Result, Token, parse::{Parse, ParseStream}, parse_quote};

pub fn resolve_crate(name: &str) -> Result<Path> {
    match proc_macro_crate::crate_name(name).map_err(|e| Error::new(Span::call_site(), e))? {
        FoundCrate::Itself => Ok(parse_quote!(crate)),
        FoundCrate::Name(name) => Ok(parse_quote!(::#name)),
    }
}

pub trait AttributeKeyword: Parse {
    type Arg: Parse;
    const EMPTY: bool = false;
    const NAME: &'static str;

    fn is(ident: &proc_macro2::Ident) -> bool {
        ident == Self::NAME
    }

    fn from_ident(ident: &proc_macro2::Ident) -> Option<Self>;

    #[inline]
    fn parse_if(ident: &proc_macro2::Ident, input: &ParseStream) -> Result<Option<Attr<Self>>> {
        Attr::parse_if(ident, input)
    }
}

pub struct Attr<A: AttributeKeyword> {
    pub arg: A::Arg,
    pub ident: A,
}

impl<A: AttributeKeyword> Deref for Attr<A> {
    type Target = A::Arg;
    fn deref(&self) -> &Self::Target {
        &self.arg
    }
}
impl<A: AttributeKeyword> DerefMut for Attr<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.arg
    }
}

impl<A: AttributeKeyword> Attr<A> {
    pub fn parse_if(ident: &proc_macro2::Ident, input: &ParseStream) -> Result<Option<Self>> {
        if let Some(ident) = A::from_ident(ident) {
            let arg = Self::parse_args(input)?;
            Ok(Some(Attr { arg, ident }))
        } else {
            Ok(None)
        }
    }

    fn parse_args(input: &ParseStream) -> Result<A::Arg> {
        if !A::EMPTY {
            input.parse::<Token![=]>()?;
        }
        input.parse::<A::Arg>()
    }
}

impl<A: AttributeKeyword> Parse for Attr<A> {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident = input.parse::<A>()?;
        let arg = Self::parse_args(&input)?;
        Ok(Attr { arg, ident })
    }
}

impl<A> ToTokens for Attr<A>
    where A: AttributeKeyword + ToTokens, A::Arg: ToTokens
{
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ident.to_tokens(tokens);
        if !A::EMPTY {
            <Token![=]>::default().to_tokens(tokens);
        }
        self.arg.to_tokens(tokens);
    }
}

macro_rules! attribute_kw {
    ($ident:ident $(: $arg:ty)?) => {
        syn::custom_keyword!($ident);

        impl $crate::utils::AttributeKeyword for $ident {
            type Arg = attribute_kw!(@arg $($arg , )? syn::parse::Nothing , );
            const EMPTY: bool = attribute_kw!(@empty $(false $arg , )? true syn::parse::Nothing , );
            const NAME: &'static str = stringify!($ident);

            fn from_ident(ident: &proc_macro2::Ident) -> Option<Self> {
                if Self::is(ident) {
                    Some($ident(ident.span()))
                } else {
                    None
                }
            }
        }
    };

    (@arg $arg:ty , $($rest:tt)*) => {
        $arg
    };
    (@empty $arg:ident $($rest:tt)*) => {
        $arg
    };
}
