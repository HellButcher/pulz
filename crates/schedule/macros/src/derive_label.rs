use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{
    Error, Ident, ItemEnum, Path, Token, parse::Parser, parse_quote, punctuated::Punctuated,
};

use crate::utils::Diagnostics;

pub fn derive_label(attributes: TokenStream, mut input: ItemEnum) -> TokenStream {
    let mut diagnostics = Diagnostics::new();
    let labels = diagnostics
        .add_if_err(Punctuated::<Path, Token![,]>::parse_terminated.parse2(attributes))
        .unwrap_or_default();
    if labels.is_empty() {
        diagnostics.add(Error::new(Span::call_site(), "labels must be specified"));
    }
    for v in input.variants.iter() {
        if v.fields != syn::Fields::Unit {
            diagnostics.add(Error::new_spanned(v, "variants must be unit variants"));
        }
    }
    let mut required_derives = vec!["Copy", "Clone", "Debug", "PartialEq", "Eq", "Hash"];
    for attr in input.attrs.iter() {
        if attr.path().is_ident("derive")
            && let Ok(derives) =
                attr.parse_args_with(<Punctuated<Path, Token![,]>>::parse_terminated)
        {
            for derive in derives {
                if let Some(pos) = required_derives.iter().position(|d| derive.is_ident(d)) {
                    required_derives.remove(pos);
                }
            }
        }
    }

    let ident = &input.ident;
    let enum_name = ident.to_string();

    if !required_derives.is_empty() {
        let required_derives: Vec<_> = required_derives
            .iter()
            .map(|d| Ident::new(d, Span::call_site()))
            .collect();
        input.attrs.push(parse_quote! {
            #[derive(#(#required_derives),*)]
        });
    }

    let mut output = input.to_token_stream();
    if let Some(errors) = diagnostics.take_compile_errors() {
        output.extend(errors);
        return output;
    }

    // Build label strings at macro expansion time: "EnumName::Variant"
    let variant_idents: Vec<_> = input.variants.iter().map(|v| &v.ident).collect();
    let variant_labels: Vec<String> = input
        .variants
        .iter()
        .map(|v| {
            let vname = v.ident.to_string();
            format!("{enum_name}::{vname}")
        })
        .collect();

    // Prefix used for stripping: "EnumName::" (lowercased)
    let prefix_lower = format!("{enum_name}::").to_ascii_lowercase();

    output.extend(quote! {
        impl #ident {
            pub fn as_str(&self) -> &'static str {
                match self {
                    #(
                        Self::#variant_idents => #variant_labels,
                    )*
                }
            }
        }

        impl ::core::str::FromStr for #ident {
            type Err = ();

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                // Matching order (first match wins):
                // 1. Full label — e.g. s == "MyEnum::First"
                // 2. Stripped form — s starts with "myenum::", remainder matches variant name
                //    case-insensitively, so that just the variant name also works as input.
                #(
                    if #variant_labels.eq_ignore_ascii_case(s) {
                        return Ok(Self::#variant_idents);
                    }
                    // Check for stripped form: "EnumName::Variant" -> compare just "Variant".
                    if s.len() > #prefix_lower.len() && s[..#prefix_lower.len()].eq_ignore_ascii_case(#prefix_lower.as_str()) {
                        let rest = &s[#prefix_lower.len()..];
                        if stringify!(#variant_idents).eq_ignore_ascii_case(rest) {
                            return Ok(Self::#variant_idents);
                        }
                    }
                )*
                Err(())
            }
        }
    });

    for label in labels {
        output.extend(quote! {
            impl #label for #ident {
                fn as_str(&self) -> &'static str {
                    self.as_str()
                }
            }
        });
    }

    output
}
