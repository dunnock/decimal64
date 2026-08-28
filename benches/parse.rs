use atoi_simd::parse_pos;
use criterion::{Criterion, criterion_group, criterion_main};
use memchr::memchr;
use scaled_int::{Decimal64, ParseError};
use std::hint::black_box;
use std::str::FromStr;

// Positive-only corpus used by both Decimal64 and UDecimal64.
const CORPUS: &[&str] = &["0", "1.23", "123.4567", "9999999999.9999", "99.9999"];

// 32-bit corpus: identical except the long input is the largest value that fits
// `Decimal32::<4>` (i32::MAX raw), since "9999999999.9999" would be an `Overflow` error.
const CORPUS32: &[&str] = &["0", "1.23", "123.4567", "214748.3647", "99.9999"];

fn parse_scalar_s4(bytes: &[u8]) -> Result<Decimal64<4>, ParseError> {
    parse_scalar_slice::<4>(bytes)
}

fn parse_atoi_simd_s4(bytes: &[u8]) -> Result<Decimal64<4>, ParseError> {
    parse_atoi_simd_slice::<4>(bytes)
}

fn parse_scalar_slice<const S: u32>(bytes: &[u8]) -> Result<Decimal64<S>, ParseError> {
    if bytes.is_empty() {
        return Err(ParseError::Empty);
    }

    let mut i = 0usize;

    let negative = match bytes[0] {
        b'-' => {
            i += 1;
            true
        }
        b'+' => {
            i += 1;
            false
        }
        _ => false,
    };

    let mut acc: i64 = 0;
    let mut has_digits = false;

    while i < bytes.len() && bytes[i] != b'.' {
        let d = bytes[i].wrapping_sub(b'0');
        if d > 9 {
            return Err(ParseError::InvalidChar {
                byte: bytes[i],
                pos: i,
            });
        }
        acc = acc
            .checked_mul(10)
            .and_then(|v| v.checked_add(d as i64))
            .ok_or(ParseError::Overflow)?;
        has_digits = true;
        i += 1;
    }

    let mut frac_digits = 0u32;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() {
            let d = bytes[i].wrapping_sub(b'0');
            if d > 9 {
                return Err(ParseError::InvalidChar {
                    byte: bytes[i],
                    pos: i,
                });
            }
            has_digits = true;
            if frac_digits < S {
                acc = acc
                    .checked_mul(10)
                    .and_then(|v| v.checked_add(d as i64))
                    .ok_or(ParseError::Overflow)?;
                frac_digits += 1;
            }
            i += 1;
        }
    }

    if !has_digits {
        return Err(ParseError::Empty);
    }

    for _ in frac_digits..S {
        acc = acc.checked_mul(10).ok_or(ParseError::Overflow)?;
    }

    Ok(Decimal64::from_raw(if negative { -acc } else { acc }))
}

fn parse_atoi_simd_slice<const S: u32>(bytes: &[u8]) -> Result<Decimal64<S>, ParseError> {
    if bytes.is_empty() {
        return Err(ParseError::Empty);
    }

    let (negative, start) = match bytes[0] {
        b'-' => (true, 1),
        b'+' => (false, 1),
        _ => (false, 0),
    };

    if start == bytes.len() {
        return Err(ParseError::Empty);
    }

    let body = &bytes[start..];

    let (integer, fraction, fraction_offset) = match memchr(b'.', body) {
        Some(dot) => (&body[..dot], &body[dot + 1..], start + dot + 1),
        None => (body, &[][..], bytes.len()),
    };

    if integer.is_empty() && fraction.is_empty() {
        return Err(ParseError::Empty);
    }

    let kept_fraction_len = fraction.len().min(S as usize);
    let kept_fraction = &fraction[..kept_fraction_len];
    let discarded_fraction = &fraction[kept_fraction_len..];

    let integer_value = parse_digits(integer, start)?;
    let fraction_value = parse_digits(kept_fraction, fraction_offset)?;

    validate_digits(discarded_fraction, fraction_offset + kept_fraction_len)?;

    let integer_scaled = mul_pow10(integer_value, S).ok_or(ParseError::Overflow)?;
    let fraction_scaled =
        mul_pow10(fraction_value, S - kept_fraction_len as u32).ok_or(ParseError::Overflow)?;

    let magnitude = integer_scaled
        .checked_add(fraction_scaled)
        .ok_or(ParseError::Overflow)?;

    let limit = if negative {
        i64::MAX as u128 + 1
    } else {
        i64::MAX as u128
    };

    if magnitude > limit {
        return Err(ParseError::Overflow);
    }

    let raw = if negative {
        if magnitude == i64::MAX as u128 + 1 {
            i64::MIN
        } else {
            -(magnitude as i64)
        }
    } else {
        magnitude as i64
    };

    Ok(Decimal64::from_raw(raw))
}

