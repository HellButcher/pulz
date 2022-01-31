use darling::{FromMeta, ToTokens};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, TokenStreamExt};
use syn::{Attribute, DeriveInput, Error, Lit, Meta, NestedMeta, Result};

use crate::utils::resolve_render_crate;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BindingType {
    Uniform,
    Texture,
    Sampler,
}

impl BindingType {
    fn from_ident(ident: &Ident) -> Option<Self> {
        if ident == "uniform" {
            Some(Self::Uniform)
        } else if ident == "texture" {
            Some(Self::Texture)
        } else if ident == "sampler" {
            Some(Self::Sampler)
        } else {
            None
        }
    }
}

impl ToTokens for BindingType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = match self {
            Self::Uniform => "uniform",
            Self::Texture => "texture",
            Self::Sampler => "sampler",
        };
        tokens.append(Ident::new(name, Span::call_site()))
    }
}

pub fn derive_as_binding_layout(input: DeriveInput) -> Result<TokenStream> {
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let crate_render = resolve_render_crate()?;

    let mut layouts = Vec::new();

    if let Some(BindingLayoutArgs {
        binding_type,
        binding_index,
        options: _,
    }) = BindingLayoutArgs::from_attributes(&input.attrs)?
    {
        if binding_type != BindingType::Uniform {
            return Err(Error::new(
                ident.span(),
                "only uniform is allowed on struct",
            ));
        }

        layouts.push(quote! {
            #crate_render::pipeline::BindGroupLayoutEntry {
                binding: #binding_index,
                binding_type: #binding_type,
                stages: #crate_render::pipeline::ShaderStages::ALL, // TODO,
            }
        });
    }

    Ok(quote! {
        impl #impl_generics #crate_render::pipeline::AsBindingLayout for #ident #ty_generics #where_clause {

        }
    })
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct BindingLayoutArgs {
    binding_type: BindingType,
    binding_index: Lit,
    options: BindingLayoutOptions,
}

#[derive(FromMeta, Default, Clone, Eq, PartialEq, Debug)]
pub struct BindingLayoutOptions {}

impl BindingLayoutArgs {
    fn from_attributes(attribs: &[Attribute]) -> Result<Option<Self>> {
        let mut binding_type = None;
        let mut binding_index = None;
        let mut options = BindingLayoutOptions::default();
        for attr in attribs {
            if let Some(ident) = attr.path.get_ident() {
                if let Some(t) = BindingType::from_ident(ident) {
                    if binding_type.is_some() {
                        return Err(Error::new(
                            ident.span(),
                            "only a single attribute is allowed",
                        ));
                    }
                    binding_type = Some(t);
                    match attr.parse_meta()? {
                        Meta::List(meta) => {
                            let path = meta.path;
                            let mut it = meta.nested.into_iter();
                            if let Some(NestedMeta::Lit(lit)) = it.next() {
                                binding_index = Some(lit);
                                let more_args: Vec<_> = it.collect();
                                options = BindingLayoutOptions::from_list(&more_args)?;
                            } else {
                                return Err(Error::new_spanned(path, "a binding-index is missing"));
                            }
                        }
                        Meta::NameValue(meta) => {
                            binding_index = Some(meta.lit);
                        }
                        Meta::Path(path) => {
                            return Err(Error::new_spanned(path, "a binding-index is missing"));
                        }
                    }
                    // TODO: parse index
                }
            }
        }
        if let Some(binding_type) = binding_type {
            Ok(Some(BindingLayoutArgs {
                binding_type,
                binding_index: binding_index.unwrap(),
                options,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use syn::{DeriveInput, FieldsNamed, LitInt};

    use super::*;

    #[test]
    fn test_empty() {
        // with specified MyAttr:
        let derive_input = syn::parse_str(
            r#"
            #[derive(AsBindingLayout)]
            struct Foo;
        "#,
        )
        .unwrap();
        let _result = derive_as_binding_layout(derive_input).unwrap();
        // TODO: assert
    }

    #[test]
    fn test_newtype() {
        let derive_input = syn::parse_str(
            r#"
            #[derive(AsBindingLayout)]
            struct Foo(Bar);
        "#,
        )
        .unwrap();
        let _result = derive_as_binding_layout(derive_input).unwrap();
        // TODO: assert
    }

    #[test]
    fn test_fail_tuple() {
        let derive_input = syn::parse_str(
            r#"
            #[derive(AsBindingLayout)]
            struct Foo(Alice,Bob);
        "#,
        )
        .unwrap();
        let result = derive_as_binding_layout(derive_input);
        match result {
            Ok(tokens) => {
                panic!("Expected error, got: {}", tokens);
            }
            Err(_err) => {
                // TODO: assert
            }
        }
    }

    #[test]
    fn test_fail_enum() {
        let derive_input = syn::parse_str(
            r#"
            #[derive(AsBindingLayout)]
            enum Foo{Bar}
        "#,
        )
        .unwrap();
        let result = derive_as_binding_layout(derive_input);
        match result {
            Ok(tokens) => {
                panic!("Expected error, got: {}", tokens);
            }
            Err(_err) => {
                // TODO: assert
            }
        }
    }

    #[test]
    fn test_struct() {
        // with no MyAttr:
        let derive_input: DeriveInput = syn::parse_str(
            r#"
            #[derive(AsBindingLayout)]
            #[myderive()]
            struct Foo{
                a: Bar,
                b: Blub,
            }
        "#,
        )
        .unwrap();
        let _result = derive_as_binding_layout(derive_input).unwrap();
        // TODO: assert
    }

    #[test]
    fn test_parse_args() {
        let fields: FieldsNamed = syn::parse_str(
            r#"{
            #[uniform(2)]
            pub foo: u32,
        }"#,
        )
        .unwrap();
        let args = BindingLayoutArgs::from_attributes(&fields.named[0].attrs).unwrap();
        assert_eq!(
            args,
            Some(BindingLayoutArgs {
                binding_type: BindingType::Uniform,
                binding_index: Lit::Int(LitInt::new("2", Span::call_site())),
                options: BindingLayoutOptions {},
            })
        );
    }
}
