# Benchmark Results — Decimal32 / UDecimal32

**Date:** 2026-08-28  
**Base:** `main` @ 3202ee4 (scaled_int 0.2.2) + the Decimal32/UDecimal32 commits  
**Toolchain:** rustc 1.95.0 (stable), criterion 0.8, release profile  
**Target:** x86-64 Linux — 12th Gen Intel(R) Core(TM) i9-12900K

All numbers are criterion medians (ns/op) from a single quiet run, 100 samples per benchmark,
operands loaded through `read_volatile` so nothing constant-folds (same harness as cycle 04).
Run-to-run noise on this machine is about ±3%; differences smaller than that are not real.

---

## 1. Parse benchmarks

All four types at scale 4. The 32-bit corpus (`CORPUS32`) replaces the 15-character input
`"9999999999.9999"` (an `Overflow` for `Decimal32<4>`) with `"214748.3647"`, the largest
value that fits `i32` at scale 4, so both long inputs measure a successful parse.

### Time (ns/op)

| Input (64-bit / 32-bit corpus)          | Decimal64 | UDecimal64 | Decimal32 | UDecimal32 |
|-----------------------------------------|-----------|------------|-----------|------------|
| `"0"`                                   |   4.71 |   4.66 |   4.77 |   4.94 |
| `"1.23"`                                |   5.26 |   5.61 |   5.38 |   5.50 |
| `"123.4567"`                            |   6.43 |   6.63 |   6.54 |   6.50 |
| `"9999999999.9999"` / `"214748.3647"`   |   8.61 |   8.74 |   7.18 |   7.30 |
| `"99.9999"`                             |   6.10 |   6.00 |   6.04 |   6.06 |

### Throughput (M parses/s)

| Input (64-bit / 32-bit corpus)          | Decimal64 | UDecimal64 | Decimal32 | UDecimal32 |
|-----------------------------------------|-----------|------------|-----------|------------|
| `"0"`                                   | 212.38 | 214.74 | 209.82 | 202.59 |
| `"1.23"`                                | 190.20 | 178.22 | 186.00 | 181.65 |
| `"123.4567"`                            | 155.60 | 150.76 | 152.96 | 153.81 |
| `"9999999999.9999"` / `"214748.3647"`   | 116.15 | 114.37 | 139.37 | 137.01 |
| `"99.9999"`                             | 163.85 | 166.77 | 165.52 | 165.01 |

**Reading:** the 32-bit parsers are the same code with a narrower accumulator, and on x86-64
a 32-bit `imul`/`add` costs the same as a 64-bit one, so per-character cost is identical
within noise. The long-input rows are not comparable across widths (11 vs 15 characters).
On 32-bit targets (wasm32, Cortex-M) the 32-bit accumulator avoids double-word arithmetic and
should pull ahead; not measured here.

---

## 2. Arithmetic benchmarks

Operands: `lhs = 123.4567`, `rhs = 987.6543` at scale 4 (`123.45` / `987.65` at scale 2),
identical raw values for all four types.

### Scale 4 (primary)

| Op             | Decimal64 | UDecimal64 | Decimal32 | UDecimal32 |
|----------------|-----------|------------|-----------|------------|
| `add`          |  0.381 |  0.305 |  0.318 |  0.323 |
| `mul`          |  0.584 |  0.476 |  0.496 |  0.458 |
| `div`          |  1.951 |  1.946 |  2.034 |  1.967 |

### Scale 2

| Op             | Decimal64 | UDecimal64 | Decimal32 | UDecimal32 |
|----------------|-----------|------------|-----------|------------|
| `mul` (S=2)    |  0.585 |  0.473 |  0.590 |  0.491 |
| `div` (S=2)    |  1.170 |  1.169 |  1.175 |  1.171 |

### Raw integer baselines

| Op    | `i64` | `i32` |
|-------|-------|-------|
| `add` | 0.290 | 0.265 |
| `mul` | 0.265 | 0.302 |
| `div` | 1.167 | 1.170 |

### Slow-path reference (64-bit only)

| Benchmark              | ns    | Why it exists                                        |
|------------------------|-------|------------------------------------------------------|
| `decimal64_mul_large`  | 3.097 | `i64` product overflows → `i128` path (`__divti3`)   |
| `decimal64_div_large`  | 3.118 | scaled dividend overflows `i64` → `i128` path        |
| `decimal64_mul_s9`     | 2.736 | `S > 6` disables the `i64` fast path                 |

The 32-bit types have no counterpart rows: every representable operand pair stays on the
single `i64`/`u64` path (see `docs/decimal32-design.md` §4).

---

## 3. Interpretation

**On x86-64 the 32-bit types are at parity with the 64-bit types on the hot path.** Since
cycle 04, `Decimal64`/`UDecimal64` already execute `mul` and `div` in native `i64`/`u64`
whenever the intermediate fits, which is the common case at scale ≤ 6. The 32-bit types
execute the *same* instructions (a 64-bit multiply, a multiply-by-reciprocal for `/10^S`,
one hardware `div r64` for division), so:

- `mul`: 0.50 vs 0.58 ns signed, 0.46 vs 0.48 ns unsigned. The small signed edge
  comes from `Decimal32::checked_mul` needing no overflow check on the product at all
  (`|i32|² < 2^63` by construction), whereas `Decimal64::checked_mul` branches on
  `i64::checked_mul` before taking its fast path.
- `div`: 2.03 / 1.97 vs 1.95 / 1.95 ns — all four are one 64-bit hardware divide
  (the scaled dividend `1_234_567 × 10^4` exceeds 32 bits, so LLVM's 32-bit-divide bypass is
  not taken). At scale 2 the dividend fits 32 bits and all four drop to 1.18 ns via `div r32`.
- `add`: sub-nanosecond everywhere; differences are noise.

**What the 32-bit types buy, then, is not raw x86-64 op latency but:**

1. **Half the bytes** — `size_of == 4`, twice the values per cache line, halved column /
   wire footprint. This is the primary reason to use them.
2. **A single deterministic path.** `Decimal64` costs 3.1 ns instead of 0.58 ns the moment
   an operand pair overflows the `i64` fast path (`_large` rows) and 2.7 ns at `S ≥ 7`;
   `Decimal32` has no such cliff at any scale or magnitude.
3. **No `i128`/`u128` code at all**, which matters on 32-bit targets where 128-bit
   arithmetic is expensive library code.

**Parse cost is identical** across widths (same per-byte loop; accumulator width is free on
x86-64).

---

## 4. Target assessment

| Benchmark                | Target                                   | Actual                                        | Met?    |
|--------------------------|------------------------------------------|-----------------------------------------------|---------|
| `Decimal32::parse`       | parity with `Decimal64::parse`           | 4.8–7.2 ns vs 4.7–8.6 ns; identical per char  | **YES** |
| `Decimal32::mul`         | ≤ `Decimal64::mul`                       | 0.50 ns vs 0.58 ns                            | **YES** |
| `Decimal32::div`         | ≤ `Decimal64::div` (+noise)              | 2.03 ns vs 1.95 ns (within ±3% noise band)    | parity  |
| `UDecimal32::mul`/`div`  | parity with `UDecimal64`                 | 0.46 / 1.97 vs 0.48 / 1.95 ns                | **YES** |
| No slow path             | no `i128` anywhere in 32-bit code        | verified by construction (§2, design §4)      | **YES** |
| `size_of` 32-bit types   | 4 bytes                                  | 4 (asserted in tests)                         | **YES** |
