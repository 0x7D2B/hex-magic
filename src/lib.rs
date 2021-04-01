//! This crate provides macros for working with bytes and hexadecimal values.
//!
//! # `hex!`
//!
//! [`hex!`](hex!) is a macro which converts string literals (`"7D2B"`) to byte arrays (`[0x7D, 0x2B]`) or match patterns at compile time.
//!
//! ```
//! assert_eq!(hex!("01020304"), [1, 2, 3, 4]);
//! ```
//! # `parse_struct!`
//!
//! [`parse_struct!`](parse_struct!) is a macro for parsing bytes from [`Read`](std::io::Read) readers into structs,
//! with the ability to skip padding bytes. It returns a `Result<STRUCT, std::io::Error>` value.
//!
//! ```
//! use hex_magic::parse_struct;
//! use std::io::{Read, Result};
//!
//! #[derive(Debug)]
//! struct Data {
//!     a: [u8; 2],
//!     b: u32,
//! }
//!
//! fn main() -> Result<Data> {
//!     let bytes = [0x48, 0x45, 0x58, 0x01, 0x02, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];
//!     let data = parse_struct!( bytes.as_ref() => Data {
//!         _: b"HEX",
//!         a: [0x01, _],
//!         _: "00",
//!         b: buf @ "AABB ____" => u32::from_le_bytes(buf)
//!     });
//!     println!("{:X?}", data); // Ok(Data { a: [1, 2], b: DDCCBBAA });
//!     data
//! }
//! ```
use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod hex_string;
mod parse_struct;
use hex_string::HexString;
use parse_struct::HexStruct;

