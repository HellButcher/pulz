use proc_macro_crate::FoundCrate;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{
    Error, Ident, Lifetime, Result, punctuated::Punctuated, spanned::Spanned, visit::Visit,
    visit_mut::VisitMut,
};

pub struct CratePath(pub Option<syn::Path>);

impl CratePath {
    const CRATE_NAME: &str = "pulz_schedule";
    const IDENT: &'static str = "__crate_path";
    pub fn remove_from_attrs(attrs: &mut Vec<syn::Attribute>) -> Self {
        if let Some(pos) = attrs
            .iter()
            .position(|attr| attr.path().is_ident(Self::IDENT))
        {
            let attr = attrs.remove(pos);
            Self(Some(attr.parse_args::<syn::Path>().unwrap()))
        } else {
            Self(None)
        }
    }

    pub fn from_attrs(attrs: &[syn::Attribute]) -> Self {
        if let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident(Self::IDENT)) {
            Self(Some(attr.parse_args::<syn::Path>().unwrap()))
        } else {
            Self(None)
        }
    }

    pub fn to_path(&self) -> syn::Path {
        self.0.clone().unwrap_or_else(Self::default_path)
    }

    pub fn default_path() -> syn::Path {
        fn mk_path(ident: &str) -> syn::Path {
            let mut p = syn::Path::from(syn::Ident::new(ident, proc_macro2::Span::call_site()));
            if ident != "crate" {
                p.leading_colon = Some(syn::Token![::](proc_macro2::Span::call_site()));
            }
            p
        }
        proc_macro_crate::crate_name(Self::CRATE_NAME)
            .ok()
            .map(|found| match found {
                FoundCrate::Itself => mk_path("crate"),
                FoundCrate::Name(name) => mk_path(&name),
            })
            .unwrap_or_else(|| mk_path(Self::CRATE_NAME))
    }
}

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
        return syn::meta::parser(|meta| self.parse_nested_meta(meta));
    }
}

#[derive(Copy, Clone)]
pub struct ReplaceAllLifetimes<'a>(pub &'a syn::Lifetime);

impl syn::visit_mut::VisitMut for ReplaceAllLifetimes<'_> {
    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident != "'static" {
            *lifetime = self.0.clone();
        }
    }
}

impl<'a> ReplaceAllLifetimes<'a> {
    pub fn in_type(mut self, mut ty: syn::Type) -> syn::Type {
        self.visit_type_mut(&mut ty);
        ty
    }
}

#[derive(Clone)]
pub struct ReplaceSpecificLifetimes<'a>(Vec<&'a syn::Lifetime>, pub &'a syn::Lifetime);

impl syn::visit_mut::VisitMut for ReplaceSpecificLifetimes<'_> {
    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident != "'static" {
            for param in self.0.iter().copied() {
                if param == lifetime {
                    *lifetime = self.1.clone();
                }
            }
        }
    }
}

impl<'a> ReplaceSpecificLifetimes<'a> {
    pub const fn new(lt: &'a syn::Lifetime) -> Self {
        Self(Vec::new(), lt)
    }

    pub fn with_generics(mut self, generics: &'a syn::Generics) -> Self {
        for param in &generics.params {
            if let syn::GenericParam::Lifetime(lt) = param {
                self.0.push(&lt.lifetime);
            }
        }
        self
    }
    pub fn in_generics(&mut self, mut g: syn::Generics) -> syn::Generics {
        self.visit_generics_mut(&mut g);
        g
    }
    pub fn in_type(&mut self, mut ty: syn::Type) -> syn::Type {
        self.visit_type_mut(&mut ty);
        ty
    }
}

pub fn remove_all_lifetimes(generics: &syn::Generics) -> syn::Generics {
    let static_lt = syn::Lifetime::new("'static", generics.span());
    remove_all_lifetimes_with(generics, &static_lt)
}

pub fn remove_all_lifetimes_with(
    generics: &syn::Generics,
    new_lt: &syn::Lifetime,
) -> syn::Generics {
    let mut replace_with_static = ReplaceSpecificLifetimes::new(new_lt).with_generics(generics);
    syn::Generics {
        lt_token: generics.lt_token,
        params: generics
            .params
            .iter()
            .filter_map(|param| {
                if let syn::GenericParam::Lifetime(_) = param {
                    None
                } else {
                    let mut param = param.clone();
                    replace_with_static.visit_generic_param_mut(&mut param);
                    Some(param)
                }
            })
            .collect(),
        gt_token: generics.gt_token,
        where_clause: generics.where_clause.as_ref().map(|wc| syn::WhereClause {
            where_token: wc.where_token,
            predicates: wc
                .predicates
                .iter()
                .filter_map(|pred| {
                    if let syn::WherePredicate::Lifetime(_) = pred {
                        None
                    } else {
                        let mut pred = pred.clone();
                        replace_with_static.visit_where_predicate_mut(&mut pred);
                        Some(pred)
                    }
                })
                .collect(),
        }),
    }
}

