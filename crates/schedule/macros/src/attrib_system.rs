use std::borrow::Cow;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, TokenStreamExt, format_ident, quote, quote_spanned};
use syn::{
    Expr, FnArg, Generics, Index, Pat, PatType, Path, PathArguments, Result, Token, Type,
    meta::ParseNestedMeta, parse::Parser, parse_quote_spanned, punctuated::Punctuated,
    spanned::Spanned, visit::Visit,
};

use crate::utils::{
    self, Diagnostics, ParseAttributes, ReduceBoundGenerics, ReplaceAllLifetimes, ReplaceSelf,
};

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

struct SystemArgGenerator<'a> {
    pub arg: &'a FnArg,
    pub index: usize,
    pub ident: Cow<'a, Ident>,
    pub needs_mut: bool,
    pub mapped_type: Type,
    pub maping_expr: syn::Expr,
}

impl<'a> SystemArgGenerator<'a> {
    pub fn new(arg: &'a FnArg, index: usize) -> Self {
        let ident = Self::get_arg_ident(arg, index);
        let span = ident.span();
        let expr: syn::Expr = parse_quote_spanned!(span => #ident);
        let (mapped_type, maping_expr, needs_mut) = Self::map_arg(arg, &expr);
        Self {
            arg,
            index,
            ident,
            needs_mut,
            mapped_type,
            maping_expr: maping_expr.unwrap_or(expr),
        }
    }

    pub fn ident(&self) -> &Ident {
        &self.ident
    }

    pub fn needs_mut(&self) -> bool {
        self.needs_mut
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

    fn get_arg_ident(arg: &FnArg, i: usize) -> Cow<'_, Ident> {
        match arg {
            FnArg::Typed(PatType { pat, .. }) => {
                if let Pat::Ident(pat_ident) = pat.as_ref() {
                    Cow::Borrowed(&pat_ident.ident)
                } else {
                    Cow::Owned(format_ident!("__arg_{i}", span = pat.span()))
                }
            }
            FnArg::Receiver(r) => Cow::Owned(Ident::new("__self", r.span())),
        }
    }
}

pub struct SystemGenerator<'a> {
    pub fn_item: &'a syn::ImplItemFn,
    pub params: SystemParams,
    pub crate_path: &'a Path,
    self_ty: Option<(&'a Type, &'a Generics)>,
    system_ident: Ident,
    system_trait_ident: Ident,
    args: Vec<SystemArgGenerator<'a>>,
}

impl<'a> SystemGenerator<'a> {
    pub fn new(
        fn_item: &'a syn::ImplItemFn,
        params: SystemParams,
        crate_path: &'a Path,
    ) -> Result<Self> {
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
        let system_trait_ident = if params.exclusive.is_some() {
            Ident::new("ExclusiveSystem", span)
        } else if params.unsend.is_none() {
            Ident::new("SendSystem", span)
        } else {
            Ident::new("System", span)
        };

        let args = fn_item
            .sig
            .inputs
            .iter()
            .enumerate()
            .map(|(i, arg)| SystemArgGenerator::new(arg, i))
            .collect::<Vec<_>>();

        diagnostics.result(Self {
            fn_item,
            params,
            self_ty: None,
            system_ident,
            system_trait_ident,
            args,
            crate_path,
        })
    }

    pub fn set_self_ty(&mut self, self_ty: &'a Type, generics: &'a Generics) {
        self.self_ty = Some((self_ty, generics));
        for arg in &mut self.args {
            arg.mapped_type = ReplaceSelf(self_ty).in_type(arg.mapped_type.clone())
        }
    }

    #[inline]
    pub fn system_ident(&self) -> &Ident {
        &self.system_ident
    }

    pub fn fn_expr_path(&self) -> syn::ExprPath {
        let ident = &self.fn_item.sig.ident;
        let span = ident.span();
        if let Some((self_ty, _)) = &self.self_ty {
            parse_quote_spanned! { span => <#self_ty>::#ident }
        } else {
            parse_quote_spanned! { span => #ident }
        }
    }

    pub fn system_expr_path(&self) -> syn::ExprPath {
        let system_ident = &self.system_ident;
        let span = system_ident.span();
        if let Some((self_ty, _)) = &self.self_ty {
            parse_quote_spanned! { span => <#self_ty>::#system_ident }
        } else {
            parse_quote_spanned! { span => #system_ident }
        }
    }

    #[inline]
    pub fn system_trait_ident(&self) -> &Ident {
        &self.system_trait_ident
    }

    pub fn bound_generics(&self) -> Generics {
        let mut generics = if let Some((_, generics_self)) = self.self_ty {
            generics_self.clone()
        } else {
            Generics::default()
        };
        generics
            .params
            .extend(self.fn_item.sig.generics.params.iter().cloned());
        if let Some(where_clause) = self.fn_item.sig.generics.where_clause.as_ref() {
            generics
                .make_where_clause()
                .predicates
                .extend(where_clause.predicates.iter().cloned());
        }
        let mut visitor = ReduceBoundGenerics::new(&generics);
        for arg in &self.args {
            visitor.visit_type(&arg.mapped_type);
        }
        visitor.get()
    }
}

impl ToTokens for SystemGenerator<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = &self.fn_item.sig.ident;
        let span = ident.span();

        let generics = self.bound_generics();
        let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
        let type_generics_turbofish = type_generics.as_turbofish();

        let argidents: Vec<_> = self.args.iter().map(|arg| &arg.ident).collect();
        let argtypes: Vec<_> = self.args.iter().map(|arg| &arg.mapped_type).collect();
        let static_lifetime = syn::Lifetime::new("'static", Span::mixed_site());
        let argtypesstatic: Vec<_> = argtypes
            .iter()
            .map(|argtype| ReplaceAllLifetimes(&static_lifetime).in_type((*argtype).clone()))
            .collect();
        let argpats = self.args.iter().map(|arg| {
            let ident = arg.ident();
            let pat: Pat = if arg.needs_mut() {
                parse_quote_spanned!(arg.ident.span() => mut #ident)
            } else {
                parse_quote_spanned!(arg.ident.span() => #ident)
            };
            pat
        });
        let argexprs = self.args.iter().map(|arg| &arg.maping_expr);

        let fn_self_path = self.fn_expr_path();

        tokens.extend(quote_spanned! { span =>
            struct __Data #impl_generics #where_clause {
                #(
                    #argidents: <#argtypesstatic as __pulz_schedule::system::SystemData>::Data,
                )*
            }
            struct __System #impl_generics #where_clause {
                __data: Option<__Data #type_generics>,
                // TODO: local vars and args
            }

            impl #impl_generics __pulz_schedule::system::SystemInit for __System #type_generics #where_clause {
                fn init(&mut self, res: &mut __pulz_schedule::resource::Resources) {
                    self.__data = Some(__Data {
                        #(
                            #argidents: <#argtypesstatic as __pulz_schedule::system::SystemData>::init(res),
                        )*
                    });
                }

                fn system_type_name(&self) -> &'static str {
                    ::std::any::type_name_of_val(&#fn_self_path)
                }
            }
        });

