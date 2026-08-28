# scaledint

64-bit and 32-bit fixed-point decimals with compile-time scale, for Rust.

[![Crates.io](https://img.shields.io/crates/v/scaledint.svg)](https://crates.io/crates/scaledint)
[![docs.rs](https://img.shields.io/docsrs/scaledint)](https://docs.rs/scaledint)

## Quick start

```rust
use scaledint::Decimal64;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let price: Decimal64<4> = "123.4567".parse()?;
    let qty:   Decimal64<4> = "10.0000".parse()?;

    // same-scale arithmetic
    let total = price * qty;
    println!("{}", total);  // "1234.5670" (truncation in fractional component)

    // rescale before mixing scales
    let qty2: Decimal64<2> = "10.00".parse()?;
    let total2 = price * qty2.rescale_into::<4>().unwrap();
    println!("{}", total2);  // "1234.5670"

    Ok(())
}
```

## Type family

All four types share the same API, arithmetic rules, parser grammar, rounding methods,
`ParseError`, `Scientific` wrapper and `serde` support. They differ only in storage width
and signedness:

| Type             | Storage | Max scale `S` | Max value (raw)                | `mul`/`div` intermediate      |
|------------------|---------|---------------|--------------------------------|-------------------------------|
| `Decimal64<S>`   | `i64`   | 18            | ±9 223 372 036 854 775 807     | `i64` fast path, `i128` slow  |
| `UDecimal64<S>`  | `u64`   | 18            | 18 446 744 073 709 551 615     | `u64` fast path, `u128` slow  |
| `Decimal32<S>`   | `i32`   | 9             | ±2 147 483 647                 | `i64` always (single path)    |
| `UDecimal32<S>`  | `u32`   | 9             | 4 294 967 295                  | `u64` always (single path)    |

The unsigned types reject `+`/`-` when parsing, have no `MIN`, and their `Sub` operator
returns `Option<Self>` (underflow is a domain condition, not a bug); use `saturating_sub`
to clamp to zero. Convert between signed and unsigned with `as_signed()` / `as_unsigned()`
(both `Option`).

The 32-bit types are for dense storage of small-magnitude quantities (4 bytes each) and match
Parquet's `INT32`-backed decimals (precision ≤ 9). Convert between widths at the same scale:

```rust
use scaled_int::{Decimal32, Decimal64};

let small: Decimal32<2> = "123.45".parse().unwrap();
let wide: Decimal64<2> = small.widen();            // lossless; also `Decimal64::from(small)`
assert_eq!(wide.narrow(), Some(small));            // None if outside i32 range
assert_eq!(Decimal64::<2>::MAX.narrow(), None);
```

## Scale table

`Decimal64<S>` stores the mathematical value as an `i64` whose unit is `10^(-S)`.

| S  | Unit name    | Typical use                              | Max absolute value   |
|----|--------------|------------------------------------------|----------------------|
|  2 | centis       | USD cents, stock shares (round lots)     | ~92.2 quadrillion    |
|  4 | basis points | FX rates, equity prices                  | ~922 trillion        |
|  6 | micros       | Crypto prices, nanosecond timing         | ~9.22 trillion       |
|  9 | nanos        | Gas prices (Ethereum), high-freq rates   | ~9.22 billion        |
| 18 | attos        | Smallest Ethereum unit (wei)             | ~9.22                |

The scale `S` is enforced at compile time: `Decimal64<A> + Decimal64<B>` where `A ≠ B`
is a **compile error**. Use `rescale_into` to align scales explicitly.

`S > 18` is rejected at compile time (would overflow `ONE = 10^S`).

### 32-bit scale table

`Decimal32<S>` / `UDecimal32<S>` store the value as an `i32` / `u32`; `S > 9` is rejected
at compile time.

| S  | Unit name    | `Decimal32` max (±)  | `UDecimal32` max    |
|----|--------------|----------------------|---------------------|
|  0 | integer      | 2 147 483 647        | 4 294 967 295       |
|  2 | centis       | 21 474 836.47        | 42 949 672.95       |
|  4 | basis points | 214 748.3647         | 429 496.7295        |
|  6 | micros       | 2 147.483647         | 4 294.967295        |
|  9 | nanos        | 2.147483647          | 4.294967295         |

## Arithmetic rules

### Addition and subtraction

Both operands must share the same scale `S` — enforced by the type system.

```rust
let a: Decimal64<4> = "1.2345".parse().unwrap();
let b: Decimal64<4> = "0.0001".parse().unwrap();
let c = a + b;  // Decimal64<4>("1.2346")
```

Overflow panics. Use `checked_add` / `saturating_add` for fallible or clamping variants.

### Multiplication (same-scale)

`a * b` computes `(a.raw() as i128 * b.raw() as i128) / 10^S` and returns a `Decimal64<S>`.
The intermediate `i128` prevents overflow for valid `i64` inputs. The fractional component
of the product is **truncated toward zero** (keeping scale `S`, not `2S`).

Overflow panics. Use `checked_mul` / `saturating_mul` for fallible or clamping variants.

### Division

`a / b` computes `(a.raw() * 10^S) / b.raw()` in `i128`, **truncating toward zero**.
Division by zero panics. Use `checked_div` or `div_round_nearest(rhs)` for alternatives.

### Checked and saturating variants

All operators have checked and saturating companions:

```rust
a.checked_add(b)           // → Option<Decimal64<S>>
a.checked_sub(b)           // → Option<Decimal64<S>>
a.checked_mul(b)           // → Option<Decimal64<S>>
a.checked_div(b)           // → Option<Decimal64<S>>  (None for div-by-zero)

a.saturating_add(b)        // → Decimal64<S>  (clamps to MAX/MIN)
a.saturating_sub(b)        // → Decimal64<S>
a.saturating_mul(b)        // → Decimal64<S>

a.div_round_nearest_even(b)          // panics on div-by-zero
a.checked_div_round_ceil(b) // → Option<Decimal64<S>>
```

### Rescaling

```rust
let d2: Decimal64<2> = "1.25".parse().unwrap();
let d4: Decimal64<4> = d2.rescale_into::<4>().unwrap();  // 1.2500 — exact, always succeeds
let d1: Decimal64<1> = d2.rescale_into::<1>();  // None — "1.25" has fractional digits lost
let d1r = d2.rescale_round_into_nearest::<1>().unwrap();  // 1.3
```

## Parsing

`Decimal64<S>` implements `FromStr`. Extra fractional digits beyond `S` are silently
**truncated toward zero**. Scientific notation and underscore separators are not supported.

```rust
let d: Decimal64<4> = "123.45678".parse().unwrap();  // stores 123.4567 (truncated)
let d = Decimal64::<4>::from_slice(b"123.4567").unwrap();
let d: Decimal64<4> = ".5".parse().unwrap();         // 0.5000
let d: Decimal64<4> = "5.".parse().unwrap();         // 5.0000
```

## f64 conversions

```rust
let d = Decimal64::<4>::from_f64(1.23456789);          // NearestEven rounding
let d = Decimal64::<4>::from_f64_nearest(x);
let f: f64 = d.to_f64();
```

`from_f64` clamps on overflow; `NaN` maps to `ZERO`. `to_f64` is lossless for
`|raw| < 2^53`; larger values lose precision inherent to `f64`.

## Benchmark

Parse throughput vs. competitors at `Decimal64::<4>` on x86-64 Linux (stable Rust, optimised):

| Input               | scaledint (M/s) | f64 (M/s) | rust_decimal (M/s) | bigdecimal (M/s) |
|---------------------|-----------------|-----------|---------------------|------------------|
| `"0"`               | 214.6           | 157.2     | 102.0               | 37.1             |
| `"1.23"`            | 191.2           | 130.9     | 90.6                | 20.9             |
| `"123.4567"`        | 155.8           | 115.9     | 83.5                | 18.5             |
| `"9999999999.9999"` | 118.2           | 113.5     | 73.8                | 15.9             |
| `"-0.000001"`       | 166.6           | 115.2     | 79.5                | 18.3             |

Full results and arithmetic benchmarks: [`docs/bench-results.md`](docs/bench-results.md).

Same-scale arithmetic for all four types, `123.4567 × / ÷ 987.6543` at scale 4 (ns/op, median):

| Op    | `Decimal64` | `UDecimal64` | `Decimal32` | `UDecimal32` |
|-------|-------------|--------------|-------------|--------------|
| `add` | 0.38        | 0.30         | 0.32        | 0.32         |
| `mul` | 0.58        | 0.48         | 0.50        | 0.46         |
| `div` | 1.95        | 1.95         | 2.03        | 1.97         |

The 32-bit types run the same native 64-bit instructions as the 64-bit fast path, so they are
at parity here; what they add is half the storage and a single code path with no `i128`
fallback (the 64-bit `mul` costs ~3.1 ns once operands overflow `i64`). Details and
parse numbers: [`docs/decimal32-bench-results.md`](docs/decimal32-bench-results.md).

## System requirements

- **Stable Rust** — MSRV: 1.65 (edition 2021, `f64::round_ties_even` since 1.77;
  see Cargo.toml for exact MSRV)
- No external C dependencies
- `std` required (for `Display` and `Error`; `no_std` is a future cycle goal)

## Design

See [`docs/design.md`](docs/design.md) for the full design rationale, arithmetic rules,
parse algorithm, and const-generic limitations on stable Rust;
[`docs/udecimal64-design.md`](docs/udecimal64-design.md) for the unsigned variant; and
[`docs/decimal32-design.md`](docs/decimal32-design.md) for the 32-bit variants.
