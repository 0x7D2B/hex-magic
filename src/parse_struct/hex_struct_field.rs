use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream};

use syn::{
    spanned::Spanned,
    token::{Colon, Underscore},
    Attribute, Expr, Ident, Member, Result, Token,
};

use super::{byte_pattern::BytePattern, internal_ident};

#[derive(Debug)]
enum HexIdent {
    Member(Member),
    Underscore(Underscore),
}
impl HexIdent {
    pub fn internal_ident(&self) -> Option<Ident> {
        match self {
            Self::Member(member) => Some(internal_ident(quote!(#member), member.span())),
            Self::Underscore(_) => None,
        }
    }
}
impl ToTokens for HexIdent {
    fn to_tokens(&self, stream: &mut TokenStream) {
        match self {
            Self::Member(member) => member.to_tokens(stream),
            Self::Underscore(underscore) => underscore.to_tokens(stream),
        }
    }
}
impl Parse for HexIdent {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(if input.peek(Token![_]) {
            Self::Underscore(input.parse()?)
        } else {
            Self::Member(input.parse()?)
        })
    }
}

#[derive(Debug)]
pub struct HexStructField {
    attrs: Vec<Attribute>,
    member: HexIdent,
    colon: Colon,
    buffer_ident: Option<Ident>,
    byte_pattern: BytePattern,
    expr: Option<Expr>,
}

impl HexStructField {
    pub fn to_instantiation_tokens(&self, stream: &mut TokenStream) {
        if !self.is_struct_member() {
            return;
        }

        let Self {
            attrs,
            member,
            colon,
            ..
        } = self;
        let member_internal = member.internal_ident().unwrap();

        quote!(
            #(#attrs)*
            #member #colon #member_internal
        )
        .to_tokens(stream);
    }

    pub fn is_struct_member(&self) -> bool {
        matches!(self.member, HexIdent::Member(_))
    }
    pub fn byte_pattern(&self) -> &BytePattern {
        &self.byte_pattern
    }
    fn reader_ident(&self) -> Ident {
        internal_ident("READER", self.byte_pattern().span())
    }
    fn array_ident(&self) -> Ident {
        internal_ident("ARRAY", self.byte_pattern().span())
    }
    fn buffer_ident(&self) -> Ident {
        match &self.buffer_ident {
            Some(ident) => ident.to_owned(),
            None => internal_ident("BUFFER", self.byte_pattern().span()),
        }
    }
}

impl ToTokens for HexStructField {
    fn to_tokens(&self, stream: &mut TokenStream) {
        let reader_ident = self.reader_ident();
        let array_ident = self.array_ident();
        let buffer_ident = self.buffer_ident();

        let byte_pattern = self.byte_pattern();
        let len = byte_pattern.len();
        let byte_pattern_string = format!("{}", byte_pattern);

        let value = {
            use HexIdent::*;
            match (&self.member, &self.expr) {
                (Underscore(_), None) => quote!(),           // only check padding
                (Member(_), None) => quote!(*#buffer_ident), // assign bytes
                (_, Some(expr)) => quote!(#expr),            // use provided expression
            }
        };

        let member_ident = match self.member.internal_ident() {
            Some(member_internal) => quote!(#member_internal),
            None => quote!(_: ()), // assert it's empty
        };

        quote_spanned!(byte_pattern.span()=>
            let #member_ident = {
                #reader_ident.read_exact(&mut #array_ident[0..#len])?;

                #[allow(non_snake_case)]
                let #buffer_ident: &[u8; #len] = #array_ident[0..#len].try_into().unwrap();

                #[allow(dead_code)]
                match #buffer_ident {
                    #byte_pattern => (),
                    _ => return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("expected `{}`, got `{:02X?}`", #byte_pattern_string, #buffer_ident),
                        ))
                }

                #value
            };
        ).to_tokens(stream);
    }
}

impl Parse for HexStructField {
    fn parse(input: ParseStream) -> Result<Self> {
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

        let expr = if buffer_ident.is_some() || input.peek(Token![=>]) {
            input.parse::<Token![=>]>().map_err(|_| {
                input.error(
                    "expected `=>` followed by an expression\n\
                     help: remove the `@` binding to only match bytes",
                )
            })?;
            Some(input.parse()?)
        } else {
            None
        };

        Ok(HexStructField {
            attrs,
            member,
            colon,
            buffer_ident,
            byte_pattern,
            expr,
        })
    }
}
