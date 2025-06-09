use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, TokenStreamExt, quote};
use syn::{
    ItemImpl, LitBool, Path, Result, Visibility,
    meta::ParseNestedMeta,
    parse::{Parse, Parser},
    parse_quote,
    spanned::Spanned,
};

use crate::{
    attrib_system::{InstallSystemGenerator, IntoSystemGenerator, SystemGenerator, SystemParams},
    utils::{self, Diagnostics, ParseAttributes},
};

#[derive(Clone, Debug)]
pub struct IdentWithVisibility(pub Visibility, pub Ident);

/// something, that has a default behaviour, that can be disabled by defining `false`.
/// (the default behaviour can be explicitly enabled by defining `true`; but this is the default).
/// Or the default behaviour can be explicitly overidden by providing a value `T`.
#[derive(Clone, Debug)]
pub enum DefaultBehaviourDisableable<T> {
    Default(Option<Span>),
    Disabled(Span),
    Enabled(T),
}

impl<T> Default for DefaultBehaviourDisableable<T> {
    fn default() -> Self {
        Self::Default(None)
    }
}

impl<T> DefaultBehaviourDisableable<T> {
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Default(_))
    }
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled(_))
    }
    pub fn or_else(self, f: impl FnOnce() -> T) -> T {
        match self {
            Self::Enabled(value) => value,
            _ => f(),
        }
    }
}

#[derive(Default, Debug)]
pub struct SystemModuleParams {
    pub schedule: Option<Path>,
    pub install_fn: DefaultBehaviourDisableable<IdentWithVisibility>,
}

impl Parse for IdentWithVisibility {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let vis: Visibility = input.parse()?;
        let ident: Ident = input.parse()?;
        Ok(Self(vis, ident))
    }
}

impl<T: Parse> Parse for DefaultBehaviourDisableable<T> {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        if input.is_empty() {
            Ok(Self::Default(None))
        } else if input.peek(LitBool) {
            let lit = input.parse::<LitBool>()?;
            if lit.value {
                Ok(Self::Default(Some(lit.span())))
            } else {
                Ok(Self::Disabled(lit.span()))
            }
        } else {
            Ok(Self::Enabled(input.parse()?))
        }
    }
}

impl ParseAttributes for SystemModuleParams {
    const IDENT: &'static str = "system_module";
    fn parse_nested_meta(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.path.is_ident("schedule") {
            self.schedule = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("install_fn") {
            self.install_fn = meta.value()?.parse()?;
        } else {
            return Err(meta.error("Unknown attribute"));
        }
        Ok(())
    }
}

pub struct SystemModuleGenerator<'a> {
    pub item_impl: &'a ItemImpl,
    pub params: SystemModuleParams,
    pub systems: Vec<SystemGenerator<'a>>,
    pub crate_path: &'a Path,
}

impl<'a> SystemModuleGenerator<'a> {
    pub fn new(
        item_impl: &'a mut ItemImpl,
        params: SystemModuleParams,
        crate_path: &'a Path,
    ) -> Result<Self> {
        let mut diagnostics = Diagnostics::new();
        if let Some(defaultness) = item_impl.defaultness.as_ref() {
            diagnostics.add(syn::Error::new_spanned(
                defaultness,
                "Default system modules are not supported",
            ));
        }
        if let Some(unsafety) = item_impl.unsafety.as_ref() {
            diagnostics.add(syn::Error::new_spanned(
                unsafety,
                "Unsafe system modules are not supported",
            ));
        }
        if let Some((_, trait_, _)) = item_impl.trait_.as_ref() {
            diagnostics.add(syn::Error::new_spanned(
                trait_,
                "System modules must be an `impl` block, not a trait impl",
            ));
        }
        let systems_params = item_impl
            .items
            .iter_mut()
            .enumerate()
            .filter_map(|(i, item)| {
                match item {
                    syn::ImplItem::Fn(fn_item) => {
                        let mut params = SystemParams::default();
                        if let Some(true) = diagnostics
                            .add_if_err(params.remove_from_attributes(&mut fn_item.attrs))
                        {
                            Some((i, params))
                        } else {
                            None
                        }
                    }
                    syn::ImplItem::Type(syn::ImplItemType { attrs, .. })
                    | syn::ImplItem::Const(syn::ImplItemConst { attrs, .. })
                    | syn::ImplItem::Macro(syn::ImplItemMacro { attrs, .. }) => {
                        attrs.retain(|attr| {
                            if attr.path().is_ident(SystemParams::IDENT) {
                                diagnostics.add(syn::Error::new(
                                    attr.span(),
                                    "system attribute must be on a function",
                                ));
                                false // Remove the attribute from the item
                            } else {
                                true
                            }
                        });
                        None
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        let systems = systems_params
            .into_iter()
            .filter_map(|(i, params)| {
                if let syn::ImplItem::Fn(fn_item) = &item_impl.items[i] {
                    match SystemGenerator::new(fn_item, params, crate_path) {
                        Ok(mut system) => {
                            system.set_self_ty(&item_impl.self_ty, &item_impl.generics);
                            Some(system)
                        }
                        Err(e) => {
                            diagnostics.add(e);
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        diagnostics.result(Self {
            item_impl,
            params,
            systems,
            crate_path,
        })
    }
}

impl ToTokens for SystemModuleGenerator<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let item_impl = &self.item_impl;
        let (impl_generics, _generic_type_args, where_clause) = item_impl.generics.split_for_impl();
        let self_ty = &item_impl.self_ty;
        let install_fn_disabled = self.params.install_fn.is_disabled();
        let IdentWithVisibility(install_fn_vis, install_fn) =
            self.params.install_fn.clone().or_else(|| {
                IdentWithVisibility(
                    Visibility::Inherited,
                    Ident::new("install_systems", Span::call_site()),
                )
            });

        let systems = self.systems.iter().map(IntoSystemGenerator);

        let install_fn = if install_fn_disabled {
            TokenStream::new()
        } else {
            let schedule_ty = self
                .params
                .schedule
                .clone()
                .unwrap_or_else(|| parse_quote!(__pulz_schedule::schedule::Schedule));

            let install_impl = self.systems.iter().map(InstallSystemGenerator);
            quote! {
                #install_fn_vis fn #install_fn (__systems: &mut #schedule_ty) {
                    #(#install_impl)*
                }
            }
        };

        tokens.append_all(quote! {
            impl #impl_generics #self_ty
                #where_clause
            {
                // TODO: allow to hide systems?
                #(#systems)*

                #install_fn
            }
        });
    }
}

pub fn attrib_system_module(attributes: TokenStream, mut input: syn::ItemImpl) -> TokenStream {
    let crate_path = utils::CratePath::remove_from_attrs(&mut input.attrs).to_path();
    let mut diagnostics = Diagnostics::new();
    let mut params = SystemModuleParams::default();
    if let Err(e) = params.parser().parse2(attributes) {
        diagnostics.add(e);
    }

    let Some(module_impl) =
        diagnostics.add_if_err(SystemModuleGenerator::new(&mut input, params, &crate_path))
    else {
        let mut output = input.to_token_stream();
        output.extend(diagnostics.take_compile_errors());
        return output;
    };
    let input = module_impl.item_impl;
    let mut output = input.to_token_stream();
    if let Some(errors) = diagnostics.take_compile_errors() {
        output.extend(errors);
        return output;
    }

    output.extend(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case,unused_qualifications)]
        const _: () = {
            use #crate_path as __pulz_schedule;

            #module_impl
        };
    });
    output
}
