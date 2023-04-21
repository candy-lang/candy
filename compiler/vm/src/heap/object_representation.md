# Heap Object Representation

The lowest granularity for values in Candy's memory (stack and heap) is one _word_: 64 bit = 8 byte.
These words are usually \_Inline Word_s.
An inline word is a tagged union of different types of values:

## Inline Word

|                                                                     Value | Meaning     |
| ------------------------------------------------------------------------: | :---------- |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx000` | Pointer     |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxx01` | Int         |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx010` | SendPort    |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx110` | ReceivePort |
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
| Reference count    |
| Item 0             |
| …                  |
| Item a-1           |

### Struct

`a` stores the number of struct fields.

| Word                 |
| :------------------- |
| Header Word (struct) |
| Reference count      |
| Hash of key 0        |
| …                    |
| Hash of key a-1      |
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
| Reference count    |
| First 8 bytes      |
| …                  |
| Last 1 to 8 bytes  |

> For now, we don't pad the last word but reuse Rust's `str` for storing text in this representation.

### Closure

A closure capturing `c` values, taking `a` arguments, and containing `b` instructions.

| Word                  |
| :-------------------- |
| Header Word (closure) |
| Reference count       |
| `b`                   |
| Captured value 0      |
| …                     |
| Captured value c-1    |
| Instruction 0         |
| …                     |
| Instruction b-1       |

> Instructions are stored in Rust's representation.
> They may take up multiple words and might not align to word boundaries.

### HirId

Rust's representation is used and stored in the subsequent 11 words.
