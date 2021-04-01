use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream};

use syn::{
    braced,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Comma, Dot2},
    Attribute, Expr, Ident, Path, Result, Token,
};

use super::hex_struct_field::HexStructField;
use super::READER_UUID;

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
        let fields = Punctuated::parse_terminated(&content)?;

        {
            let mut iter = fields.iter().peekable();
            while let Some(field) = iter.next() {
                if let (HexStructField::Match { .. }, Some(next @ HexStructField::Match { .. })) =
                    (field, iter.peek())
                {
                    return Err(syn::Error::new(
                        field
                            .span()
                            .join(next.span())
                            .unwrap_or_else(|| field.span()),
                        "consecutive `_` patterns are not allowed",
                    ));
                }
            }
        }

        let mut dot2_token = None;
        let mut rest = None;
        if content.peek(Token![..]) {
            dot2_token.replace(content.parse()?);
            rest.replace(content.parse()?);
        }

        Ok(Self {
            reader,
            attrs,
            path,
            brace,
            fields,
            dot2_token,
            rest,
        })
    }
}

impl ToTokens for HexStruct {
    fn to_tokens(&self, output_tokens: &mut TokenStream) {
        let mut tokens = TokenStream::new();
        let tokens = &mut tokens;

        let HexStruct {
            reader,
            attrs,
            path,
            fields,
            dot2_token,
            rest,
            ..
        } = self;

        // setup reader
        let reader_ident = Ident::new(READER_UUID, reader.span());
        quote_spanned!(reader.span()=>
             #[allow(non_snake_case)]
             let mut #reader_ident = #reader;
        )
        .to_tokens(tokens);

        // setup iterator and handle the first `_` ocurrence
        let mut iter = fields.iter().peekable();
        if let Some(HexStructField::Match { .. }) = iter.peek() {
            iter.next().unwrap().to_tokens(None).to_tokens(tokens);
        }

        let mut ok_tokens = TokenStream::new();
        {
            // struct setup
            quote! (
                #(#attrs)*
                #path
            )
            .to_tokens(&mut ok_tokens);

            // struct fields
            self.brace.surround(&mut ok_tokens, |tokens| {
                while let Some(field) = iter.next() {
                    match field {
                        HexStructField::Match { .. } => (), // handled previously
                        HexStructField::Field { .. } => {
                            field.to_tokens(iter.peek()).to_tokens(tokens);
                        }
                    }
                }
            });

            // .. rest
            quote!(#dot2_token#rest).to_tokens(&mut ok_tokens);
        }
        quote!(Ok(#ok_tokens)).to_tokens(tokens);

        quote!(
            (|| {
                #tokens
            })()
        )
        .to_tokens(output_tokens);
    }
}
