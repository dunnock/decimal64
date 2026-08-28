use criterion::{Criterion, criterion_group, criterion_main};
use scaled_int::{Decimal32, Decimal64, UDecimal32, UDecimal64};
use std::hint::black_box;

// Scale 4 — primary benchmark operands (financial prices)
const LHS_RAW: i64 = 1_234_567; // 123.4567 at scale 4
const RHS_RAW: i64 = 9_876_543; // 987.6543 at scale 4

// Scale 2 — common currency scale
const LHS_RAW_S2: i64 = 12_345; // 123.45 at scale 2
const RHS_RAW_S2: i64 = 98_765; // 987.65 at scale 2

// Scale 9 — high-precision (product of 1.234567e9 raw values overflows i64 at this scale)
const LHS_RAW_S9: i64 = 1_234_567_000; // 1.234567000 at scale 9
const RHS_RAW_S9: i64 = 9_876_543_000; // 9.876543000 at scale 9

// Large-magnitude mul (S=4): product overflows i64 but result fits (slow path triggered)
//   2e9 * 5e9 = 1e19 > i64::MAX; 1e19 / 10^4 = 1e15 < i64::MAX
const MUL_LARGE_LHS: i64 = 2_000_000_000;
const MUL_LARGE_RHS: i64 = 5_000_000_000;

// Large-magnitude div (S=4): scaled numerator overflows i64 but result fits (slow path triggered)
//   i64::MAX * 10^4 > i64::MAX; i64::MAX * 10^4 / 10^4 = i64::MAX
const DIV_LARGE_LHS: i64 = i64::MAX;
const DIV_LARGE_RHS: i64 = 10_000; // = 10^4 = const_pow10(4)

// Plain statics — read_volatile prevents LLVM from proving the load is constant,
// so the arithmetic ops are not folded away at compile time.
static S_LHS: i64 = LHS_RAW;
static S_RHS: i64 = RHS_RAW;
static S_LHS_S2: i64 = LHS_RAW_S2;
static S_RHS_S2: i64 = RHS_RAW_S2;
static S_LHS_S9: i64 = LHS_RAW_S9;
static S_RHS_S9: i64 = RHS_RAW_S9;
static S_MUL_LHS: i64 = MUL_LARGE_LHS;
static S_MUL_RHS: i64 = MUL_LARGE_RHS;
static S_DIV_LHS: i64 = DIV_LARGE_LHS;
static S_DIV_RHS: i64 = DIV_LARGE_RHS;
static S_LHS_U: u64 = LHS_RAW as u64;
static S_RHS_U: u64 = RHS_RAW as u64;
static S_LHS_S2_U: u64 = LHS_RAW_S2 as u64;
static S_RHS_S2_U: u64 = RHS_RAW_S2 as u64;
static S_LHS_S9_U: u64 = LHS_RAW_S9 as u64;
static S_RHS_S9_U: u64 = RHS_RAW_S9 as u64;
static S_MUL_LHS_U: u64 = MUL_LARGE_LHS as u64;
static S_MUL_RHS_U: u64 = MUL_LARGE_RHS as u64;
static S_DIV_LHS_U: u64 = DIV_LARGE_LHS as u64;
static S_DIV_RHS_U: u64 = DIV_LARGE_RHS as u64;

// 32-bit operands: the same scale-4 and scale-2 values fit i32/u32
// (Decimal32<4> max is 214748.3647, so 987.6543 is comfortably in range).
static S_LHS_32: i32 = LHS_RAW as i32;
static S_RHS_32: i32 = RHS_RAW as i32;
static S_LHS_S2_32: i32 = LHS_RAW_S2 as i32;
static S_RHS_S2_32: i32 = RHS_RAW_S2 as i32;
static S_LHS_32_U: u32 = LHS_RAW as u32;
static S_RHS_32_U: u32 = RHS_RAW as u32;
static S_LHS_S2_32_U: u32 = LHS_RAW_S2 as u32;
static S_RHS_S2_32_U: u32 = RHS_RAW_S2 as u32;

