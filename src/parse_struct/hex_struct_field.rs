use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};

use syn::{
    spanned::Spanned,
    token::{Colon, Underscore},
    Attribute, Expr, Ident, Member, Result, Token,
};

use super::byte_pattern::BytePattern;
use super::{BUFFER_UUID, READER_UUID, VALUE_UUID};

#[derive(Debug)]
pub enum HexStructField {
    Field {
        attrs: Vec<Attribute>,
        member: Member,
        colon: Colon,
        buffer_ident: Option<Ident>,
        byte_pattern: BytePattern,
        expr: Option<Expr>,
    },
    Match {
        underscore: Underscore,
        colon: Colon,
        byte_pattern: BytePattern,
    },
}

impl HexStructField {
    fn byte_pattern(&self) -> &BytePattern {
        match self {
            Self::Field { byte_pattern, .. } | Self::Match { byte_pattern, .. } => byte_pattern,
        }
    }
    fn reader_ident(&self) -> Ident {
        Ident::new(READER_UUID, self.byte_pattern().span())
    }
    fn value_ident(&self) -> Ident {
        Ident::new(VALUE_UUID, self.byte_pattern().span())
    }
    fn buffer_ident(&self) -> Ident {
        match self {
            Self::Field {
                buffer_ident: Some(ident),
                ..
            } => ident.to_owned(),
            _ => Ident::new(BUFFER_UUID, self.byte_pattern().span()),
        }
    }
}

impl Spanned for HexStructField {
    fn span(&self) -> Span {
        match self {
            Self::Field {
                member,
                expr: Some(expr),
                ..
            } => member
                .span()
                .join(expr.span())
                .unwrap_or_else(|| member.span()),
            Self::Field {
                member,
                byte_pattern,
                ..
            } => member
                .span()
                .join(byte_pattern.span())
                .unwrap_or_else(|| member.span()),
            Self::Match {
                underscore,
                byte_pattern,
                ..
            } => underscore
                .span()
                .join(byte_pattern.span())
                .unwrap_or_else(|| underscore.span()),
        }
    }
}

impl HexStructField {
    pub fn to_tokens(&self, next: Option<&&HexStructField>) -> TokenStream {
        match self {
            Self::Match { byte_pattern, .. } => {
                let reader = self.reader_ident();
                let len = byte_pattern.len();
                let byte_pattern_string = format!("{}", byte_pattern);

                quote!(
                    {
                        let mut buf: [u8; #len] = [0; #len];
                        #reader.read(&mut buf)?;

                        #[allow(dead_code)]
                        match buf {
                            #byte_pattern => (),
                            _ => return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!("expected {}, got {:02X?}", #byte_pattern_string, buf),
                                ))
                        }
                    }
                )
            }
            Self::Field {
                attrs,
                member,
                colon,
                byte_pattern,
                expr,
                ..
            } => {
                let reader_ident = self.reader_ident();
                let buffer_ident = self.buffer_ident();

                let match_insert = match next {
                    Some(next @ HexStructField::Match { .. }) => next.to_tokens(None),
                    _ => quote!(),
                };
                let len = byte_pattern.len();
                let byte_pattern_string = format!("{}", byte_pattern);

                let value_ident = self.value_ident();
                let value = match expr {
                    Some(expr) => quote!(#expr),
                    None => quote!(#buffer_ident),
                };

                quote!(
                    #(#attrs)*
                    #member#colon {
                        #[allow(non_snake_case)]
                        let mut #buffer_ident: [u8; #len] = [0; #len];
                        #reader_ident.read(&mut #buffer_ident)?;
                        #[allow(dead_code)]
                        match #buffer_ident {
                            #byte_pattern => (),
                            _ => return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!("expected {}, got {:02X?}", #byte_pattern_string, #buffer_ident),
                                ))
                        }

                        #[allow(non_snake_case)]
                        let #value_ident = #value;
                        #match_insert
                        #value_ident
                    },
                )
            }
        }
    }
}

impl Parse for HexStructField {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(if input.peek(Token![_]) {
            HexStructField::Match {
                underscore: input.parse()?,
                colon: input.parse()?,
                byte_pattern: {
                    if input.peek(Ident) {
                        return Err(
                            input.error("binding of `_` match-only byte patterns is not allowed")
                        );
                    }
                    let pattern = input.parse()?;
                    if input.peek(Token![=>]) {
                        return Err(
                            input.error("binding of `_` match-only byte patterns is not allowed")
                        );
                    }
                    pattern
                },
            }
        } else {
            let attrs = Attribute::parse_inner(input)?;
            let member = input.parse()?;
            let colon = input.parse()?;
            let buffer_ident = if input.peek(Ident) {
                let ident = input.parse()?;
                input.parse::<Token![@]>()?;
                Some(ident)
            } else {
                None
            };

            let byte_pattern = input.parse()?;

            let expr = if buffer_ident.is_some() {
                input.parse::<Token![=>]>()?;
                Some(input.parse()?)
            } else {
                None
            };

            HexStructField::Field {
                attrs,
                member,
                colon,
                buffer_ident,
                byte_pattern,
                expr,
            }
        })
    }
}
