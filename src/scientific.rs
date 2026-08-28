#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::string::ToString;
#[cfg(any(feature = "std", feature = "alloc"))]
use core::fmt;
use core::str::FromStr;

use crate::ParseError;
use crate::decimal32::Decimal32;
use crate::decimal64::Decimal64;
use crate::udecimal32::UDecimal32;
use crate::udecimal64::UDecimal64;

/// Newtype wrapper that adds scientific-notation parsing and display to any
/// `Decimal64<S>` or `UDecimal64<S>`.
///
/// The inner value is accessible via `into_inner()` or the public `.0` field.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scientific<D>(pub D);

impl<const S: u32> Scientific<Decimal64<S>> {
    pub fn into_inner(self) -> Decimal64<S> {
        self.0
    }
}

impl<const S: u32> Scientific<UDecimal64<S>> {
    pub fn into_inner(self) -> UDecimal64<S> {
        self.0
    }
}

impl<const S: u32> Scientific<Decimal32<S>> {
    pub fn into_inner(self) -> Decimal32<S> {
        self.0
    }
}

impl<const S: u32> Scientific<UDecimal32<S>> {
    pub fn into_inner(self) -> UDecimal32<S> {
        self.0
    }
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Locate the first `e` or `E` byte using an OR-mask trick: both bytes satisfy
/// `b | 0x20 == b'e'` (0x65), so a single comparison covers both cases.
/// This form is more amenable to LLVM auto-vectorization than a two-equality closure.
#[inline(always)]
fn find_exp_marker(bytes: &[u8]) -> Option<usize> {
    for (i, &b) in bytes.iter().enumerate() {
        if b | 0x20 == b'e' {
            return Some(i);
        }
    }
    None
}

#[inline(always)]
fn pow10_i64(exp: u32) -> i64 {
    const TABLE: [i64; 19] = [
        1,
        10,
        100,
        1_000,
        10_000,
        100_000,
        1_000_000,
        10_000_000,
        100_000_000,
        1_000_000_000,
        10_000_000_000,
        100_000_000_000,
        1_000_000_000_000,
        10_000_000_000_000,
        100_000_000_000_000,
        1_000_000_000_000_000,
        10_000_000_000_000_000,
        100_000_000_000_000_000,
        1_000_000_000_000_000_000,
    ];
    TABLE[exp as usize]
}

#[inline(always)]
fn pow10_u64(exp: u32) -> u64 {
    const TABLE: [u64; 20] = [
        1,
        10,
        100,
        1_000,
        10_000,
        100_000,
        1_000_000,
        10_000_000,
        100_000_000,
        1_000_000_000,
        10_000_000_000,
        100_000_000_000,
        1_000_000_000_000,
        10_000_000_000_000,
        100_000_000_000_000,
        1_000_000_000_000_000,
        10_000_000_000_000_000,
        100_000_000_000_000_000,
        1_000_000_000_000_000_000,
        10_000_000_000_000_000_000,
    ];
    TABLE[exp as usize]
}

/// Parse the substring after the `e`/`E` marker into a signed exponent.
#[inline(always)]
fn parse_exponent(s: &str) -> Result<i32, ParseError> {
    let bytes = s.as_bytes();
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
    let mut acc: i32 = 0;
    for (idx, &b) in bytes[start..].iter().enumerate() {
        let d = b.wrapping_sub(b'0');
        if d > 9 {
            return Err(ParseError::InvalidChar {
                byte: b,
                pos: start + idx,
            });
        }
        acc = acc
            .checked_mul(10)
            .and_then(|v| v.checked_add(d as i32))
            .ok_or(ParseError::Overflow)?;
    }
    Ok(if negative { -acc } else { acc })
}

#[inline(always)]
fn apply_exponent_i64(raw: i64, exponent: i32) -> Result<i64, ParseError> {
    if exponent > 0 {
        apply_positive_exp_i64(raw, exponent as u32)
    } else if exponent < 0 {
        apply_negative_exp_i64(raw, exponent.unsigned_abs())
    } else {
        Ok(raw)
    }
}

#[inline(always)]
fn apply_positive_exp_i64(raw: i64, exp: u32) -> Result<i64, ParseError> {
    if exp > 18 {
        return if raw == 0 {
            Ok(0)
        } else {
            Err(ParseError::Overflow)
        };
    }
    raw.checked_mul(pow10_i64(exp)).ok_or(ParseError::Overflow)
}

#[inline(always)]
fn apply_negative_exp_i64(raw: i64, neg_exp: u32) -> Result<i64, ParseError> {
    if neg_exp > 18 {
        return if raw == 0 {
            Ok(0)
        } else {
            Err(ParseError::Underflow)
        };
    }
    Ok(raw / pow10_i64(neg_exp))
}

#[inline(always)]
fn apply_exponent_u64(raw: u64, exponent: i32) -> Result<u64, ParseError> {
    if exponent > 0 {
        apply_positive_exp_u64(raw, exponent as u32)
    } else if exponent < 0 {
        apply_negative_exp_u64(raw, exponent.unsigned_abs())
    } else {
        Ok(raw)
    }
}

#[inline(always)]
fn apply_positive_exp_u64(raw: u64, exp: u32) -> Result<u64, ParseError> {
    if exp > 19 {
        return if raw == 0 {
            Ok(0)
        } else {
            Err(ParseError::Overflow)
        };
    }
    raw.checked_mul(pow10_u64(exp)).ok_or(ParseError::Overflow)
}

#[inline(always)]
fn apply_negative_exp_u64(raw: u64, neg_exp: u32) -> Result<u64, ParseError> {
    if neg_exp > 19 {
        return if raw == 0 {
            Ok(0)
        } else {
            Err(ParseError::Underflow)
        };
    }
    Ok(raw / pow10_u64(neg_exp))
}

// ─── FromStr implementations ──────────────────────────────────────────────────

impl<const S: u32> FromStr for Scientific<Decimal64<S>> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        match find_exp_marker(bytes) {
            None => Ok(Scientific(crate::parse::parse::<S>(s)?)),
            Some(pos) => {
                let mantissa_raw = crate::parse::parse::<S>(&s[..pos])?.raw();
                let exponent = parse_exponent(&s[pos + 1..])?;
                let raw = apply_exponent_i64(mantissa_raw, exponent)?;
                Ok(Scientific(Decimal64::from_raw(raw)))
            }
        }
    }
}

