# Heap Object Representation

The lowest granularity for objects in Candy's memory (stack and heap) is one _word_: 64 bit = 8 byte.
An object is stored as an _inline object_:

## Inline Object

An inline object is a single word containing a tagged union of different types of values:

|                                                                     Value | Meaning           |
| ------------------------------------------------------------------------: | :---------------- |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx000` | Pointer           |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx001` | Int               |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx010` | Builtin           |
| `xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx xxxxx011` | Tag without value |
| `bbbbbbbb bbbbbbbb bbbbbbbb bbbbbbbb aaaaaaaa aaaaaaaa aaaaaaaa aaaaa100` | Handle            |

> The remaining patterns are invalid.

### Pointer

Values that don't fit inside an inline word are stored in the heap.
The whole word is used as a pointer directly (i.e., the three trailing zeros are part of the pointer).
Each pointer points to the _header word_ of a heap object.

### Int

`x` stores the signed integer value.
For larger values, a pointer to a heap object containing an integer of (practically) unlimited size is used.

### Builtin

`x` stores the builtin function index as an unsigned integer.

### Tag without Value

When setting the lowest three bits to zero, the word is a pointer to a heap text representing the symbol.

### Handle

`a` stores the argument count.
`b` stores the handle ID as an unsigned integer.

## Heap Object

Each heap object has the following structure:

- one header word
- iff `r == 1`: one word containing the reference count as an unsigned integer (`u64`)
- zero to many words containing the actual data
  - for now, there are some objects whose content data length isn't a multiple of the word size since they use Rust's representation for simplicity

The header word is a tagged union of different types of values:

|                                                                     Value | Meaning  |
| ------------------------------------------------------------------------: | :------- |
| `00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000r000` | Int      |
| `00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000r001` | Tag      |
| `aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaar010` | Text     |
| `cccccccc cccccccc cccccccc cccccccc aaaaaaaa aaaaaaaa aaaaaaaa aaaar011` | Function |
| `aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaar100` | List     |
| `aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaar101` | Struct   |
| `00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000r110` | HirId    |

> The remaining patterns are invalid.

`r` is set to one iff the object is a reference-counted object.
(Constants are not reference-counted so that they can be shared across fibers without locks.)

### Int

Uses Rust's `BigInt` representation after the header word and reference count.
Values that fit into an inline word _must_ be stored inline.

### Tag

| Word                          |
| :---------------------------- |
| Header Word (tag)             |
| Reference count               |
| Pointer to symbol (heap text) |
| InlineWord with value         |

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

### Function

A function capturing `c` values, taking `a` arguments, and with a body starting at instruction pointer `b`.

| Word                   |
| :--------------------- |
| Header Word (function) |
| Reference count        |
| `b`                    |
| Captured value 0       |
| …                      |
| Captured value c-1     |

> Instructions are stored in Rust's representation.
> They may take up multiple words and might not align to word boundaries.

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

### HirId

Rust's representation is used and stored in the subsequent 11 words.
