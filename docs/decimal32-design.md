# Decimal32 / UDecimal32 Design

**Date:** 2026-08-28  
**Status:** Accepted  
**Crate version:** 0.2.2 → next

---

## 1. Overview and Motivation

`Decimal32<const S: u32>` and `UDecimal32<const S: u32>` are the half-width siblings of
`Decimal64<S>` and `UDecimal64<S>`. They store fixed-point decimal values in an `i32` /
`u32` using the same scale-as-const-generic discipline: the raw integer represents the
mathematical value multiplied by `10^S`.

### 1.1 Why 32-bit Variants?

- **Storage density.** `size_of::<Decimal32<S>>() == 4` and `size_of::<Option<Decimal32<S>>>() == 8`.
  Columnar stores, tick buffers, and on-wire structs holding millions of small-magnitude
  quantities (prices in cents, percentages in basis points, weights in grams) halve their
  footprint and double the values per cache line / SIMD lane.
- **No wide-arithmetic path at all.** `Decimal64::checked_mul`/`checked_div` (cycle 04) try an
  `i64` fast path and fall back to `i128` when the product or scaled dividend overflows.
  For `Decimal32` the `i64` intermediate *always* suffices (§4), so there is a single
  straight-line path with one range check and no `i128` code anywhere.
- **Format interop.** Parquet stores `DECIMAL` with precision ≤ 9 as physical `INT32`;
  `Decimal32<S>` is the exact in-memory match (S ≤ 9, raw fits `i32`).

### 1.2 Relationship to the 64-bit Types

Shared:
- Const-generic scale, `repr(transparent)`, derive set, and every method name / trait impl
  of the corresponding 64-bit type, including the per-mode rounding families
  (`div_round_{nearest_even,nearest,zero,ceil,floor}`, `checked_div_round_*`,
  `rescale_round_into_*::<OUT>()`, `from_f64_*`).
- `ParseError` (no new variants), `no_std` gating (`f64` conversions behind `std`,
  `Display`/`ToString` tests behind `std`/`alloc`), `Scientific<D>` wrapper support, and
  `serde` support (`Serialize`/`Deserialize` as strings, `serde_as::raw_i32` / `raw_u32`
  adapters).
- Parse grammar (`[+-]?digits[.digits]`, extra fractional digits truncated, unsigned
  rejects any sign byte); `parse`, `from_slice`, `FromStr`.
- Display format of the respective sibling: `Decimal32` trims trailing fractional zeros like
  `Decimal64` (`"1.5"`, `"-0.25"`, `"42"`); `UDecimal32` zero-pads to `S` digits like
  `UDecimal64` (`"1.50"`, `"0.00"`).

Different:
- Storage: `i32` / `u32`.
- **Scale limit: `S ≤ 9`** (vs 18). `10^9 < 2^31 < 10^10`, so 10 is the first power that
  overflows `ONE` in both `i32` and `u32`.
- Intermediates: `i64` / `u64` only (vs `i64`-fast-path + `i128` slow path).
- New width-conversion API: `widen()` / `From` (lossless, to 64-bit) and `narrow()` on the
  64-bit types (`Option`, to 32-bit).

---

## 2. Internal Representation

```rust
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Decimal32<const S: u32>(i32);

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UDecimal32<const S: u32>(u32);
```

Identical justification to the 64-bit types: `repr(transparent)` makes the wrapper
ABI-identical to the primitive, the derived `Ord` on the raw integer is the numeric order
at a fixed scale, and `Hash` is the raw hash.

### 2.1 Scale Limit

`const_pow10_i32` / `const_pow10_u32` are `#[inline(always)] const fn`s asserting `s <= 9`.
Because `ONE = Self(const_pow10(S))` is an associated const, the assertion fires at compile
time the first time any method that touches `ONE` or calls `const_pow10(S)` is monomorphised
for an offending `S`.

`narrow()` on the 64-bit types is the one entry point that can mint a 32-bit type at a
scale the 32-bit type does not support (e.g. `Decimal64::<12>::narrow()`), so it references
`Decimal32::<S>::ONE` explicitly to force the check at monomorphisation. This is verified by
`compile_fail` doc-tests on both `narrow()` methods.

### 2.2 Representable Range

| S | Unit          | `Decimal32` max (±)  | `UDecimal32` max    |
|---|---------------|----------------------|---------------------|
| 0 | integer       | 2 147 483 647        | 4 294 967 295       |
| 2 | centis        | 21 474 836.47        | 42 949 672.95       |
| 4 | basis points  | 214 748.3647         | 429 496.7295        |
| 6 | micros        | 2 147.483647         | 4 294.967295        |
| 9 | nanos         | 2.147483647          | 4.294967295         |