impl<const S: u32> FromStr for Scientific<UDecimal64<S>> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        match find_exp_marker(bytes) {
            None => Ok(Scientific(crate::parse_unsigned::parse::<S>(s)?)),
            Some(pos) => {
                let mantissa_raw = crate::parse_unsigned::parse::<S>(&s[..pos])?.raw();
                let exponent = parse_exponent(&s[pos + 1..])?;
                let raw = apply_exponent_u64(mantissa_raw, exponent)?;
                Ok(Scientific(UDecimal64::from_raw(raw)))
            }
        }
    }
}

/// 32-bit variant: the exponent is applied in `i64` (the mantissa always fits), then the
/// result must fit `i32` — anything larger is `ParseError::Overflow`, exactly as a literal
/// of that magnitude would be for `Decimal32<S>`.
impl<const S: u32> FromStr for Scientific<Decimal32<S>> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        match find_exp_marker(bytes) {
            None => Ok(Scientific(crate::parse32::parse::<S>(s)?)),
            Some(pos) => {
                let mantissa_raw = crate::parse32::parse::<S>(&s[..pos])?.raw() as i64;
                let exponent = parse_exponent(&s[pos + 1..])?;
                let raw = apply_exponent_i64(mantissa_raw, exponent)?;
                let raw = i32::try_from(raw).map_err(|_| ParseError::Overflow)?;
                Ok(Scientific(Decimal32::from_raw(raw)))
            }
        }
    }
}

impl<const S: u32> FromStr for Scientific<UDecimal32<S>> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        match find_exp_marker(bytes) {
            None => Ok(Scientific(crate::parse_unsigned32::parse::<S>(s)?)),
            Some(pos) => {
                let mantissa_raw = crate::parse_unsigned32::parse::<S>(&s[..pos])?.raw() as u64;
                let exponent = parse_exponent(&s[pos + 1..])?;
                let raw = apply_exponent_u64(mantissa_raw, exponent)?;
                let raw = u32::try_from(raw).map_err(|_| ParseError::Overflow)?;
                Ok(Scientific(UDecimal32::from_raw(raw)))
            }
        }
    }
}

// ─── Display implementations ──────────────────────────────────────────────────

