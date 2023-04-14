# Heap Object Representation

Word Size: 64 bit

## Inline Word

|                                                                     Value | Meaning     |
| ------------------------------------------------------------------------: | :---------- |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx000` | Pointer     |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxx01` | Int         |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx010` | ReceivePort |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx110` | SendPort    |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxx11` | Builtin     |

> The remaining patterns are reserved for future use.

## Header Word

|                                                                     Value | Meaning |
| ------------------------------------------------------------------------: | :------ |
| `00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000` | Int     |
| `aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaa001` | List    |
| `aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaa101` | Struct  |
| `aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaa010` | Symbol  |
| `aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaa110` | Text    |
| `cccccccc cccccccc cccccccc cccccccc aaaaaaaa aaaaaaaa aaaaaaaa aaaaa011` | Closure |
| `00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000111` | HirId   |

> The remaining patterns are reserved for future use.

Directly after the header word, the reference count is stored as a `u64`.

### Int

Uses Rust's `BigInt` representation after the header word.

### List

`a` stores the number of list elements.

| Word               |
| :----------------- |
| Header Word (list) |
| Item 0             |
| …                  |
| Item a-1           |

### Struct

`a` stores the number of struct fields.

| Word                 |
| :------------------- |
| Header Word (struct) |
| Key 0                |
| …                    |
| Key a-1              |
| Value 0              |
| …                    |
| Value a-1            |

### Symbol

See text.

### Text

`a` stores the number of bytes in UTF-8 encoding.
The last word is padded with zeros if necessary.

| Word               |
| :----------------- |
| Header Word (text) |
| First 8 bytes      |
| …                  |
| Last 1 to 8 bytes  |

### Closure

A closure capturing `c` values, taking `a` arguments, and containing `b` instructions.

| Word                                        |
| :------------------------------------------ |
| Header Word (closure)                       |
| Captured value 0                            |
| …                                           |
| Captured value c-1                          |
| `Vec<Instruction>` in Rust's representation |

### HirId

Rust's representation is used and stored in the subsequent 11 words.
