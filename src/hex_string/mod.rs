use proc_macro2::{Literal, Span, TokenStream};
use std::fmt;

use syn::parse::{Parse, ParseStream};

use quote::{quote, quote_spanned, ToTokens};
use syn::{LitStr, Result};

#[derive(Debug)]
pub enum HexValue {
    Number { value: u8, span: Span },
    Underscore { span: Span },
    DotDot { span: Span },
}

impl fmt::Display for HexValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Number { value, .. } => write!(f, "{:02X}", value),
            Self::Underscore { .. } => write!(f, "__"),
            Self::DotDot { .. } => write!(f, ".."),
        }
    }
}

impl ToTokens for HexValue {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Number { value, span } => {
                let mut lit = Literal::u8_suffixed(*value);
                lit.set_span(*span);
                lit.to_tokens(tokens);
            }
            Self::Underscore { span } => quote_spanned!(*span=>_).to_tokens(tokens),
            Self::DotDot { span } => quote_spanned!(*span=>..).to_tokens(tokens),
        }
    }
}

#[derive(Debug)]
pub struct HexString {
    elems: Vec<HexValue>,
}

impl HexString {
    pub fn len(&self) -> usize {
        self.elems.len()
    }
    pub fn elems(&self) -> &Vec<HexValue> {
        &self.elems
    }
}

impl fmt::Display for HexString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}]",
            self.elems
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

impl Parse for HexString {
    fn parse(input: ParseStream) -> Result<Self> {
        let litstr = input.parse::<LitStr>()?;
        let span = litstr.span();
        let chars: Vec<u8> = litstr.value().into();
        let mut elems: Vec<HexValue> = vec![];

        let mut msb: u8 = 0;
        let mut need_hex = false;
        let mut need_underscore = false;
        let mut need_dot = false;

        for c in chars {
            match c {
                // insert ..
                b'.' if need_dot => {
                    need_dot = false;
                    elems.push(HexValue::DotDot { span });
                }
                b'.' => need_dot = true,
                _ if need_dot => {
                    return Err(syn::Error::new(
                        span,
                        format!("expected a second `.`, got `{}`", c as char),
                    ))
                }

                // insert _
                b'_' if need_underscore => {
                    need_underscore = false;
                    elems.push(HexValue::Underscore { span });
                }
                b'_' => need_underscore = true,
                _ if need_underscore => {
                    return Err(syn::Error::new(
                        span,
                        format!("expected a matching `_`, got `{}`", c as char),
                    ))
                }

                // insert hex byte
                b'0'..=b'9' if need_hex => {
                    need_hex = false;
                    elems.push(HexValue::Number {
                        value: (msb << 4) | (c - b'0'),
                        span,
                    });
                }
                b'0'..=b'9' => {
                    need_hex = true;
                    msb = c - b'0';
                }

                b'a'..=b'f' if need_hex => {
                    need_hex = false;
                    elems.push(HexValue::Number {
                        value: (msb << 4) | (c - b'a' + 10),
                        span,
                    });
                }
                b'a'..=b'f' => {
                    need_hex = true;
                    msb = c - b'a' + 10;
                }

                b'A'..=b'F' if need_hex => {
                    need_hex = false;
                    elems.push(HexValue::Number {
                        value: (msb << 4) | (c - b'A' + 10),
                        span,
                    });
                }
                b'A'..=b'F' => {
                    need_hex = true;
                    msb = c - b'A' + 10;
                }
                _ if need_hex => {
                    return Err(syn::Error::new(
                        span,
                        format!("expected a matching hex digit, got `{}`", c as char),
                    ))
                }

                // clear whitespace
                b' ' | b'\r' | b'\n' | b'\t' => continue,

                // fail on anything else
                _ => {
                    return Err(syn::Error::new(
                        span,
                        format!("invalid character: `{}`", c as char),
                    ))
                }
            }
        }
        Ok(Self { elems })
    }
}

impl ToTokens for HexString {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let elems = &self.elems;
        quote!([#(#elems),*]).to_tokens(tokens)
    }
}
