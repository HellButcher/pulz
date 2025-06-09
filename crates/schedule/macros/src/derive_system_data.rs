use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{
    Data, DeriveInput, Error, Path, Result, meta::ParseNestedMeta, parse_quote_spanned,
    punctuated::Punctuated, spanned::Spanned,
};

use crate::utils::{self, Diagnostics, ParseAttributes, ReplaceSpecificLifetimes};

mod kw {
    syn::custom_keyword!(skip);
    syn::custom_keyword!(unsend);
}

#[derive(Default, Debug)]
pub struct SystemDataContainerParams {
    pub unsend: Option<kw::unsend>,
}

#[derive(Default, Debug)]
pub struct SystemDataParams {
    skip: Option<(kw::skip, Option<Path>)>,
}

impl ParseAttributes for SystemDataContainerParams {
    const IDENT: &'static str = "system_data";
    fn parse_nested_meta(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.path.is_ident("unsend") {
            self.unsend = Some(kw::unsend(meta.path.span()));
        } else {
            return Err(meta.error("Unknown attribute"));
        }
        Ok(())
    }
}

impl ParseAttributes for SystemDataParams {
    const IDENT: &'static str = "system_data";
    fn parse_nested_meta(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.path.is_ident("skip") {
            if let Ok(path) = meta.value() {
                self.skip = Some((kw::skip(meta.path.span()), Some(path.parse()?)));
            } else {
                self.skip = Some((kw::skip(meta.path.span()), None));
            }
        } else {
            return Err(meta.error("Unknown attribute"));
        }
        Ok(())
    }
}