#[derive(Copy, Clone)]
pub struct ReplaceSelf<'a>(pub &'a syn::Type);

impl VisitMut for ReplaceSelf<'_> {
    fn visit_type_mut(&mut self, i: &mut syn::Type) {
        if let syn::Type::Path(p) = i {
            if p.path.is_ident("Self") {
                *i = self.0.clone();
                return;
            }
        }
        syn::visit_mut::visit_type_mut(self, i);
    }
}

impl ReplaceSelf<'_> {
    pub fn in_type(mut self, mut ty: syn::Type) -> syn::Type {
        self.visit_type_mut(&mut ty);
        ty
    }
}

pub struct ReduceBoundGenerics<'a> {
    pub source: &'a syn::Generics,
    pub included: Vec<bool>,
}

impl<'a> ReduceBoundGenerics<'a> {
    pub fn new(source: &'a syn::Generics) -> Self {
        let mut included = Vec::new();
        included.resize(source.params.len(), false);
        Self { source, included }
    }

    pub fn mark_type_ident(&mut self, ident: &Ident) -> bool {
        for (i, param) in self.source.params.iter().enumerate() {
            if let syn::GenericParam::Type(p) = param {
                if p.ident == *ident {
                    self.included[i] = true;
                    return true;
                }
            }
        }
        false
    }
    pub fn mark_const_ident(&mut self, ident: &Ident) -> bool {
        for (i, param) in self.source.params.iter().enumerate() {
            if let syn::GenericParam::Const(p) = param {
                if p.ident == *ident {
                    self.included[i] = true;
                    return true;
                }
            }
        }
        false
    }
    pub fn mark_lifetime(&mut self, lt: &Lifetime) -> bool {
        for (i, param) in self.source.params.iter().enumerate() {
            if let syn::GenericParam::Lifetime(p) = param {
                if p.lifetime == *lt {
                    self.included[i] = true;
                    return true;
                }
            }
        }
        false
    }

    pub fn get(&self) -> syn::Generics {
        let mut params = Punctuated::new();
        for (i, param) in self.source.params.iter().enumerate() {
            if self.included[i] {
                params.push(param.clone());
            }
        }
        let where_clause = self
            .source
            .where_clause
            .as_ref()
            .map(|wc| syn::WhereClause {
                where_token: wc.where_token,
                predicates: wc
                    .predicates
                    .iter()
                    .filter_map(|pred| {
                        if let syn::WherePredicate::Type(t) = pred {
                            if let syn::Type::Path(p) = &t.bounded_ty {
                                if let Some(i) = p.path.get_ident() {
                                    if params.iter().any(|param| {
                                        if let syn::GenericParam::Type(tp) = param {
                                            tp.ident == *i
                                        } else {
                                            false
                                        }
                                    }) {
                                        return Some(pred.clone());
                                    }
                                }
                            }
                        }
                        None
                    })
                    .collect(),
            });
        syn::Generics {
            lt_token: self.source.lt_token,
            params,
            gt_token: self.source.gt_token,
            where_clause,
        }
    }
}

impl Visit<'_> for ReduceBoundGenerics<'_> {
    fn visit_generic_param(&mut self, _param: &syn::GenericParam) {
        // don't walk down the generic parameters
    }
    fn visit_lifetime(&mut self, lt: &syn::Lifetime) {
        self.mark_lifetime(lt);
    }
    fn visit_type_path(&mut self, t: &syn::TypePath) {
        if let Some(ident) = t.path.get_ident() {
            if t.qself.is_none() && self.mark_type_ident(ident) {
                return;
            }
        }
        syn::visit::visit_type_path(self, t);
    }
    fn visit_expr_path(&mut self, e: &syn::ExprPath) {
        if let Some(ident) = e.path.get_ident() {
            if e.qself.is_none() && self.mark_const_ident(ident) {
                return;
            }
        }
        syn::visit::visit_expr_path(self, e);
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
        if let Some(err) = self.0.take() {
            Some(err.into_compile_error())
        } else {
            None
        }
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
