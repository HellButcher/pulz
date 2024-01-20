use darling::{export::NestedMeta, Error, FromMeta, Result, ToTokens};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, TokenStreamExt};
use syn::{Attribute, Data, DeriveInput, Expr, Fields, Lit, LitInt, Meta};

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

    let input_struct = match input.data {
        Data::Struct(input_struct) => input_struct,
        Data::Enum(_) => return Err(Error::unsupported_shape_with_expected("enum", &"struct")),
        Data::Union(_) => return Err(Error::unsupported_shape_with_expected("union", &"struct")),
    };
    match input_struct.fields {
        Fields::Unit => {}
        Fields::Named(_fields) => {
            // TODO: named fields
        }
        Fields::Unnamed(fields) => {
            if fields.unnamed.len() == 1 {
                // TODO: newtype / wrapper
            } else {
                return Err(Error::unsupported_shape_with_expected(
                    "tuple",
                    &"named struct",
                ));
            }
        }
    }

    if let Some(BindingLayoutArgs {
        binding_type,
        binding_index,
        options: _,
    }) = BindingLayoutArgs::from_attributes(&input.attrs)?
    {
        if binding_type != BindingType::Uniform {
            return Err(Error::custom("only uniform is allowed on struct").with_span(&ident));
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
    binding_index: LitInt,
    options: BindingLayoutOptions,
}

#[derive(FromMeta, Default, Clone, Eq, PartialEq, Debug)]
pub struct BindingLayoutOptions {}

impl BindingLayoutArgs {
    fn from_attributes(attrs: &[Attribute]) -> Result<Option<Self>> {
        let mut errors = Error::accumulator();
        let mut binding_type = None;
        let mut binding_index = None;
        let mut meta_list = Vec::new();
        for attr in attrs {
            if let Some(ident) = attr.path().get_ident() {
                if let Some(t) = BindingType::from_ident(ident) {
                    if binding_type.is_none() {
                        binding_type = Some(t);
                    } else if binding_type != Some(t) {
                        errors.push(
                            Error::custom("only a single attribute is allowed").with_span(ident),
                        );
                    }
                    match &attr.meta {
                        Meta::List(meta) => {
                            let mut items = darling::export::NestedMeta::parse_meta_list(
                                meta.tokens.to_token_stream(),
                            )?;
                            if let Some(NestedMeta::Lit(Lit::Int(index))) = items.first() {
                                if binding_index.is_none() {
                                    binding_index = Some(index.clone());
                                } else {
                                    errors.push(
                                        Error::duplicate_field("binding_index").with_span(index),
                                    );
                                }
                                meta_list.extend_from_slice(&items[1..]);
                            } else {
                                meta_list.append(&mut items);
                            }
                        }
                        Meta::NameValue(meta) => {
                            if binding_index.is_none() {
                                if let Some(index) = errors.handle(parse_index(&meta.value)) {
                                    binding_index = Some(index);
                                }
                            } else {
                                errors.push(
                                    Error::duplicate_field("binding_index").with_span(&meta.value),
                                );
                            }
                        }
                        Meta::Path(_path) => {}
                    }
                }
            }
        }
        errors.finish()?;
        let Some(binding_type) = binding_type else {
            return Ok(None);
        };
        let Some(binding_index) = binding_index else {
            return Err(Error::missing_field("binding_index"));
        };
        let options = BindingLayoutOptions::from_list(&meta_list)?;
        Ok(Some(Self {
            binding_type,
            binding_index,
            options,
        }))
    }
}

fn parse_index(expr: &Expr) -> Result<LitInt> {
    if let Expr::Lit(lit) = expr {
        if let Lit::Int(index) = &lit.lit {
            Ok(index.clone())
        } else {
            Err(Error::unexpected_lit_type(&lit.lit))
        }
    } else {
        Err(Error::unexpected_expr_type(expr))
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
                binding_index: LitInt::new("2", Span::call_site()),
                options: BindingLayoutOptions {},
            })
        );
    }
}