/// Shared display logic for both signed and unsigned variants.
///
/// Emits normalized scientific notation: one significant digit before the decimal
/// point, trailing zeros in the fractional coefficient stripped, lowercase `e`.
/// Zero displays as `0e0`.
#[cfg(any(feature = "std", feature = "alloc"))]
fn fmt_scientific_u64(
    abs_raw: u64,
    scale: u32,
    negative: bool,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let coeff = abs_raw.to_string();
    let coeff_len = coeff.len();

    // exponent = number of integer digits in mathematical value - 1
    // = (coeff_len - scale - 1) for the leading digit at position 0
    let exponent: i64 = coeff_len as i64 - scale as i64 - 1;

    let lead = &coeff[..1];
    let frac = coeff[1..].trim_end_matches('0');

    if negative {
        write!(f, "-")?;
    }
    if frac.is_empty() {
        write!(f, "{}e{}", lead, exponent)
    } else {
        write!(f, "{}.{}e{}", lead, frac, exponent)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<const S: u32> fmt::Display for Scientific<Decimal64<S>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = self.0.raw();
        if raw == 0 {
            return write!(f, "0e0");
        }
        fmt_scientific_u64(raw.unsigned_abs(), S, raw < 0, f)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<const S: u32> fmt::Display for Scientific<UDecimal64<S>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = self.0.raw();
        if raw == 0 {
            return write!(f, "0e0");
        }
        fmt_scientific_u64(raw, S, false, f)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<const S: u32> fmt::Display for Scientific<Decimal32<S>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = self.0.raw();
        if raw == 0 {
            return write!(f, "0e0");
        }
        fmt_scientific_u64(raw.unsigned_abs() as u64, S, raw < 0, f)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<const S: u32> fmt::Display for Scientific<UDecimal32<S>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = self.0.raw();
        if raw == 0 {
            return write!(f, "0e0");
        }
        fmt_scientific_u64(raw as u64, S, false, f)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::string::ToString;

    type S4 = Scientific<Decimal64<4>>;
    type U4 = Scientific<UDecimal64<4>>;
    type S4_32 = Scientific<Decimal32<4>>;
    type U4_32 = Scientific<UDecimal32<4>>;

    #[test]
    fn no_exponent_delegates() {
        let r: S4 = "1.2345".parse().unwrap();
        assert_eq!(r.0.raw(), 12345);
    }

    #[test]
    fn positive_exponent_exact() {
        let r: S4 = "1.5e3".parse().unwrap();
        assert_eq!(r.0.raw(), 15_000_000);
    }

    #[test]
    fn positive_exponent_zero_mantissa() {
        let r: S4 = "0e100".parse().unwrap();
        assert_eq!(r.0.raw(), 0);
    }

    #[test]
    fn positive_exponent_overflow() {
        let r: Result<S4, _> = "9.9e18".parse();
        assert_eq!(r, Err(ParseError::Overflow));
    }

    #[test]
    fn positive_exponent_boundary_exp18() {
        // scale=0: "1" → raw=1; apply ×10^18 = 10^18 < i64::MAX
        let r: Result<Scientific<Decimal64<0>>, _> = "1e18".parse();
        assert_eq!(r.unwrap().0.raw(), 10_i64.pow(18));
    }

    #[test]
    fn negative_exponent_truncate() {
        let r: S4 = "1.5e-5".parse().unwrap();
        assert_eq!(r.0.raw(), 0);
    }

    #[test]
    fn negative_exponent_exact() {
        let r: S4 = "1.5e-3".parse().unwrap();
        assert_eq!(r.0.raw(), 15);
    }

    #[test]
    fn negative_exponent_boundary_neg18() {
        // scale=0: "1" → raw=1; apply ÷10^18 = 0 (truncation, not underflow)
        let r: Result<Scientific<Decimal64<0>>, _> = "1e-18".parse();
        assert_eq!(r.unwrap().0.raw(), 0);
    }

    #[test]
    fn negative_exponent_underflow() {
        // scale=0: "1" → raw=1; |exp|=19 > 18 → Underflow
        let r: Result<Scientific<Decimal64<0>>, _> = "1e-19".parse();
        assert_eq!(r, Err(ParseError::Underflow));
    }

    #[test]
    fn negative_exponent_zero_mantissa() {
        let r: S4 = "0e-100".parse().unwrap();
        assert_eq!(r.0.raw(), 0);
    }

    #[test]
    fn uppercase_e() {
        let r: S4 = "1.5E3".parse().unwrap();
        assert_eq!(r.0.raw(), 15_000_000);
    }

    #[test]
    fn signed_positive_exponent() {
        let r: S4 = "1.5e+3".parse().unwrap();
        assert_eq!(r.0.raw(), 15_000_000);
    }

    #[test]
    fn empty_exponent() {
        let r: Result<S4, _> = "1e".parse();
        assert_eq!(r, Err(ParseError::Empty));
    }

    #[test]
    fn sign_only_exponent() {
        let r: Result<S4, _> = "1e+".parse();
        assert_eq!(r, Err(ParseError::Empty));
    }

    #[test]
    fn invalid_exponent_char() {
        let r: Result<S4, _> = "1e3.0".parse();
        assert!(matches!(r, Err(ParseError::InvalidChar { .. })));
    }

    #[test]
    fn empty_mantissa() {
        let r: Result<S4, _> = "e5".parse();
        assert_eq!(r, Err(ParseError::Empty));
    }

    #[test]
    fn negative_mantissa_signed() {
        let r: S4 = "-1.5e3".parse().unwrap();
        assert_eq!(r.0.raw(), -15_000_000);
    }

    #[test]
    fn negative_mantissa_unsigned_rejected() {
        let r: Result<U4, _> = "-1.5e3".parse();
        assert!(matches!(r, Err(ParseError::InvalidChar { .. })));
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_zero() {
        assert_eq!(Scientific(Decimal64::<4>::from_raw(0)).to_string(), "0e0");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_basic() {
        assert_eq!(
            Scientific(Decimal64::<4>::from_raw(12345)).to_string(),
            "1.2345e0"
        );
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_small_coeff() {
        // raw=100, scale=4 → 0.0100 → 1e-2
        assert_eq!(
            Scientific(Decimal64::<4>::from_raw(100)).to_string(),
            "1e-2"
        );
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_large() {
        // raw=1234500, scale=4 → 123.45 → 1.2345e2 (trailing zeros stripped)
        assert_eq!(
            Scientific(Decimal64::<4>::from_raw(1234500)).to_string(),
            "1.2345e2"
        );
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_one_unit() {
        // raw=10000, scale=4 → 1.0 → 1e0 (fractional zeros stripped)
        assert_eq!(
            Scientific(Decimal64::<4>::from_raw(10000)).to_string(),
            "1e0"
        );
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_smallest_nonzero() {
        // raw=1, scale=4 → 0.0001 → 1e-4
        assert_eq!(Scientific(Decimal64::<4>::from_raw(1)).to_string(), "1e-4");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_negative() {
        assert_eq!(
            Scientific(Decimal64::<4>::from_raw(-12345)).to_string(),
            "-1.2345e0"
        );
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_unsigned_zero() {
        assert_eq!(Scientific(UDecimal64::<4>::from_raw(0)).to_string(), "0e0");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_unsigned_basic() {
        assert_eq!(
            Scientific(UDecimal64::<4>::from_raw(12345)).to_string(),
            "1.2345e0"
        );
    }

    #[test]
    fn into_inner_signed() {
        let s: S4 = "1.2345".parse().unwrap();
        let inner: Decimal64<4> = s.into_inner();
        assert_eq!(inner.raw(), 12345);
    }

    #[test]
    fn into_inner_unsigned() {
        let s: U4 = "1.2345".parse().unwrap();
        let inner: UDecimal64<4> = s.into_inner();
        assert_eq!(inner.raw(), 12345);
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn round_trip_display_parse() {
        let cases: &[i64] = &[
            12345,
            15_000_000,
            100,
            1234500,
            10000,
            1,
            -12345,
            -15_000_000,
            15,
            1500,
        ];
        for &raw in cases {
            let d = Scientific(Decimal64::<4>::from_raw(raw));
            let s = d.to_string();
            let parsed: S4 = s
                .parse()
                .unwrap_or_else(|e| panic!("round-trip failed for raw={raw}: s={s:?}, err={e}"));
            assert_eq!(
                parsed.0.raw(),
                raw,
                "round-trip mismatch for raw={raw}, s={s:?}"
            );
        }
    }

    #[test]
    fn unsigned_exponent_overflow() {
        // UDecimal64: very large positive exponent on nonzero value
        let r: Result<U4, _> = "1e25".parse();
        assert_eq!(r, Err(ParseError::Overflow));
    }

    #[test]
    fn unsigned_exponent_underflow() {
        // UDecimal64: exponent magnitude > 19 on nonzero mantissa
        let r: Result<Scientific<UDecimal64<0>>, _> = "1e-20".parse();
        assert_eq!(r, Err(ParseError::Underflow));
    }

    // ── 32-bit variants ─────────────────────────────────────────────────────

    #[test]
    fn d32_no_exponent_delegates() {
        let r: S4_32 = "1.2345".parse().unwrap();
        assert_eq!(r.0.raw(), 12345);
        assert_eq!(r.into_inner(), Decimal32::<4>::from_raw(12345));
    }

    #[test]
    fn d32_positive_exponent_exact() {
        let r: S4_32 = "1.2345e2".parse().unwrap();
        assert_eq!(r.0.raw(), 1_234_500);
        let r: S4_32 = "-1.2345E2".parse().unwrap();
        assert_eq!(r.0.raw(), -1_234_500);
    }

    #[test]
    fn d32_negative_exponent_truncates() {
        // 1.2345e-2 = 0.012345 → 0.0123 at scale 4
        let r: S4_32 = "1.2345e-2".parse().unwrap();
        assert_eq!(r.0.raw(), 123);
    }

    #[test]
    fn d32_result_must_fit_i32() {
        // 3e5 at scale 4 = raw 3_000_000_000 > i32::MAX, fits i64 → Overflow
        let r: Result<S4_32, _> = "3e5".parse();
        assert_eq!(r, Err(ParseError::Overflow));
        // 2e5 at scale 4 = raw 2_000_000_000 fits
        let r: S4_32 = "2e5".parse().unwrap();
        assert_eq!(r.0.raw(), 2_000_000_000);
        // huge exponent still reports Overflow, zero mantissa is fine
        assert_eq!("1e40".parse::<S4_32>(), Err(ParseError::Overflow));
        assert_eq!("0e40".parse::<S4_32>().unwrap().0.raw(), 0);
    }

    #[test]
    fn d32_underflow_matches_64() {
        assert_eq!("1e-40".parse::<S4_32>(), Err(ParseError::Underflow));
        assert_eq!("1e-40".parse::<S4>(), Err(ParseError::Underflow));
    }

    #[test]
    fn u32_parse_and_range() {
        let r: U4_32 = "1.2345e2".parse().unwrap();
        assert_eq!(r.0.raw(), 1_234_500);
        // 4e5 at scale 4 = raw 4_000_000_000 fits u32 but not i32
        let r: U4_32 = "4e5".parse().unwrap();
        assert_eq!(r.0.raw(), 4_000_000_000);
        assert_eq!("5e5".parse::<U4_32>(), Err(ParseError::Overflow));
        assert!(matches!(
            "-1e2".parse::<U4_32>(),
            Err(ParseError::InvalidChar { byte: b'-', pos: 0 })
        ));
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_32_matches_64_for_same_value() {
        for raw in [0i32, 1, -1, 12345, -12345, 1_000_000, 2_000_000_000, i32::MIN, i32::MAX] {
            let d32 = Scientific(Decimal32::<4>::from_raw(raw));
            let d64 = Scientific(Decimal32::<4>::from_raw(raw).widen());
            assert_eq!(d32.to_string(), d64.to_string(), "raw={raw}");
        }
        for raw in [0u32, 1, 12345, 1_000_000, 4_000_000_000, u32::MAX] {
            let u32v = Scientific(UDecimal32::<4>::from_raw(raw));
            let u64v = Scientific(UDecimal32::<4>::from_raw(raw).widen());
            assert_eq!(u32v.to_string(), u64v.to_string(), "raw={raw}");
        }
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_32_round_trips() {
        // Round-trips exactly when the normalised mantissa has at most S fractional digits
        // (parse truncates the mantissa to S digits before applying the exponent).
        for raw in [1i32, -1, 12345, -1_234_500, 2_000_000_000, 1_000_000_000] {
            let d = Scientific(Decimal32::<4>::from_raw(raw));
            let back: S4_32 = d.to_string().parse().unwrap();
            assert_eq!(back, d, "raw={raw}");
        }
        // A 10-significant-digit value is lossy by design — identically for both widths.
        let s = Scientific(Decimal32::<4>::MIN).to_string();
        assert_eq!(s, "-2.147483648e5");
        let back32: S4_32 = s.parse().unwrap();
        let back64: S4 = s.parse().unwrap();
        assert_eq!(back32.0.raw(), -2_147_400_000);
        assert_eq!(back32.0.widen(), back64.0);
    }
}
