use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Span, TokenStream, TokenTree};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use syn::{
    bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::{self, Token},
    DeriveInput, LitInt, Result, Token,
};

const DEFAULT_TO: usize = 20;

enum ItemTemplate {
    Itent(Ident),
    Index(Token![#]),
}

struct GeneratorArgs {
    _bracket_token: token::Bracket,
    from: usize,
    to: usize,
    items: Punctuated<ItemTemplate, Token![,]>,
}

pub struct VariadicTupleGenerator {
    macro_name: Ident,
    macro_args: Group,
    tail: TokenStream,
    args: GeneratorArgs,
}

impl ItemTemplate {
    fn gen_item(&self, index: usize) -> TokenTree {
        match self {
            Self::Index(_) => TokenTree::Literal(Literal::usize_unsuffixed(index)),
            Self::Itent(ident) => TokenTree::Ident(format_ident!("{}{}", ident, index)),
        }
    }
}

impl GeneratorArgs {
    fn gen_tuple(&self, index: usize) -> TokenStream {
        if self.items.len() == 1 {
            self.items[0].gen_item(index).into()
        } else {
            let items = self.items.iter().map(|item| item.gen_item(index));
            quote! {
                (#(#items),*)
            }
        }
    }
}

impl VariadicTupleGenerator {
    fn gen_invocation(&self, to: usize, items: &[TokenStream]) -> TokenStream {
        let macro_name = &self.macro_name;
        let provided_macro_args = self.macro_args.stream();
        let items_slice = &items[0..to];
        let tail = &self.tail;
        let macro_args_delim = self.macro_args.delimiter();
        let all_macro_args_group = Group::new(
            macro_args_delim,
            quote! {
                #provided_macro_args
                [#(#items_slice),*]
            },
        );
        quote! {
            #macro_name! #all_macro_args_group #tail
        }
    }
}

impl Default for GeneratorArgs {
    fn default() -> Self {
        let mut items = Punctuated::new();
        items.push(ItemTemplate::Itent(Ident::new("T", Span::call_site())));
        Self {
            _bracket_token: token::Bracket::default(),
            from: 0,
            to: DEFAULT_TO,
            items,
        }
    }
}

impl Parse for ItemTemplate {
    fn parse(input: ParseStream) -> Result<Self> {
        if let Ok(hash_token) = input.parse::<Token![#]>() {
            Ok(Self::Index(hash_token))
        } else {
            Ok(Self::Itent(input.parse::<Ident>()?))
        }
    }
}

fn parse_range(input: ParseStream) -> Result<(usize, usize)> {
    let value1 = input.parse::<LitInt>()?.base10_parse()?;
    Ok(if input.parse::<Token![..=]>().is_ok() {
        let value2 = input.parse::<LitInt>()?.base10_parse()?;
        (value2, value2 + 1)
    } else if input.parse::<Token![..]>().is_ok() {
        if let Ok(value2) = input.parse::<LitInt>() {
            let value2 = value2.base10_parse()?;
            (value1, value2)
        } else {
            (value1, DEFAULT_TO)
        }
    } else {
        (0, value1)
    })
}

impl Parse for GeneratorArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        let _bracket_token = bracketed!(content in input);
        let (from, to) = if content.peek(LitInt) {
            parse_range(&content)?
        } else {
            (0, DEFAULT_TO)
        };
        let items = Punctuated::parse_terminated(&content)?;
        Ok(Self {
            _bracket_token,
            from,
            to,
            items,
        })
    }
}

impl Parse for VariadicTupleGenerator {
    fn parse(input: ParseStream) -> Result<Self> {
        let args = if input.peek(token::Bracket) {
            input.parse::<GeneratorArgs>()?
        } else {
            GeneratorArgs::default()
        };
        let macro_name = input.parse::<Ident>()?;
        input.parse::<Token![!]>()?;
        if input.is_empty() {
            Ok(VariadicTupleGenerator {
                args,
                macro_name,
                macro_args: Group::new(Delimiter::Brace, TokenStream::new()),
                tail: TokenStream::new(),
            })
        } else {
            Ok(VariadicTupleGenerator {
                args,
                macro_name,
                macro_args: input.parse()?,
                tail: input.parse()?,
            })
        }
    }
}

impl ToTokens for VariadicTupleGenerator {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if self.args.to < self.args.from {
            return;
        }
        let items: Vec<_> = (self.args.from..self.args.to)
            .map(|i| self.args.gen_tuple(i))
            .collect();
        for i in 0..=items.len() {
            tokens.extend(self.gen_invocation(i, &items));
        }
    }
}