/// Load a signed value via volatile read to prevent constant-folding.
///
/// # Safety
/// Pointer is valid and aligned (it's a reference to a static).
#[inline(always)]
unsafe fn vload_i64(p: &i64) -> i64 {
    // SAFETY: this is just for test to not cache values
    unsafe { std::ptr::read_volatile(p) }
}

/// Load an unsigned value via volatile read to prevent constant-folding.
///
/// # Safety
/// Pointer is valid and aligned (it's a reference to a static).
#[inline(always)]
unsafe fn vload_u64(p: &u64) -> u64 {
    // SAFETY: this is just for test to not cache values
    unsafe { std::ptr::read_volatile(p) }
}

/// 32-bit signed volatile load; see `vload_i64`.
#[inline(always)]
unsafe fn vload_i32(p: &i32) -> i32 {
    // SAFETY: this is just for test to not cache values
    unsafe { std::ptr::read_volatile(p) }
}

/// 32-bit unsigned volatile load; see `vload_u64`.
#[inline(always)]
unsafe fn vload_u32(p: &u32) -> u32 {
    // SAFETY: this is just for test to not cache values
    unsafe { std::ptr::read_volatile(p) }
}

fn bench_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("arithmetic");

    // ── Scale 4 (primary) ─────────────────────────────────────────────────────

    group.bench_function("decimal64_add", |b| {
        b.iter(|| {
            // SAFETY: reading from an aligned static i64; volatile prevents constant-folding.
            let lhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_LHS) });
            let rhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_RHS) });
            black_box(lhs + rhs)
        })
    });

    group.bench_function("udecimal64_add", |b| {
        b.iter(|| {
            let lhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_LHS_U) });
            let rhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_RHS_U) });
            black_box(lhs + rhs)
        })
    });

    group.bench_function("i64_add", |b| {
        b.iter(|| {
            let lhs = unsafe { vload_i64(&S_LHS) };
            let rhs = unsafe { vload_i64(&S_RHS) };
            black_box(lhs + rhs)
        })
    });

    group.bench_function("decimal64_mul", |b| {
        b.iter(|| {
            let lhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_LHS) });
            let rhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_RHS) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("udecimal64_mul", |b| {
        b.iter(|| {
            let lhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_LHS_U) });
            let rhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_RHS_U) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("i64_mul", |b| {
        b.iter(|| {
            let lhs = unsafe { vload_i64(&S_LHS) };
            let rhs = unsafe { vload_i64(&S_RHS) };
            black_box(lhs * rhs)
        })
    });

    group.bench_function("decimal64_div", |b| {
        b.iter(|| {
            let lhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_LHS) });
            let rhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_RHS) });
            black_box(lhs / rhs)
        })
    });

    group.bench_function("udecimal64_div", |b| {
        b.iter(|| {
            let lhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_LHS_U) });
            let rhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_RHS_U) });
            black_box(lhs / rhs)
        })
    });

    group.bench_function("i64_div", |b| {
        b.iter(|| {
            let lhs = unsafe { vload_i64(&S_LHS) };
            let rhs = unsafe { vload_i64(&S_RHS) };
            black_box(lhs / rhs)
        })
    });

    // ── Scale 2 ───────────────────────────────────────────────────────────────

    group.bench_function("decimal64_mul_s2", |b| {
        b.iter(|| {
            let lhs = Decimal64::<2>::from_raw(unsafe { vload_i64(&S_LHS_S2) });
            let rhs = Decimal64::<2>::from_raw(unsafe { vload_i64(&S_RHS_S2) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("udecimal64_mul_s2", |b| {
        b.iter(|| {
            let lhs = UDecimal64::<2>::from_raw(unsafe { vload_u64(&S_LHS_S2_U) });
            let rhs = UDecimal64::<2>::from_raw(unsafe { vload_u64(&S_RHS_S2_U) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("decimal64_div_s2", |b| {
        b.iter(|| {
            let lhs = Decimal64::<2>::from_raw(unsafe { vload_i64(&S_LHS_S2) });
            let rhs = Decimal64::<2>::from_raw(unsafe { vload_i64(&S_RHS_S2) });
            black_box(lhs / rhs)
        })
    });

    group.bench_function("udecimal64_div_s2", |b| {
        b.iter(|| {
            let lhs = UDecimal64::<2>::from_raw(unsafe { vload_u64(&S_LHS_S2_U) });
            let rhs = UDecimal64::<2>::from_raw(unsafe { vload_u64(&S_RHS_S2_U) });
            black_box(lhs / rhs)
        })
    });

    // ── Scale 9 ───────────────────────────────────────────────────────────────

    group.bench_function("decimal64_mul_s9", |b| {
        b.iter(|| {
            let lhs = Decimal64::<9>::from_raw(unsafe { vload_i64(&S_LHS_S9) });
            let rhs = Decimal64::<9>::from_raw(unsafe { vload_i64(&S_RHS_S9) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("udecimal64_mul_s9", |b| {
        b.iter(|| {
            let lhs = UDecimal64::<9>::from_raw(unsafe { vload_u64(&S_LHS_S9_U) });
            let rhs = UDecimal64::<9>::from_raw(unsafe { vload_u64(&S_RHS_S9_U) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("decimal64_div_s9", |b| {
        b.iter(|| {
            let lhs = Decimal64::<9>::from_raw(unsafe { vload_i64(&S_LHS_S9) });
            let rhs = Decimal64::<9>::from_raw(unsafe { vload_i64(&S_RHS_S9) });
            black_box(lhs / rhs)
        })
    });

    group.bench_function("udecimal64_div_s9", |b| {
        b.iter(|| {
            let lhs = UDecimal64::<9>::from_raw(unsafe { vload_u64(&S_LHS_S9_U) });
            let rhs = UDecimal64::<9>::from_raw(unsafe { vload_u64(&S_RHS_S9_U) });
            black_box(lhs / rhs)
        })
    });

    // ── Large magnitude — i128/u128 slow path ─────────────────────────────────
    // mul: 2e9 * 5e9 = 1e19 > i64::MAX; result 1e15 fits. Fast path overflows → slow path.
    // div: i64::MAX * 10^4 > i64::MAX; result = i64::MAX fits. Fast path overflows → slow path.

    group.bench_function("decimal64_mul_large", |b| {
        b.iter(|| {
            let lhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_MUL_LHS) });
            let rhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_MUL_RHS) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("udecimal64_mul_large", |b| {
        b.iter(|| {
            let lhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_MUL_LHS_U) });
            let rhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_MUL_RHS_U) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("decimal64_div_large", |b| {
        b.iter(|| {
            let lhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_DIV_LHS) });
            let rhs = Decimal64::<4>::from_raw(unsafe { vload_i64(&S_DIV_RHS) });
            black_box(lhs / rhs)
        })
    });

    group.bench_function("udecimal64_div_large", |b| {
        b.iter(|| {
            let lhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_DIV_LHS_U) });
            let rhs = UDecimal64::<4>::from_raw(unsafe { vload_u64(&S_DIV_RHS_U) });
            black_box(lhs / rhs)
        })
    });

    // ── 32-bit types — scale 4 (primary) and scale 2 ─────────────────────────
    // No slow path exists: i64/u64 intermediates always suffice, so no `_large` variants.

    group.bench_function("decimal32_add", |b| {
        b.iter(|| {
            let lhs = Decimal32::<4>::from_raw(unsafe { vload_i32(&S_LHS_32) });
            let rhs = Decimal32::<4>::from_raw(unsafe { vload_i32(&S_RHS_32) });
            black_box(lhs + rhs)
        })
    });

    group.bench_function("udecimal32_add", |b| {
        b.iter(|| {
            let lhs = UDecimal32::<4>::from_raw(unsafe { vload_u32(&S_LHS_32_U) });
            let rhs = UDecimal32::<4>::from_raw(unsafe { vload_u32(&S_RHS_32_U) });
            black_box(lhs + rhs)
        })
    });

    group.bench_function("i32_add", |b| {
        b.iter(|| {
            let lhs = unsafe { vload_i32(&S_LHS_32) };
            let rhs = unsafe { vload_i32(&S_RHS_32) };
            black_box(lhs + rhs)
        })
    });

    group.bench_function("decimal32_mul", |b| {
        b.iter(|| {
            let lhs = Decimal32::<4>::from_raw(unsafe { vload_i32(&S_LHS_32) });
            let rhs = Decimal32::<4>::from_raw(unsafe { vload_i32(&S_RHS_32) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("udecimal32_mul", |b| {
        b.iter(|| {
            let lhs = UDecimal32::<4>::from_raw(unsafe { vload_u32(&S_LHS_32_U) });
            let rhs = UDecimal32::<4>::from_raw(unsafe { vload_u32(&S_RHS_32_U) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("i32_mul", |b| {
        b.iter(|| {
            let lhs = unsafe { vload_i32(&S_LHS_32) };
            let rhs = unsafe { vload_i32(&S_RHS_32) };
            black_box(lhs.wrapping_mul(rhs))
        })
    });

    group.bench_function("decimal32_div", |b| {
        b.iter(|| {
            let lhs = Decimal32::<4>::from_raw(unsafe { vload_i32(&S_LHS_32) });
            let rhs = Decimal32::<4>::from_raw(unsafe { vload_i32(&S_RHS_32) });
            black_box(lhs / rhs)
        })
    });

    group.bench_function("udecimal32_div", |b| {
        b.iter(|| {
            let lhs = UDecimal32::<4>::from_raw(unsafe { vload_u32(&S_LHS_32_U) });
            let rhs = UDecimal32::<4>::from_raw(unsafe { vload_u32(&S_RHS_32_U) });
            black_box(lhs / rhs)
        })
    });

    group.bench_function("i32_div", |b| {
        b.iter(|| {
            let lhs = unsafe { vload_i32(&S_LHS_32) };
            let rhs = unsafe { vload_i32(&S_RHS_32) };
            black_box(lhs / rhs)
        })
    });

    group.bench_function("decimal32_mul_s2", |b| {
        b.iter(|| {
            let lhs = Decimal32::<2>::from_raw(unsafe { vload_i32(&S_LHS_S2_32) });
            let rhs = Decimal32::<2>::from_raw(unsafe { vload_i32(&S_RHS_S2_32) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("udecimal32_mul_s2", |b| {
        b.iter(|| {
            let lhs = UDecimal32::<2>::from_raw(unsafe { vload_u32(&S_LHS_S2_32_U) });
            let rhs = UDecimal32::<2>::from_raw(unsafe { vload_u32(&S_RHS_S2_32_U) });
            black_box(lhs * rhs)
        })
    });

    group.bench_function("decimal32_div_s2", |b| {
        b.iter(|| {
            let lhs = Decimal32::<2>::from_raw(unsafe { vload_i32(&S_LHS_S2_32) });
            let rhs = Decimal32::<2>::from_raw(unsafe { vload_i32(&S_RHS_S2_32) });
            black_box(lhs / rhs)
        })
    });

    group.bench_function("udecimal32_div_s2", |b| {
        b.iter(|| {
            let lhs = UDecimal32::<2>::from_raw(unsafe { vload_u32(&S_LHS_S2_32_U) });
            let rhs = UDecimal32::<2>::from_raw(unsafe { vload_u32(&S_RHS_S2_32_U) });
            black_box(lhs / rhs)
        })
    });

    group.finish();
}

criterion_group!(arith_benches, bench_arithmetic);
criterion_main!(arith_benches);