`Decimal32::MIN` is `-2 147 483 648 / 10^S`, one unit further from zero than `-MAX`.

---

## 3. Public API Surface

Every item on `Decimal64<S>` exists on `Decimal32<S>` with `i64 → i32`, and every item on
`UDecimal64<S>` exists on `UDecimal32<S>` with `u64 → u32`. In addition, `UDecimal32` gets
`from_slice` (which `UDecimal64` currently lacks) so both 32-bit types expose the same
parse entry points.

### 3.1 Width Interop (new)

```rust
impl<const S: u32> Decimal32<S>  { pub const fn widen(self) -> Decimal64<S>; }
impl<const S: u32> UDecimal32<S> { pub const fn widen(self) -> UDecimal64<S>; }
impl<const S: u32> From<Decimal32<S>>  for Decimal64<S>  { … }
impl<const S: u32> From<UDecimal32<S>> for UDecimal64<S> { … }

impl<const S: u32> Decimal64<S>  { pub fn narrow(self) -> Option<Decimal32<S>>; }
impl<const S: u32> UDecimal64<S> { pub fn narrow(self) -> Option<UDecimal32<S>>; }
```

Widening is a sign/zero extension and always lossless, hence `From`. Narrowing fails when
the raw value is outside the 32-bit range, and follows the crate's `Option` convention
(`as_signed`, `rescale_into`) rather than `TryFrom`. Scale is preserved in both directions;
combine with `rescale_into` to change it.

The `narrow()` methods live in `decimal32.rs` / `udecimal32.rs` (inherent impls on the 64-bit
types), mirroring how `Decimal64::as_unsigned` lives in `udecimal64.rs`, so `decimal64.rs` and
`udecimal64.rs` are untouched.

### 3.2 `Scientific<D>`

`Scientific<Decimal32<S>>` and `Scientific<UDecimal32<S>>` implement `FromStr`, `Display`
(behind `std`/`alloc`) and `into_inner()`. Parsing reuses the 64-bit exponent helpers: the
mantissa is parsed at 32 bits, the exponent is applied in `i64`/`u64` (the mantissa always
fits), and the result must fit the 32-bit raw type — otherwise `ParseError::Overflow`, exactly
as a plain literal of that magnitude would be. `Underflow` semantics are unchanged.

### 3.3 `serde`

Behind the `serde` feature: `Serialize`/`Deserialize` for `Decimal32<S>`, `UDecimal32<S>`,
`Scientific<Decimal32<S>>`, `Scientific<UDecimal32<S>>` (string form, via `Display`/`FromStr`),
plus `serde_as::raw_i32` and `serde_as::raw_u32` for `#[serde(with = "…")]` raw-integer encoding.

---

## 4. Arithmetic: Why 64-bit Intermediates Always Suffice

All range checks happen once, on the final result, via `i32::try_from` / `u32::try_from`.
The intermediates themselves cannot overflow:

| Operation                    | Worst-case intermediate         | Bound            |
|------------------------------|---------------------------------|------------------|
| `checked_mul` (signed)       | `(-2^31)²`                      | `2^62 < 2^63`    |
| `checked_mul` (unsigned)     | `(2^32 − 1)²`                   | `< 2^64`         |
| `checked_div` (signed)       | `2^31 × 10^9`                   | `< 2^31 × 2^30 = 2^61` |
| `checked_div` (unsigned)     | `(2^32 − 1) × 10^9`             | `< 2^32 × 2^30 = 2^62` |
| `div_round_*`: `r × 2`       | `den < 2^32` for every caller   | `< 2^33`         |
| `rescale_into` upscale       | `raw × 10^(OUT−S)`              | uses `checked_mul` on the raw type; `None` on overflow |

Consequently `Decimal32::checked_mul` is

```rust
let product = self.0 as i64 * rhs.0 as i64;
let result = product / const_pow10_i32(S) as i64;
Some(Self(result.try_into().ok()?))
```

with no branch other than the final range check; `const_pow10_i32(S)` is a compile-time
constant, so LLVM strength-reduces the division to a multiply-high and shift.

`div_round_i64` / `div_round_u64` are line-for-line the `i128` / `u128` helpers from the
64-bit modules with the width changed. A 5 000-iteration randomised cross-check in each test
module asserts that, for inputs in 32-bit range, every `checked_*`, `checked_div_round_*`,
`rescale_round_into_*`, `rescale_into` and `Display` result equals the 64-bit result narrowed.

`saturating_mul` (signed) clamps the `i64` quotient to `[i32::MIN, i32::MAX]`; unsigned
`saturating_mul` is `checked_mul(..).unwrap_or(MAX)`, as in the 64-bit types.

