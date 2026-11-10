# `coqui-builder-derive` — API header

Header-file-style reference for the `coqui-builder-derive` crate: every public item with its
signature, contract and gotchas, so the crate can be used without reading its source. It
ships inside the package and is self-contained. Development documentation lives with the
source at <https://github.com/Boriken-Dev/coqui-builder>.

A proc-macro crate with no features; its dependencies are `syn` 2 (default features),
`quote` 1 and `proc-macro2` 1. MSRV 1.89. It holds no `unsafe`. It exports two macros and is
reached through `coqui-builder`'s `derive` feature, which re-exports it beside the trait of
the same name.

## `#[derive(Describe)]`

```rust
#[proc_macro_derive(Describe, attributes(map, schema, builder, leaf))]
#[proc_macro_derive(Leaf, attributes(leaf, map, schema))]
```

Writes, inside an anonymous `const` so nothing leaks into the caller's namespace:
`Describe` (with the hidden `Fill` the `Partial` holds), `Inspect`, and for a struct of named
fields `Fields`; for an enum of unit variants or an untagged enum, `Leaf` (a type that is one
text). The expansion
names `::coqui_builder` (or the path of `#[builder(crate = path)]`), `::core` and the
hidden `coqui_builder::__private`.

| Declaration | Setting |
| --- | --- |
| struct with named fields | a group, one key per field; may be generic over types |
| newtype struct, or `#[map(transparent)]` struct | the field's own setting |
| enum of unit variants | a `Leaf` with choices; `#[map(other)] Other(String)` keeps an unknown name |
| enum with `#[map(tag = "k")]` | arms: unit, named-field or `Name(T)` (the keys of `T`, a struct with named fields, sit beside the tag) |
| enum with `#[map(untagged)]` | a `Leaf` that reads its unit names, then each held leaf's parser in order |

Refused with a sentence: a tuple struct of two or more fields, a unit struct, a union, a
generic enum, a lifetime or const parameter, an untagged variant with named fields, a
variant that carries fields in an enum without a tag, and a field whose type names the type
it is in.

### `#[derive(Leaf)]`

Makes a type one setting, as one text. A unit-only enum reads and writes its variant names
(`#[map(rename)]`, `#[map(rename_all)]`); any other type uses its `Display` and `FromStr`
(`from_text` refuses without quoting its input). `#[leaf(name = "..")]` names the type a datum
shows (default: the type's name), `#[leaf(ordered)]` takes the order from `PartialOrd`,
`#[leaf(display)]` makes a unit-only enum use `Display` and `FromStr`. Equality is the
canonical text, as `Leaf::same` states.

### Attributes

`#[map(..)]` on a type: `rename_all = "lowercase" | "snake_case" | "kebab-case" |
"SCREAMING_SNAKE_CASE"`, `transparent`, `deny_unknown_fields` (struct); `tag = ".."`,
`untagged`, `rename_all` (enum); `crate = path` (accepted and ignored: guatiao's spelling).

`#[map(..)]` on a field: `rename = ".."`, `skip`, `default`, `default = <expr>`, `flatten`,
`with = path`, `as_text`, `none = ".."`. On a variant: `rename = ".."`, `other`.

`#[schema(..)]` on a field: `title`, `description`, `section`, `order`, `advanced`,
`sensitive`, `default = <expr>`, `min_length`, `max_length`, `pattern`, `format`,
`multiple_of`, `min_items`, `max_items`, `read_only`, `deprecated`, `examples(..)`. A doc
comment is the description unless one is given. On a tagged variant: `title`, `description`;
on a unit variant: `label`.

`#[builder(..)]` on a field: `stricter = "higher" | "lower"`, `setter = "name"`. On a type:
`crate = path`.

Contracts:

- A key is required unless the field has a default or is an `Option`.
- `default` on an `Option` is refused unless `none = ".."` is beside it with `default = <expr>`;
  `none` is refused on a field that is not an `Option`, beside `with`, `as_text` or `flatten`,
  and without a default expression. The inner type's parser must refuse the `none` text, or the text would be ambiguous.
- `flatten` stands alone (no rename, default or conversion), takes a type with named fields,
  and is refused beside `deny_unknown_fields`.
- `as_text` applies per item to `Vec<T>` and `Option<Vec<T>>` (`T: Display + FromStr`).
- `with = path` names a module with `const TYPE: &str`, `to_text(&T) -> String` and
  `from_text(&str) -> Result<T, LeafError>`; the field is a leaf of that type. It cannot use a generic parameter of the type it sits in, and is
  refused on an `Option`.
- A setting whose setter name (the field, or `setter = ".."`) is `build`, `fill`,
  `fill_from`, `builder`, `new`, `configured`, `non_defaults` or `partial_mut` is refused.
- `skip_serializing_if` and `alias` are refused: nothing is written, and a description has
  no alias.

## Gotchas

- The derive sees the item after `cfg` and `cfg_attr` are applied; a field behind a false
  `cfg` is no key.
- Every type parameter of a generic struct is bound by `Describe + Inspect`.
- Indirect recursion (A holds B holds A) is not detected.