        let (datatrait, dataget) = if self.params.unsend.is_some() {
            (
                Ident::new("SystemData", Span::call_site()),
                Ident::new("get", Span::call_site()),
            )
        } else {
            (
                Ident::new("SystemDataSend", Span::call_site()),
                Ident::new("get_send", Span::call_site()),
            )
        };

        let run_impl = quote_spanned! { span =>
            let __data = self.__data.as_mut().expect("System not initialized");
            #(
                let #argpats = <#argtypes as __pulz_schedule::system::#datatrait>::#dataget(__res, &mut __data.#argidents);
            )*
            #fn_self_path ( #(#argexprs),* );
        };
        let updateaccess_impl = quote_spanned! { span =>
            let __data = self.__data.as_ref().expect("System not initialized");
            #(
                <#argtypesstatic as __pulz_schedule::system::SystemData>::update_access(__res, __access, &__data.#argidents);
            )*
        };

        if self.params.exclusive.is_some() {
            tokens.extend(quote_spanned! { span =>
                impl #impl_generics __pulz_schedule::system::ExclusiveSystem for __System #type_generics #where_clause {
                    fn run_exclusive(&mut self, __res: &mut __pulz_schedule::resource::Resources) {
                        #run_impl
                    }
                }
            });
        } else {
            if self.params.unsend.is_some() {
                tokens.extend(quote_spanned! { span =>
                    impl #impl_generics __pulz_schedule::system::System for __System #type_generics #where_clause {
                        fn run(&mut self, __res: &__pulz_schedule::resource::Resources) {
                            #run_impl
                        }

                        fn update_access(&self, __res: &__pulz_schedule::resource::Resources, __access: &mut __pulz_schedule::resource::ResourceAccess) {
                            #updateaccess_impl
                        }
                    }
                });
            } else {
                tokens.extend(quote_spanned! { span =>
                    impl #impl_generics __pulz_schedule::system::System for __System #type_generics #where_clause {
                        #[inline]
                        fn run(&mut self, __res: &__pulz_schedule::resource::Resources) {
                            __pulz_schedule::system::SendSystem::run_send(self, __res);
                        }

                        fn update_access(&self, __res: &__pulz_schedule::resource::Resources, __access: &mut __pulz_schedule::resource::ResourceAccess) {
                            #updateaccess_impl
                        }
                    }
                    impl #impl_generics __pulz_schedule::system::SendSystem for __System #type_generics #where_clause {
                        fn run_send(&mut self, __res: &__pulz_schedule::resource::ResourcesSend) {
                            #run_impl
                        }
                    }
                });
            }

            tokens.extend(quote_spanned! { span =>
                __System #type_generics_turbofish {
                    __data: None,
                    // TODO
                }
            });
        }
    }
}

pub struct IntoSystemGenerator<'a>(pub &'a SystemGenerator<'a>);

impl ToTokens for IntoSystemGenerator<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let crate_path = self.0.crate_path;
        let vis = &self.0.fn_item.vis;
        let span = self.0.fn_item.sig.ident.span();
        let system_ident = &self.0.system_ident;
        let system_trait = &self.0.system_trait_ident;
        let system = self.0;
        tokens.append_all(quote_spanned! { span =>
            #[doc(hidden)]
            #[allow(non_snake_case,unused_qualifications)]
            #vis const fn #system_ident() -> impl #crate_path::system::#system_trait {
                use #crate_path as __pulz_schedule;

                #system
            }
        });
    }
}

pub struct InstallSystemGenerator<'a>(pub &'a SystemGenerator<'a>);

impl ToTokens for InstallSystemGenerator<'_> {
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

    let Some(system) = diagnostics.add_if_err(SystemGenerator::new(&input, params, &crate_path))
    else {
        output.extend(diagnostics.take_compile_errors());
        return output;
    };

    if let Some(errors) = diagnostics.take_compile_errors() {
        output.extend(errors);
        return output;
    }

    IntoSystemGenerator(&system).to_tokens(&mut output);

    output
}