/// Macro which converts string literals (`"7D2B"`) to byte arrays (`[0x7D, 0x2B]`) at compile time.
///
/// It's a rewrite of the `hex!` macro provided by the [`hex-literal`](https://docs.rs/hex-literal/) crate
/// with stricter rules requiring bytes to come in pairs (so `"12 34"` is allowed but `"1 2 3 4"` is
/// not) and with the addition of being able to parse `__` and `..` to create match patterns.
///
/// It accepts the following characters in the input string:
///
/// - `'0'...'9'`, `'a'...'f'`, `'A'...'F'` -- hex characters which will be used
///     in construction of the output byte array
/// - `' '`, `'\r'`, `'\n'`, `'\t'` -- formatting characters which will be
///     ignored
/// - `'_'`, `'.'` -- formatting characters which will be used to create match patterns
///
/// # Example
///
/// ```
/// use hex_magic::hex;
///
/// const BYTES: [u8; 3] = hex!("DEAD AF");
///
/// fn main() {
///     assert_eq!(BYTES, [0xDE, 0xAD, 0xAF]);
///     assert_eq!(hex!("aA aa aA Aa aa"), [0xAA; 5]);
///
///     match [1, 2, 3, 4] {
///         hex!("AABBCCDD") => panic!("bytes don't match at all"),
///         hex!("01__FF__") => panic!("[1, _, 0xFF, _] does not match"),
///         hex!("01..04") => println!("[1, .., 4] would match"),
///         hex!("..") => unreachable!("[..] would match"),
///     }
/// }
/// ```
#[proc_macro]
pub fn hex(stream: TokenStream) -> TokenStream {
    let input = parse_macro_input!(stream as HexString);
    TokenStream::from(quote!(#input))
}

/// Macro for parsing bytes from [`Read`](std::io::Read) readers into structs,
/// with the ability to skip padding bytes.
///
/// # Syntax
///
/// ```
/// parse_struct!(READER => STRUCT {
///     _: PATTERN,
///     byte_array_field: PATTERN,
///     field: BINDING @ PATTERN => EXPRESSION
/// })
/// ```
///
/// First, the macro expects a reader or an expression the result of which would be a reader.
/// The reader is followed by `=>` and then by a modified form of struct instantiation.
///
/// The basic syntax of struct instantiation takes the form of `FIELD: PATTERN`. This will assign
/// the read byte array to the given field if it matches the pattern. For more complicated
/// scenarios when the bytes need to be parsed first, bindings can be used: `FIELD: BINDING @ PATTERN => EXPRESSION`.
/// In this case, the result of `EXPRESSION` will be assigned to the `FIELD`.
///
/// There is also the ability to have match-only fields with the `_: PATTERN` syntax. This is
/// useful for skipping padding bytes or for matching against bytes that don't need to be saved in
/// the struct. These fields are match-only and can't be used for bindings.
///
/// Patterns can be any of:
/// - `[1, 2, 3, _, 5]` - standard byte array patterns
/// - `b"byte string!"` - byte strings
/// - `"FF00FF 00FF00"` - hex strings usable with the [`hex!`](hex!) macro
///
/// Patterns can include `_` wildcards but not `..` wildcards since the length of the pattern is
/// used to determine the size of the byte array to be read into.
///
/// This macro returns a `Result`: `Ok(STRUCT)` or [`Err(std::io::Error)`](std::io::Error).
/// Reader errors are returned as is, while errors caused by unsuccessful byte pattern matching
/// will use [`std::io::ErrorKind::InvalidData`](std::io::ErrorKind::InvalidData).
///
/// # Example
///
/// ```
/// use hex_magic::parse_struct;
/// use std::io::{Read, Result};
///
/// #[derive(Debug)]
/// struct Data {
///     a: [u8; 2],
///     b: u32,
/// }
///
/// fn main() -> Result<Data> {
///     let bytes = [0x48, 0x45, 0x58, 0x01, 0x02, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];
///     let data = parse_struct!( bytes.as_ref() => Data {
///         _: b"HEX",
///         a: [0x01, _],
///         _: "00",
///         b: buf @ "AABB ____" => u32::from_le_bytes(buf)
///     });
///     println!("{:X?}", data); // Ok(Data { a: [1, 2], b: DDCCBBAA });
///     data
/// }
/// ```
///
/// # Details
///
/// The macro invocation above would be parsed into a closure which is instantly called. This
/// closure would read from the reader and return either `Ok(STRUCT)` or `Err(std::io::Error)`.
///
/// The example above is parsed into the following code (internal variable names changed for clarity):
///
/// ```
/// (|| {
///     #[allow(non_snake_case)]
///     let mut _READER = reader; // reader variable
///
///     // handle the first `_` field
///     {
///         let mut _BUFFER: [u8; 3usize] = [0; 3usize];
///         _READER.read(&mut _BUFFER)?;
///         #[allow(dead_code)]
///         match _BUFFER {
///             [72u8, 69u8, 88u8] => (), // b"HEX" parsed into a pattern
///             _ => {
///                 return Err(std::io::Error::new(
///                     std::io::ErrorKind::InvalidData,
///                     format!("expected {}, got {:02X?}", "b\"HEX\"", _BUFFER),
///                 ))
///             }
///         }
///     }
///     Ok(Data {
///         a: {
///             // `a` has no binding so a generic name is used for the byte array
///             #[allow(non_snake_case)]
///             let mut _BUFFER: [u8; 2usize] = [0; 2usize];
///             _READER.read(&mut _BUFFER)?;
///             #[allow(dead_code)]
///             match _BUFFER {
///                 [0x01, _] => (),
///                 _ => {
///                     return Err(std::io::Error::new(
///                         std::io::ErrorKind::InvalidData,
///                         format!("expected {}, got {:02X?}", "[0x01, _]", _BUFFER),
///                     ))
///                 }
///             }
///
///             #[allow(non_snake_case)]
///             let _VALUE = _BUFFER; // no binding for `a` so the array will be used as is
///
///             // match second `_` field (after `a`)
///             {
///                 let mut _BUFFER: [u8; 1usize] = [0; 1usize];
///                 _READER.read(&mut _BUFFER)?;
///                 #[allow(dead_code)]
///                 match _BUFFER {
///                     [0u8] => (),
///                     _ => {
///                         return Err(std::io::Error::new(
///                             std::io::ErrorKind::InvalidData,
///                             format!("expected {}, got {:02X?}", "[00]", _BUFFER),
///                         ))
///                     }
///                 }
///             }
///
///             _VALUE // use the byte array
///         },
///         b: {
///             #[allow(non_snake_case)]
///             let mut buf: [u8; 4usize] = [0; 4usize]; // `b` has a binding so it's used for the byte array
///             _READER.read(&mut buf)?;
///             #[allow(dead_code)]
///             match buf {
///                 [170u8, 187u8, _, _] => (),
///                 _ => {
///                     return Err(std::io::Error::new(
///                         std::io::ErrorKind::InvalidData,
///                         format!("expected {}, got {:02X?}", "[AA, BB, __, __]", buf),
///                     ))
///                 }
///             }
///
///             #[allow(non_snake_case)]
///             let _VALUE = u32::from_le_bytes(buf); // use provided expression to convert the binding to `u32`
///             _VALUE // no `_` after `b` so the result is immediately returned
///         },
///     })
/// })();
/// ```
#[proc_macro]
pub fn parse_struct(stream: TokenStream) -> TokenStream {
    let input = parse_macro_input!(stream as HexStruct);
    TokenStream::from(quote!(#input))
}