#[cold]
fn classify_digit_error(bytes: &[u8], offset: usize) -> ParseError {
    if let Some((i, &byte)) = bytes
        .iter()
        .enumerate()
        .find(|(_, byte)| !byte.is_ascii_digit())
    {
        ParseError::InvalidChar {
            byte,
            pos: offset + i,
        }
    } else {
        ParseError::Overflow
    }
}

#[inline]
fn parse_digits(bytes: &[u8], offset: usize) -> Result<u128, ParseError> {
    if bytes.is_empty() {
        return Ok(0);
    }

    parse_pos::<u128>(bytes).map_err(|_| classify_digit_error(bytes, offset))
}

#[inline]
fn validate_digits(bytes: &[u8], offset: usize) -> Result<(), ParseError> {
    const BLOCK: usize = 38;

    for (block_index, block) in bytes.chunks(BLOCK).enumerate() {
        parse_pos::<u128>(block)
            .map_err(|_| classify_digit_error(block, offset + block_index * BLOCK))?;
    }

    Ok(())
}

#[inline]
fn mul_pow10(value: u128, exponent: u32) -> Option<u128> {
    if value == 0 {
        Some(0)
    } else {
        value.checked_mul(10u128.checked_pow(exponent)?)
    }
}

fn bench_parse_decimal64(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_decimal64");
    for s in CORPUS.iter().copied() {
        group.bench_function(s, |b| {
            b.iter(|| black_box(Decimal64::<4>::from_slice(black_box(s.as_bytes()))))
        });
    }
    group.finish();
}

fn bench_parse_decimal64_scalar_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_decimal64_scalar_baseline");
    for s in CORPUS.iter().copied() {
        group.bench_function(s, |b| {
            b.iter(|| black_box(parse_scalar_s4(black_box(s.as_bytes()))))
        });
    }
    group.finish();
}

fn bench_parse_decimal64_atoi_simd_candidate(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_decimal64_atoi_simd_candidate");
    for s in CORPUS.iter().copied() {
        group.bench_function(s, |b| {
            b.iter(|| black_box(parse_atoi_simd_s4(black_box(s.as_bytes()))))
        });
    }
    group.finish();
}

fn bench_parse_udecimal64(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_udecimal64");
    for s in CORPUS.iter().copied() {
        group.bench_function(s, |b| {
            b.iter(|| black_box(scaled_int::UDecimal64::<4>::parse(black_box(s))))
        });
    }
    group.finish();
}

fn bench_parse_decimal32(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_decimal32");
    for s in CORPUS32.iter().copied() {
        group.bench_function(s, |b| {
            b.iter(|| black_box(scaled_int::Decimal32::<4>::from_slice(black_box(s.as_bytes()))))
        });
    }
    group.finish();
}

fn bench_parse_udecimal32(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_udecimal32");
    for s in CORPUS32.iter().copied() {
        group.bench_function(s, |b| {
            b.iter(|| black_box(scaled_int::UDecimal32::<4>::parse(black_box(s))))
        });
    }
    group.finish();
}

fn bench_parse_f64(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_f64");
    for s in CORPUS.iter().copied() {
        group.bench_function(s, |b| b.iter(|| black_box(f64::from_str(black_box(s)))));
    }
    group.finish();
}

fn bench_parse_rust_decimal(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_rust_decimal");
    for s in CORPUS.iter().copied() {
        group.bench_function(s, |b| {
            b.iter(|| black_box(rust_decimal::Decimal::from_str(black_box(s))))
        });
    }
    group.finish();
}

fn bench_parse_bigdecimal(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_bigdecimal");
    for s in CORPUS.iter().copied() {
        group.bench_function(s, |b| {
            b.iter(|| black_box(bigdecimal::BigDecimal::from_str(black_box(s))))
        });
    }
    group.finish();
}

criterion_group!(
    parse_benches,
    bench_parse_decimal64,
    bench_parse_decimal64_scalar_baseline,
    bench_parse_decimal64_atoi_simd_candidate,
    bench_parse_udecimal64,
    bench_parse_decimal32,
    bench_parse_udecimal32,
    bench_parse_f64,
    bench_parse_rust_decimal,
    bench_parse_bigdecimal
);
criterion_main!(parse_benches);
