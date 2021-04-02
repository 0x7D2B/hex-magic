use proc_macro2::{Ident, Span};
use std::fmt::Display;

mod byte_pattern;
mod hex_struct;
mod hex_struct_field;

pub use hex_struct::HexStruct;

const INTERNAL_PREFIX: &str = "__hex_magic__FC9DC740_9AE7_4B27_A3B6_FAC53B953F22";

fn internal_ident<T: Display>(ident: T, span: Span) -> Ident {
    Ident::new(format!("{}_{}", INTERNAL_PREFIX, ident).as_str(), span)
}
