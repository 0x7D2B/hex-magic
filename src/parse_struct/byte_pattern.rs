use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::fmt;
use syn::parse::{Parse, ParseStream};

use crate::hex_string::{HexString, HexValue};

use syn::{
    bracketed,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Bracket, Comma},
    Attribute, Expr, LitByteStr, LitStr, Result,
};

#[derive(Debug)]
pub enum BytePattern {
    Array {
        attrs: Vec<Attribute>,
        bracket: Bracket,
        elems: Punctuated<Expr, Comma>,
    },
    HexString(HexString),
    LitByteStr(LitByteStr),
}
impl BytePattern {
    pub fn len(&self) -> usize {
        match self {
            Self::Array { elems, .. } => elems.len(),
            Self::HexString(hex) => hex.len(),
            Self::LitByteStr(bstr) => bstr.value().len(),
        }
    }
}
impl fmt::Display for BytePattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Array { elems, .. } => write!(
                f,
                "[{}]",
                elems
                    .iter()
                    .map(|t| {
                        let s = quote!(#t).to_string();
                        s.parse::<u8>().map(|u| format!("{:02X}", u)).unwrap_or(s)
                    })
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Self::HexString(hex) => write!(f, "{}", hex),
            Self::LitByteStr(bstr) => write!(f, "{}", quote!(#bstr)),
        }
    }
}

impl Parse for BytePattern {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(LitByteStr) {
            Ok(Self::LitByteStr(input.parse::<LitByteStr>()?))
        } else if input.peek(LitStr) {
            let hex = input.parse::<HexString>()?;
            for elem in hex.elems() {
                if let HexValue::DotDot { .. } = elem {
                    return Err(syn::Error::new(
                        elem.span(),
                        "ranges are not allowed in byte patterns.\n\
                        help: try using `_` to specify the exact number of bytes to match.",
                    ));
                }
            }
            Ok(Self::HexString(hex))
        } else {
            let attrs = Attribute::parse_inner(input)?;

            // better error
            let (content, bracket) = (|| {
                let content;
                let bracket = bracketed!(content in input);
                Ok((content, bracket))
            })()
            .map_err(|_| {
                input.error("expected a byte array pattern, a byte string, or a hex string")
            })?;

            let elems = Punctuated::parse_terminated(&content)?;
            for elem in &elems {
                if let Expr::Range(_) = elem {
                    return Err(syn::Error::new(
                        elem.span(),
                        "ranges are not allowed in byte patterns.\n\
                        help: try using `_` to specify the exact number of bytes to match.",
                    ));
                }
            }

            Ok(Self::Array {
                attrs,
                bracket,
                elems,
            })
        }
    }
}

impl ToTokens for BytePattern {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Array {
                attrs,
                bracket,
                elems,
            } => {
                for attr in attrs {
                    attr.to_tokens(tokens);
                }
                bracket.surround(tokens, |tokens| elems.to_tokens(tokens));
            }
            Self::HexString(hex) => hex.to_tokens(tokens),
            Self::LitByteStr(bstr) => {
                let values = bstr.value();
                quote!([#(#values),*]).to_tokens(tokens);
            }
        }
    }
}
