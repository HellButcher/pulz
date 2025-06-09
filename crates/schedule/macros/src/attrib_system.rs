use std::borrow::Cow;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, TokenStreamExt, format_ident, quote, quote_spanned};
use syn::{
    Expr, FnArg, Index, Pat, PatType, Path, PathArguments, Result, Token, Type,
    meta::ParseNestedMeta, parse::Parser, parse_quote_spanned, punctuated::Punctuated,
    spanned::Spanned,
};

use crate::utils::{self, Diagnostics, ParseAttributes, ReplaceAllLifetimes, ReplaceSelf};

mod kw {
    syn::custom_keyword!(unsend);
    syn::custom_keyword!(exclusive);
}

#[derive(Default, Debug)]
pub struct SystemParams {
    pub unsend: Option<kw::unsend>,
    pub exclusive: Option<kw::exclusive>,
    pub after: Punctuated<Path, Token![,]>,
    pub before: Punctuated<Path, Token![,]>,
    pub into: Option<Path>,
}

impl ParseAttributes for SystemParams {
    const IDENT: &'static str = "system";
    fn parse_nested_meta(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.path.is_ident("unsend") {
            if !meta.input.is_empty() {
                return Err(meta.error("Expected no arguments"));
            }
            self.unsend = Some(kw::unsend(meta.path.span()));
        } else if meta.path.is_ident("exclusive") {
            if !meta.input.is_empty() {
                return Err(meta.error("Expected no arguments"));
            }
            self.exclusive = Some(kw::exclusive(meta.path.span()));
        } else if meta.path.is_ident("after") {
            self.after = Punctuated::parse_terminated(meta.value()?)?;
        } else if meta.path.is_ident("before") {
            self.before = Punctuated::parse_terminated(meta.value()?)?;
        } else if meta.path.is_ident("into") || meta.path.is_ident("phase") {
            self.into = Some(meta.value()?.parse()?);
        } else {
            return Err(meta.error("Unknown attribute"));
        }
        Ok(())
    }
}

pub struct SystemGenerator<'a> {
    pub fn_item: &'a syn::ImplItemFn,
    pub params: SystemParams,
    pub self_ty: Option<Cow<'a, Type>>,
    pub private_ty: Option<&'a Type>,
    pub in_system_module: bool,
    system_ident: Ident,
    wrapper_ident: Ident,
    system_trait_ident: Ident,
}

impl<'a> SystemGenerator<'a> {
    pub fn new(fn_item: &'a syn::ImplItemFn, params: SystemParams) -> Result<Self> {
        let mut diagnostics = Diagnostics::new();
        if !fn_item.sig.generics.params.is_empty() {
            diagnostics.add(syn::Error::new_spanned(
                &fn_item.sig.generics,
                "System functions cannot have generic parameters",
            ));
        }
        if fn_item.sig.asyncness.is_some() {
            diagnostics.add(syn::Error::new_spanned(
                &fn_item.sig.asyncness,
                "System functions must not be async",
            ));
        }
        if !matches!(fn_item.sig.output, syn::ReturnType::Default) {
            diagnostics.add(syn::Error::new_spanned(
                &fn_item.sig.output,
                "System functions must not have a return type",
            ));
        }

        let span = fn_item.sig.ident.span();
        let system_ident = into_system_ident(&fn_item.sig.ident);
        let wrapper_ident = format_ident!("__syswrp_{}", fn_item.sig.ident, span = span);
        let system_trait_ident = if params.exclusive.is_some() {
            Ident::new("ExclusiveSystem", span)
        } else if params.unsend.is_none() {
            Ident::new("SendSystem", span)
        } else {
            Ident::new("System", span)
        };

        diagnostics.result(Self {
            fn_item,
            params,
            self_ty: None,
            private_ty: None,
            in_system_module: false,
            system_ident,
            wrapper_ident,
            system_trait_ident,
        })
    }

    fn get_option_type(path: &Path) -> Option<&Type> {
        if path.segments.len() != 1 || path.segments[0].ident != "Option" {
            return None;
        }
        if let PathArguments::AngleBracketed(args) = &path.segments[0].arguments {
            if args.args.len() == 1 {
                if let syn::GenericArgument::Type(ty) = &args.args[0] {
                    return Some(ty);
                }
            }
        }
        None
    }

