# coqui-builder

A **crate that describes the settings a builder takes as data, with no value model and no
dependencies**. A settings type derives `Describe` once; from that one description it is
filled from any source, a missing key is named, a value is compared with its defaults, and a
profile says whether a value is what is expected, with every secret hidden. The
[Rust API reference](https://boriken-dev.github.io/coqui-builder/rust/index.html) lists every item.

## Installation

```bash
cargo add coqui-builder --features derive
```

| Feature | Adds | Needed for |
| --- | --- | --- |
| `std` (default) | `PathBuf`, `HashMap` and the `Env` source | the environment as a source |
| `derive` | `coqui-builder-derive` (`syn`, `quote`) | `#[derive(Describe)]` |

## 30-second tour

Describe a type once:

```rust
use coqui_builder::Describe;

#[derive(Describe, Debug, PartialEq)]
struct Limits {
    /// How many sources may wait at once.
    #[map(default = 16)]
    waiting: u32,
    #[schema(sensitive)]
    #[map(default)]
    secret: String,
}
```

Fill it a key at a time, from text or a source such as the environment; the last write
of a key wins, and `build` names every key still missing:

```rust
let mut partial = Partial::<Limits>::new();
partial.set("waiting", "64")?;
let limits = partial.build()?;
```

Say what differs from the defaults, as `{setting, value, default}`, each a type and its text,
with a secret as `String(***)`:

```rust
for found in non_defaults(&limits) {
    println!("{found}"); // waiting = u32(64) (default u32(16))
}
```

Say what a value is expected to be, and read why it is not:

```rust
let profile = Profile::new().expecting("waiting", Expect::Equal("16".into()))?;
let report = profile.verify(&limits);
```

## Learn more

- [Rust API reference](https://boriken-dev.github.io/coqui-builder/rust/index.html)
- [Changelog](https://github.com/Boriken-Dev/coqui-builder/blob/main/CHANGELOG.md)
- [Source](https://github.com/Boriken-Dev/coqui-builder)
