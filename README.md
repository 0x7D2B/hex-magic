This crate provides macros for working with bytes and hexadecimal values.

# `hex!`

`hex!` is a macro which converts string literals (`"7D2B"`) to byte arrays (`[0x7D, 0x2B]`) or match patterns at compile time.

```
assert_eq!(hex!("01020304"), [1, 2, 3, 4]);
```
# `parse_struct!`

`parse_struct!` is a macro for parsing bytes from `Read` readers into structs,
with the ability to skip padding bytes. It returns a `Result<STRUCT, std::io::Error>` value.

```
use hex_magic::parse_struct;
use std::io::{Read, Result};

#[derive(Debug)]
struct Data {
    a: [u8; 2],
    b: u32,
}

fn main() -> Result<Data> {
    let bytes = [0x48, 0x45, 0x58, 0x01, 0x02, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];
    let data = parse_struct!( bytes.as_ref() => Data {
        _: b"HEX",
        a: [0x01, _],
        _: "00",
        b: buf @ "AABB ____" => u32::from_le_bytes(buf)
    });
    println!("{:X?}", data); // Ok(Data { a: [1, 2], b: DDCCBBAA });
    data
}
```