    fn map_arg_type(ty: &Type, expr: &syn::Expr) -> (Type, Option<syn::Expr>, bool) {
        match ty {
            Type::Paren(ty) => return Self::map_arg_type(&ty.elem, expr),
            Type::Path(p) => {
                if let Some(ty) = Self::get_option_type(&p.path) {
                    let mapper_arg_ident = Ident::new("__mapped_arg", expr.span());
                    let mapper_expr = parse_quote_spanned!(expr.span() => #mapper_arg_ident);
                    let (argtype, argwrap, needmut) = Self::map_arg_type(ty, &mapper_expr);
                    return (
                        parse_quote_spanned!(argtype.span() => Option<#argtype>),
                        if let Some(nested_wrap) = argwrap {
                            Some(
                                parse_quote_spanned!(argtype.span() => #expr.map(|#mapper_arg_ident| #nested_wrap)),
                            )
                        } else {
                            None
                        },
                        needmut,
                    );
                }
            }
            Type::Reference(r) => {
                let ty = &r.elem;
                if r.mutability.is_some() {
                    return (
                        parse_quote_spanned!(r.span() => __pulz_schedule::resource::ResMut<'_, #ty>),
                        Some(
                            parse_quote_spanned!(r.span() => ::std::ops::DerefMut::deref_mut(&mut #expr)),
                        ),
                        true,
                    );
                } else {
                    return (
                        parse_quote_spanned!(r.span() => __pulz_schedule::resource::Res<'_, #ty>),
                        Some(parse_quote_spanned!(r.span() => ::std::ops::Deref::deref(&#expr))),
                        false,
                    );
                }
            }
            Type::Tuple(t) => {
                let mut ret = t.clone();
                let mut needmut = false;
                let mut needwrap = false;
                let mut wrapper = Vec::new();
                for (i, elem) in ret.elems.iter_mut().enumerate() {
                    let index: Index = Index::from(i);
                    let sub_expr = Expr::Field(parse_quote_spanned!(expr.span() => #expr.#index));
                    let (elem_type, elem_wrap, elem_needmut) = Self::map_arg_type(elem, &sub_expr);
                    *elem = elem_type;
                    needmut |= elem_needmut;
                    if let Some(wrap) = elem_wrap {
                        needwrap = true;
                        wrapper.push(wrap);
                    } else {
                        wrapper.push(sub_expr);
                    }
                }
                return (
                    Type::Tuple(ret),
                    if needwrap {
                        Some(parse_quote_spanned!(expr.span() => (#(#wrapper,)*)))
                    } else {
                        None
                    },
                    needmut,
                );
            }
            _ => {} // TODO: Tuple, Array
        }
        (ty.clone(), None, false)
    }

    fn map_arg(arg: &FnArg, expr: &Expr) -> (Type, Option<syn::Expr>, bool) {
        match arg {
            FnArg::Typed(pat) => Self::map_arg_type(&pat.ty, &expr),
            FnArg::Receiver(r) => Self::map_arg_type(&r.ty, &expr),
        }
    }

    fn get_arg_ident(arg: &FnArg, i: usize) -> Ident {
        match arg {
            FnArg::Typed(PatType { pat, .. }) => {
                if let Pat::Ident(pat_ident) = pat.as_ref() {
                    pat_ident.ident.clone()
                } else {
                    format_ident!("__arg_{i}", span = pat.span())
                }
            }
            FnArg::Receiver(r) => Ident::new("__self", r.span()),
        }
    }

    #[inline]
    pub fn system_ident(&self) -> &Ident {
        &self.system_ident
    }

    pub fn fn_expr_path(&self) -> syn::ExprPath {
        let ident = &self.fn_item.sig.ident;
        let span = ident.span();
        if let Some(self_ty) = &self.self_ty {
            parse_quote_spanned! { span => <#self_ty>::#ident }
        } else {
            parse_quote_spanned! { span => #ident }
        }
    }

    pub fn fn_wrapper_expr_path(&self) -> syn::ExprPath {
        let wrapper_ident = &self.wrapper_ident;
        let span = wrapper_ident.span();
        if let Some(private_ty) = self.private_ty {
            parse_quote_spanned! { span => <#private_ty>::#wrapper_ident }
        } else if let Some(self_ty) = &self.self_ty {
            parse_quote_spanned! { span => <#self_ty>::#wrapper_ident }
        } else {
            parse_quote_spanned! { span => #wrapper_ident }
        }
    }

    pub fn system_expr_path(&self) -> syn::ExprPath {
        let system_ident = &self.system_ident;
        let span = system_ident.span();
        if let Some(private_ty) = self.private_ty {
            parse_quote_spanned! { span => <#private_ty>::#system_ident }
        } else if let Some(self_ty) = &self.self_ty {
            parse_quote_spanned! { span => <#self_ty>::#system_ident }
        } else {
            parse_quote_spanned! { span => #system_ident }
        }
    }

    #[inline]
    pub fn system_trait_ident(&self) -> &Ident {
        &self.system_trait_ident
    }
}

impl ToTokens for SystemGenerator<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = &self.fn_item.sig.ident;
        let span = ident.span();
        let system_ident = &self.system_ident;
        let wrapper_ident = &self.wrapper_ident;
        let system_trait = &self.system_trait_ident;
        let inputs = &self.fn_item.sig.inputs;
        let mut require_wrapper = false;
        let mut argtypes = Vec::with_capacity(inputs.len());
        let mut argtypesstatic = Vec::with_capacity(inputs.len());
        let mut argpats = Vec::with_capacity(inputs.len());
        let mut argexprs = Vec::with_capacity(inputs.len());
        let static_lifetime = syn::Lifetime::new("'static", Span::mixed_site());
        for (i, arg) in inputs.iter().enumerate() {
            let arg_ident = Self::get_arg_ident(arg, i);
            let expr: syn::Expr = parse_quote_spanned!(arg_ident.span() => #arg_ident);
            let (mut argtype, argexpr, needmut) = Self::map_arg(arg, &expr);
            require_wrapper |= argexpr.is_some();
            if let Some(self_ty) = &self.self_ty {
                argtype = ReplaceSelf(self_ty).in_type(argtype);
            }
            argtypesstatic.push(ReplaceAllLifetimes(&static_lifetime).in_type(argtype.clone()));
            argtypes.push(argtype);
            argexprs.push(argexpr.unwrap_or(expr));
            if needmut {
                let arg_pat: Pat = parse_quote_spanned!(arg_ident.span() => mut #arg_ident );
                argpats.push(arg_pat);
            } else {
                let arg_pat: Pat = parse_quote_spanned!(arg_ident.span() => #arg_ident );
                argpats.push(arg_pat);
            }
        }

        let fn_self_path = self.fn_expr_path();
        let fn_or_wrapper_path = if require_wrapper {
            tokens.append_all(quote_spanned! { span =>
                #[doc(hidden)]
                fn #wrapper_ident(#(#argpats: #argtypes),*) {
                    #fn_self_path(#(#argexprs),*)
                }
            });
            self.fn_wrapper_expr_path()
        } else {
            fn_self_path
        };

        let fn_inner = quote_spanned! { span =>
            let s: __pulz_schedule::system::FuncSystem<_, (#(#argtypesstatic,)*)>
                = __pulz_schedule::system::FuncSystem::new(#fn_or_wrapper_path);
            s
        };

        if self.in_system_module {
            tokens.append_all(quote_spanned! { span =>
                #[doc(hidden)]
                const fn #system_ident() -> impl __pulz_schedule::system::#system_trait {
                    #fn_inner
                }
            });
        } else {
            tokens.append_all(fn_inner);
        }
    }
}

pub struct SystemInstallGenerator<'a>(pub &'a SystemGenerator<'a>);

impl ToTokens for SystemInstallGenerator<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let span = self.0.fn_item.sig.ident.span();
        let system_path = self.0.system_expr_path();
        let add_call = if self.0.params.exclusive.is_some() {
            quote! {
                __systems.add_system_exclusive(#system_path())
            }
        } else if self.0.params.unsend.is_some() {
            quote! {
                __systems.add_system_unsend(#system_path())
            }
        } else {
            quote! {
                __systems.add_system(#system_path())
            }
        };
        let into = self.0.params.into.iter();
        let after = self.0.params.after.iter();
        let before = self.0.params.before.iter();
        tokens.extend(quote_spanned! { span =>
            #add_call
              #(.parent(#into))*
              #(.after(#after))*
              #(.before(#before))*
            ;
        });
    }
}

pub fn into_system_ident(ident: &Ident) -> Ident {
    format_ident!("__{}__into_system", ident, span = ident.span())
}

pub fn into_system_path(mut path: Path) -> Path {
    if let Some(last) = path.segments.last_mut() {
        last.ident = into_system_ident(&last.ident);
    }
    path
}

pub fn attrib_system(attributes: TokenStream, mut input: syn::ImplItemFn) -> TokenStream {
    let crate_path = utils::CratePath::remove_from_attrs(&mut input.attrs).to_path();
    let mut output = input.to_token_stream();
    let mut diagnostics = Diagnostics::new();
    let mut params = SystemParams::default();
    diagnostics.add_if_err(params.parser().parse2(attributes));

    let Some(system) = diagnostics.add_if_err(SystemGenerator::new(&input, params)) else {
        output.extend(diagnostics.take_compile_errors());
        return output;
    };

    if let Some(errors) = diagnostics.take_compile_errors() {
        output.extend(errors);
        return output;
    }

    let vis = &input.vis;
    let system_ident = system.system_ident();
    let system_trait = system.system_trait_ident();

    output.extend(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case,unused_qualifications)]
        #vis const fn #system_ident () -> impl #crate_path::system::#system_trait
        {
            use #crate_path as __pulz_schedule;

            #system
        }
    });
    output
}