pub fn derive_system_data(mut input: DeriveInput) -> TokenStream {
    let crate_path = utils::CratePath::remove_from_attrs(&mut input.attrs).to_path();
    let mut diagnostics = Diagnostics::new();
    let mut params = SystemDataContainerParams::default();
    diagnostics.add_if_err(params.parse_attributes(&input.attrs));

    let Data::Struct(data) = &input.data else {
        diagnostics.add(Error::new_spanned(
            &input.ident,
            "SystemData can only be derived for structs",
        ));
        return diagnostics.take_compile_errors().unwrap_or_default();
    };
    let field_params = data
        .fields
        .iter()
        .map(|field| {
            let mut field_params = SystemDataParams::default();
            diagnostics.add_if_err(field_params.parse_attributes(&field.attrs));
            field_params
        })
        .collect::<Vec<_>>();
    if let Some(errors) = diagnostics.take_compile_errors() {
        return errors;
    }

    let ident = &input.ident;
    let vis = input.vis;
    let replaced_lifetime = syn::Lifetime::new("'__x", Span::mixed_site());
    let getter_lifetime = syn::Lifetime::new("'__y", Span::mixed_site());
    let static_lifetime = syn::Lifetime::new("'static", Span::mixed_site());
    let mut replace_with_new_replaced_lt =
        ReplaceSpecificLifetimes::new(&replaced_lifetime).with_generics(&input.generics);
    let mut replace_with_static_lt =
        ReplaceSpecificLifetimes::new(&static_lifetime).with_generics(&input.generics);

    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let generics_new_lt = replace_with_new_replaced_lt.in_generics(input.generics.clone());
    let (_, type_generics_new_lifetime, _) = generics_new_lt.split_for_impl();

    let generics_data = utils::remove_all_lifetimes(&input.generics);
    let (_, type_generics_data, _) = generics_data.split_for_impl();

    let mut send_where_clause = where_clause.cloned();
    let send_wc = send_where_clause.get_or_insert_with(|| syn::WhereClause {
        where_token: Default::default(),
        predicates: Punctuated::new(),
    });
    send_wc.predicates.push(parse_quote_spanned!(
        input.ident.span() => Self: Send
    ));
    send_wc.predicates.push(parse_quote_spanned!(
        input.ident.span() => __Data #type_generics_data: Send + Sync
    ));
    for param in generics_data.type_params() {
        let ident = &param.ident;
        send_wc.predicates.push(parse_quote_spanned!(
            input.ident.span() => #ident: Send + Sync
        ));
    }

    let mut data_idents = Vec::new();
    let mut data_types = Vec::new();
    let mut arg_update_access = Vec::new();
    let mut arg_idents = Vec::new();
    let mut arg_gets = Vec::new();
    let mut arg_gets_send = Vec::new();
    for (field, field_params) in data.fields.iter().zip(field_params.iter()) {
        if let Some(ident) = &field.ident {
            arg_idents.push(ident);
        }
        if let Some((kw_skip, path)) = &field_params.skip {
            let arg_get = if let Some(default) = path {
                quote_spanned! { default.span() => #default() }
            } else {
                quote_spanned! { kw_skip.span() => ::std::default::Default::default() }
            };
            arg_gets.push(arg_get.clone());
            if params.unsend.is_none() {
                arg_gets_send.push(arg_get);
            }
            continue;
        }
        let static_type = replace_with_static_lt.in_type(field.ty.clone());
        let (acces_data, acces_data_mut) = if let Some(ident) = &field.ident {
            data_idents.push(ident);
            (
                quote_spanned! { ident.span() => &_data.#ident },
                quote_spanned! { ident.span() => &mut _data.#ident },
            )
        } else {
            let index = data_types.len() - 1;
            (
                quote_spanned! { field.span() => &_data.#index },
                quote_spanned! { field.span() => &mut _data.#index },
            )
        };
        arg_update_access.push(quote_spanned! { field.span() => <#static_type as __pulz_schedule::system::SystemData>::update_access(_resources, _access, #acces_data); });
        arg_gets.push(quote_spanned! { field.span() => <#static_type as __pulz_schedule::system::SystemData>::get(_resources, #acces_data_mut) });

        if params.unsend.is_none() {
            let replaced_lt_type = replace_with_new_replaced_lt.in_type(field.ty.clone());
            arg_gets_send.push(quote_spanned! { field.span() => <#static_type as __pulz_schedule::system::SystemDataSend>::get_send(_resources, #acces_data_mut) });
            send_wc.predicates.push(parse_quote_spanned!(
                field.span() => #static_type: for<#replaced_lifetime> __pulz_schedule::system::SystemDataSend<Arg<#replaced_lifetime> = #replaced_lt_type>
            ));
        }

        data_types.push(static_type);
    }

    let (data_impl, data_init, arg_get, arg_get_send) = match data.fields {
        syn::Fields::Named(_) => (
            quote! {
                #[doc(hidden)]
                #vis struct __Data #generics_data {
                    #(#data_idents: <#data_types as __pulz_schedule::system::SystemData>::Data,)*
                }
            },
            quote! {
                __Data {
                    #(#data_idents: <#data_types as __pulz_schedule::system::SystemData>::init(_resources),)*
                }
            },
            quote! {
                #ident {
                    #(#arg_idents: #arg_gets,)*
                }
            },
            quote! {
                #ident {
                    #(#arg_idents: #arg_gets_send,)*
                }
            },
        ),
        syn::Fields::Unnamed(_) => (
            quote! {
                #[doc(hidden)]
                #vis struct __Data #generics_data (
                    #(#data_types,)*
                )
            },
            quote! {
                __Data(
                    #(<#data_types as __pulz_schedule::system::SystemData>::init(_resources),)*
                )
            },
            quote! {
                #ident{
                    #(#arg_idents: #arg_gets,)*
                }
            },
            quote! {
                #ident(
                    #(#arg_gets_send,)*
                )
            },
        ),
        syn::Fields::Unit => (
            quote! {
                #[doc(hidden)]
                #vis struct __Data;
            },
            quote! {
                __Data
            },
            quote! {
                Self
            },
            quote! {
                Self
            },
        ),
    };

    let send_impl = if params.unsend.is_none() {
        quote! {
            impl #impl_generics __pulz_schedule::system::SystemDataSend for  #ident #type_generics
                #send_where_clause
            {
                fn get_send<#getter_lifetime>(_resources: &#getter_lifetime __pulz_schedule::resource::ResourcesSend, _data: &#getter_lifetime mut Self::Data) -> Self::Arg<#getter_lifetime> {
                    #arg_get_send
                }
            }
        }
    } else {
        TokenStream::new()
    };

    // Generate the code for the derive macro
    quote::quote! {
        #[doc(hidden)]
        #[allow(unused_qualifications)]
        const _: () = {
            use #crate_path as __pulz_schedule;

            #data_impl

            #[automatically_derived]
            impl #impl_generics __pulz_schedule::system::SystemData for  #ident #type_generics
                #where_clause
            {
                type Data = __Data #type_generics_data;
                type Arg<#replaced_lifetime> = #ident #type_generics_new_lifetime;
                fn init(_resources: &mut __pulz_schedule::resource::Resources) -> Self::Data {
                    #data_init
                }
                fn update_access(_resources: &__pulz_schedule::resource::Resources, _access: &mut __pulz_schedule::resource::ResourceAccess, _data: &Self::Data) {
                    #(#arg_update_access)*
                }
                fn get<#getter_lifetime>(_resources: &#getter_lifetime __pulz_schedule::resource::Resources, _data: &#getter_lifetime mut Self::Data) -> Self::Arg<#getter_lifetime> {
                    #arg_get
                }
            }

            #send_impl
        };
    }
}
