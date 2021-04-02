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
//! [`parse_struct!`](parse_struct!) is a macro for parsing bytes from [`Read`](std::io::Read) readers
//! into structs (or enums), with the ability to skip padding bytes.
//! It returns a `Result<Struct, std::io::Error>` value.
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
//! fn main() -> Result<()> {
//!     let bytes = [
//!         0x48, 0x45, 0x58, 0x00, 0x01, 0x02, 0x00, 0xAA, 0xBB, 0xCC, 0xDD,
//!     ];
//!     let data = parse_struct!(bytes.as_ref() => Data {
//!         _: b"HEX",
//!         _: [0],
//!         a: [0x01, _],
//!         _: "00",
//!         b: buf @ "AABB ____" => u32::from_le_bytes(*buf),
//!     })?;
//!     println!("{:X?}", data); // Data { a: [1, 2], b: DDCCBBAA }
//!     Ok(())
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

/// Macro for parsing bytes from [`Read`](std::io::Read) readers into structs
/// with the ability to skip padding bytes.
///
/// # Syntax
///
/// ```
/// parse_struct!(READER => STRUCT {
///     ...
///     FIELD: [BINDING @] BYTE_PATTERN [=> EXPRESSION],
///     ...
/// })
/// ```
///
/// First, the macro expects a reader or an expression the result of which would be a reader.
/// The reader is followed by `=>` and then by a modified form of struct instantiation.
///
/// The basic syntax of struct instantiation takes the form of `FIELD: BYTE_PATTERN`. This will assign
/// the read bytes (`[u8; N]`) to the given field if it matches the pattern.
/// For more advanced scenarios, such as for converting the bytes to other types,
/// bindings and expressions can be used:
/// `FIELD: BINDING @ BYTE_PATTERN => EXPRESSION`.
/// In this case, the result of `EXPRESSION` will be assigned to `FIELD`.
///
/// A special `_` field is available for matching against bytes without including them in the
/// struct. `_` fields can be specified multiple times and
/// can be used for skipping padding bytes or for matching against bytes without including them in
/// the struct. Bindings and expressions can be used with these fields as well but expressions must
/// evaluate to ().
///
/// Patterns can be any of:
/// - `[1, 2, 3, _, 5]` - standard byte array patterns
/// - `b"byte string!"` - byte strings
/// - `"FF00FF 00FF00"` - hex strings usable with the [`hex!`](hex!) macro
///
/// Patterns can include `_` but not `..` wildcards since the length of the pattern is
/// used to determine the amount of bytes to read.
///
/// Structs or enum variants with unnamed members (`Item(A, B)`) can be used with the
/// `Struct { 0: ..., 1: ... }` syntax.
///
/// This macro returns `Result` containing either the resulting struct
/// or [`std::io::Error`](std::io::Error) if an error occurred while reading or matching the bytes.
/// [`std::io::ErrorKind::InvalidData`](std::io::ErrorKind::InvalidData)
/// if the bytes were not matched successfully.
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
/// fn main() -> Result<()> {
///     let bytes = [
///         0x48, 0x45, 0x58, 0x00, 0x01, 0x02, 0x00, 0xAA, 0xBB, 0xCC, 0xDD,
///     ];
///     let data = parse_struct!(bytes.as_ref() => Data {
///         _: b"HEX",
///         _: [0],
///         a: [0x01, _],
///         _: "00",
///         b: buf @ "AABB ____" => u32::from_le_bytes(*buf),
///     })?;
///     println!("{:X?}", data); // Data { a: [1, 2], b: DDCCBBAA }
///     Ok(())
/// }
/// ```
///
/// # Details
///
/// This macro would be parsed into a closure which is instantly called so that any
/// potential errors caused by `Read` can be handled explicitly by the user.
///
/// The macro in the example above would be parsed into the following code
/// (internal variable names prefixed with `_` changed for clarity):
///
/// ```
/// (|| {
///     use std::convert::TryInto;
///     #[allow(non_snake_case)]
///     let mut _READER = bytes.as_ref();
///     #[allow(non_snake_case)]
///     let mut _ARRAY: [u8; 4usize] = [0; 4usize]; // length of the longest pattern
///     let _: () = {
///         _READER.read_exact(&mut _ARRAY[0..3usize])?;
///         #[allow(non_snake_case)]
///         let _BUFFER: &[u8; 3usize] = _ARRAY[0..3usize].try_into().unwrap();
///         #[allow(dead_code)]
///         match _BUFFER {
///             [72u8, 69u8, 88u8] => (), // b"HEX"
///             _ => {
///                 return Err(std::io::Error::new(
///                     std::io::ErrorKind::InvalidData,
///                     format!("expected `{}`, got `{:02X?}`", "b\"HEX\"", _BUFFER),
///                 ))
///             }
///         }
///     };
///     let _: () = {
///         _READER.read_exact(&mut _ARRAY[0..1usize])?;
///         #[allow(non_snake_case)]
///         let _BUFFER: &[u8; 1usize] = _ARRAY[0..1usize].try_into().unwrap();
///         #[allow(dead_code)]
///         match _BUFFER {
///             [0] => (),
///             _ => {
///                 return Err(std::io::Error::new(
///                     std::io::ErrorKind::InvalidData,
///                     format!("expected `{}`, got `{:02X?}`", "[0]", _BUFFER),
///                 ))
///             }
///         }
///     };
///     let _a = {
///         _READER.read_exact(&mut _ARRAY[0..2usize])?;
///         #[allow(non_snake_case)]
///         let _BUFFER: &[u8; 2usize] = _ARRAY[0..2usize].try_into().unwrap();
///         #[allow(dead_code)]
///         match _BUFFER {
///             [0x01, _] => (),
///             _ => {
///                 return Err(std::io::Error::new(
///                     std::io::ErrorKind::InvalidData,
///                     format!("expected `{}`, got `{:02X?}`", "[0x01, _]", _BUFFER),
///                 ))
///             }
///         }
///         *_BUFFER // [u8; 2] as the result if no expression given
///     };
///     let _: () = {
///         _READER.read_exact(&mut _ARRAY[0..1usize])?;
///         #[allow(non_snake_case)]
///         let _BUFFER: &[u8; 1usize] = _ARRAY[0..1usize].try_into().unwrap();
///         #[allow(dead_code)]
///         match _BUFFER {
///             [0u8] => (),
///             _ => {
///                 return Err(std::io::Error::new(
///                     std::io::ErrorKind::InvalidData,
///                     format!("expected `{}`, got `{:02X?}`", "[0x00]", _BUFFER),
///                 ))
///             }
///         }
///     };
///     let _b = {
///         _READER.read_exact(&mut _ARRAY[0..4usize])?;
///         #[allow(non_snake_case)]
///         let buf: &[u8; 4usize] = _ARRAY[0..4usize].try_into().unwrap(); // assign binding
///         #[allow(dead_code)]
///         match buf {
///             [170u8, 187u8, _, _] => (), // "AABB ____"
///             _ => {
///                 return Err(std::io::Error::new(
///                     std::io::ErrorKind::InvalidData,
///                     format!("expected `{}`, got `{:02X?}`", "[0xAA, 0xBB, _, _]", buf),
///                 ))
///             }
///         }
///         u32::from_le_bytes(*buf) // provided expression
///     };
///     Ok(Data { a: _a, b: _b }) // `_` fields are not included in the resulting struct
/// })()
/// ```
#[proc_macro]
pub fn parse_struct(stream: TokenStream) -> TokenStream {
    let input = parse_macro_input!(stream as HexStruct);
    TokenStream::from(quote!(#input))
}
