use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};

use syn::{
    braced,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Comma, Dot2},
    Attribute, Expr, Path, Result, Token,
};

use super::{hex_struct_field::HexStructField, internal_ident};

#[derive(Debug)]
pub struct HexStruct {
    reader: Expr,
    attrs: Vec<Attribute>,
    path: Path,
    brace: Brace,
    fields: Punctuated<HexStructField, Comma>,
    dot2_token: Option<Dot2>,
    rest: Option<Box<Expr>>,
}

impl Parse for HexStruct {
    fn parse(input: ParseStream) -> Result<Self> {
        let reader = input.parse()?;
        input.parse::<Token![=>]>()?;

        let attrs = Attribute::parse_outer(input)?;
        let path = input.parse()?;
        let content;
        let brace = braced!(content in input);
        let mut fields = Punctuated::new();

        while !content.is_empty() {
            if content.peek(Token![..]) {
                return Ok(Self {
                    reader,
                    attrs,
                    path,
                    brace,
                    fields,
                    dot2_token: Some(content.parse()?),
                    rest: if content.is_empty() {
                        None
                    } else {
                        Some(Box::new(content.parse()?))
                    },
                });
            }

            fields.push(content.parse()?);
            if content.is_empty() {
                break;
            }
            let punct: Token![,] = content.parse()?;
            fields.push_punct(punct);
        }

        Ok(HexStruct {
            reader,
            attrs,
            path,
            brace,
            fields,
            dot2_token: None,
            rest: None,
        })
    }
}

impl ToTokens for HexStruct {
    fn to_tokens(&self, output_stream: &mut TokenStream) {
        let mut closure_stream = TokenStream::new();
        self.brace.surround(&mut closure_stream, |stream| {
            let HexStruct {
                reader,
                attrs,
                path,
                fields,
                dot2_token,
                rest,
                ..
            } = self;

            // setup
            let array_ident = internal_ident("ARRAY", reader.span());
            let len = fields
                .iter()
                .map(|field| field.byte_pattern().len())
                .max()
                .unwrap_or_default();

            let reader_ident = internal_ident("READER", reader.span());
            quote!(
                 use std::convert::TryInto;

                 #[allow(non_snake_case)]
                 let mut #reader_ident = #reader;

                 #[allow(non_snake_case)]
                 let mut #array_ident: [u8; #len] = [0; #len];
            )
            .to_tokens(stream);

            for field in fields {
                field.to_tokens(stream);
            }

            let mut struct_stream = TokenStream::new();
            let struct_stream = &mut struct_stream;
            {
                // struct fields
                for pair in fields.pairs() {
                    let field = pair.value();
                    let comma = pair.punct();

                    if !field.is_struct_member() {
                        continue;
                    } else {
                        field.to_instantiation_tokens(struct_stream);
                        comma.to_tokens(struct_stream);
                    }
                }
                // .. rest
                dot2_token.to_tokens(struct_stream);
                rest.to_tokens(struct_stream);
            }

            // struct setup
            quote!(
                Ok(#(#attrs)* #path { #struct_stream })
            )
            .to_tokens(stream);
        });

        quote!(
            (|| { #closure_stream })()
        )
        .to_tokens(output_stream);
    }
}