---

## 5. Parsing

`parse32.rs` and `parse_unsigned32.rs` are the 64-bit parsers with the accumulator narrowed
to `i32` / `u32`. Grammar, error positions, truncation of extra fractional digits, and the
trailing `× 10^(S − frac_digits)` padding loop are unchanged.

Behavioural consequences of the narrower accumulator:

- `ParseError::Overflow` is returned for inputs that fit `i64` but not `i32`
  (`"3000000000"` at scale 0; `"1000"` at scale 9, since `1000 × 10^9 > i32::MAX`).
- As with `Decimal64` and `i64::MIN`, the most negative value (`-21474836.48` at scale 2)
  is not parseable: the magnitude is accumulated before negation and overflows first. The
  value is still representable via `from_raw` and `Display` prints it correctly.
- `UDecimal32` parses values above `i32::MAX` (`"3000000000"` at scale 0) that `Decimal32`
  rejects; `as_signed()` on them returns `None`.

The four parser files are intentional duplication following the existing precedent
(`parse.rs` vs `parse_unsigned.rs`). Unifying them behind a private accumulator trait is a
reasonable future cleanup; it was not done here to keep the change purely additive and leave
the benchmarked 64-bit hot paths untouched.

---

## 6. f64 Conversions

Same algorithm and `std` gating as the 64-bit types, with two simplifications that hold only
at 32 bits:

- `i32::MIN`, `i32::MAX`, and `u32::MAX` are exactly representable in `f64`, so the pre-cast
  `clamp` is exact and the saturating `as` cast never has to resolve a rounding ambiguity at
  the boundary.
- `to_f64` is exact on the raw value (every 32-bit integer is below `2^53`); the only
  rounding is the final division by `10^S`.

`NaN → ZERO`, `±∞` and out-of-range values clamp to `MAX`/`MIN` (signed) or `MAX`/`ZERO`
(unsigned), negative inputs to the unsigned type → `ZERO`.

---

## 7. Module Layout

```
src/
  lib.rs               exports Decimal32, UDecimal32; ParseError::Overflow doc generalised
  parse32.rs           signed i32 parser (parse, parse_slice)
  parse_unsigned32.rs  unsigned u32 parser (parse, parse_slice)
  decimal32.rs         Decimal32<S>, div_round_i64, Decimal64::narrow
  udecimal32.rs        UDecimal32<S>, div_round_u64, Decimal32::as_unsigned, UDecimal64::narrow
  scientific.rs        + Scientific<Decimal32<S>> / Scientific<UDecimal32<S>>
  serde_impls.rs       + Serialize/Deserialize for the four new type instantiations
  serde_as.rs          + raw_i32 / raw_u32
benches/
  arithmetic.rs        + decimal32_/udecimal32_ add/mul/div at scale 4 and 2, i32 baselines
  parse.rs             + parse_decimal32 / parse_udecimal32 groups over CORPUS32
```

`CORPUS32` replaces `"9999999999.9999"` (an `Overflow` for `Decimal32<4>`) with
`"214748.3647"` (`i32::MAX` raw at scale 4) so the long-input case measures a successful parse.
There are no `_large` (slow-path) benchmarks for the 32-bit types because no slow path exists.

---

## 8. Out of Scope

- `f32` conversions (`from_f32` / `to_f32`). `f32` has a 24-bit mantissa, so it cannot
  round-trip most `Decimal32` values; `f64` remains the only float bridge.
- 16- and 8-bit variants.
- Generic parser / `div_round` unification across widths (see §5).
- Cross-width arithmetic (`Decimal32 + Decimal64`); use `widen()` first.
- Harmonising the `Display` trailing-zero behaviour between the signed and unsigned families;
  the 32-bit types mirror their respective siblings as they stand today.

---

## 9. Design Invariants Summary

1. `Decimal32<S>` / `UDecimal32<S>` are `repr(transparent)` over `i32` / `u32`; `size_of == 4`.
2. `S ≤ 9`, enforced at compile time whenever `ONE` or `const_pow10_*` is instantiated;
   `narrow()` forces this check explicitly.
3. All `mul`/`div` intermediates are `i64` / `u64` and provably cannot overflow; the single
   range check is on the final result; no `i128`/`u128` is used anywhere in the 32-bit code.
4. For inputs in 32-bit range, every operation returns exactly what the 64-bit sibling
   returns, narrowed. Verified by randomised cross-check tests.
5. `widen()` is total and lossless; `narrow()` is `Option`; both preserve `S`.
6. Grammar, `ParseError`, feature gating, `Scientific` and `serde` behaviour are shared with
   the 64-bit types unchanged.
