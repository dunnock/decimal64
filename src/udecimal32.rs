use core::fmt;
use core::ops::{Add, Div, Mul, Sub};
use core::str::FromStr;

use crate::{RoundFlag, RoundFlagEnum};

#[inline(always)]
const fn const_pow10_u32(s: u32) -> u32 {
    assert!(s <= 9, "UDecimal32 scale must be <= 9");
    let mut result: u32 = 1;
    let mut i = 0u32;
    while i < s {
        result *= 10;
        i += 1;
    }
    result
}

/// Fixed-scale 32-bit unsigned decimal.
///
/// The raw value is a `u32` whose unit is `10^(-S)`.
/// Scale `S` is a compile-time const; no runtime overhead.
///
/// Only non-negative values are representable; the type system enforces this statically.
/// The full `u32` range doubles the positive capacity vs [`crate::Decimal32`].
/// This is the half-width sibling of [`crate::UDecimal64`]: same API and semantics,
/// half the storage, and every intermediate fits in native `u64` (no 128-bit path).
///
/// # Scale limit
///
/// `S` must be ≤ 9; larger values overflow `ONE = 10^S` and are rejected at compile time.
///
/// # Range
///
/// `value ≤ 4294967295 / 10^S` — e.g. `42949672.95` at scale 2, `4.294967295` at scale 9.
///
/// ```rust
/// use scaled_int::UDecimal32;
///
/// let qty: UDecimal32<2> = "42949672.95".parse().unwrap();
/// assert_eq!(qty, UDecimal32::MAX);
/// assert_eq!(core::mem::size_of::<UDecimal32<2>>(), 4);
/// assert!("-1.00".parse::<UDecimal32<2>>().is_err());
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UDecimal32<const S: u32>(u32);

impl<const S: u32> UDecimal32<S> {
    /// The scale parameter `S`.
    pub const SCALE: u32 = S;
    /// Additive identity: `0`.
    pub const ZERO: Self = Self(0);
    /// Multiplicative identity: `1.0` stored as `10^S`.
    pub const ONE: Self = Self(const_pow10_u32(S));
    /// Largest representable value (`u32::MAX` raw).
    pub const MAX: Self = Self(u32::MAX);

    /// Wrap a raw `u32` without any scaling — caller manages the scale invariant.
    #[inline(always)]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Return the raw `u32` storage value (the mathematical value × `10^S`).
    #[inline(always)]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

// ── Parse ────────────────────────────────────────────────────────────────────

impl<const S: u32> UDecimal32<S> {
    /// Parse a decimal string. Signs (`+`, `-`) are rejected. Extra fractional digits
    /// beyond `S` are silently truncated toward zero.
    #[inline]
    pub fn parse(s: &str) -> Result<Self, crate::ParseError> {
        Self::from_slice(s.as_bytes())
    }

    /// Parse decimal bytes. Signs are rejected; extra fractional digits are truncated.
    #[inline]
    pub fn from_slice(bytes: &[u8]) -> Result<Self, crate::ParseError> {
        crate::parse_unsigned32::parse_slice::<S>(bytes)
    }
}

impl<const S: u32> FromStr for UDecimal32<S> {
    type Err = crate::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_slice(s.as_bytes())
    }
}

// ── f64 conversions ──────────────────────────────────────────────────────────

impl<const S: u32> UDecimal32<S> {
    /// Convert from `f64` using nearest-even (banker's) rounding.
    ///
    /// `NaN` and negative inputs map to `ZERO`; overflow clamps to `MAX`.
    #[cfg(feature = "std")]
    #[inline]
    pub fn from_f64(x: f64) -> Self {
        Self::from_f64_round_impl::<{ RoundFlag::NEAREST_EVEN }>(x)
    }

    /// Convert from `f64` using nearest-even (banker's) rounding.
    #[cfg(feature = "std")]
    #[inline]
    pub fn from_f64_nearest_even(x: f64) -> Self {
        Self::from_f64_round_impl::<{ RoundFlag::NEAREST_EVEN }>(x)
    }

    /// Convert from `f64` using nearest, ties away from zero.
    #[cfg(feature = "std")]
    #[inline]
    pub fn from_f64_nearest(x: f64) -> Self {
        Self::from_f64_round_impl::<{ RoundFlag::NEAREST }>(x)
    }

    /// Convert from `f64` by truncating toward zero.
    #[cfg(feature = "std")]
    #[inline]
    pub fn from_f64_zero(x: f64) -> Self {
        Self::from_f64_round_impl::<{ RoundFlag::ZERO }>(x)
    }

    /// Convert from `f64` by rounding toward positive infinity.
    #[cfg(feature = "std")]
    #[inline]
    pub fn from_f64_ceil(x: f64) -> Self {
        Self::from_f64_round_impl::<{ RoundFlag::CEIL }>(x)
    }

    /// Convert from `f64` by rounding toward negative infinity.
    #[cfg(feature = "std")]
    #[inline]
    pub fn from_f64_floor(x: f64) -> Self {
        Self::from_f64_round_impl::<{ RoundFlag::FLOOR }>(x)
    }

    #[cfg(feature = "std")]
    fn from_f64_round_impl<const MODE: RoundFlagEnum>(x: f64) -> Self {
        if x.is_nan() || x < 0.0 {
            return Self::ZERO;
        }
        let scaled = x * (const_pow10_u32(S) as f64);
        let rounded = match RoundFlag::from_u8(MODE) {
            RoundFlag::NearestEven => scaled.round_ties_even(),
            RoundFlag::Nearest => scaled.round(),
            RoundFlag::Zero => scaled.trunc(),
            RoundFlag::Ceil => scaled.ceil(),
            RoundFlag::Floor => scaled.floor(),
        };
        // u32::MAX is exactly representable in f64, so the clamp is exact.
        let clamped = rounded.clamp(0.0, u32::MAX as f64);
        Self(clamped as u32)
    }

    /// Convert to `f64`. Every `u32` is exactly representable in `f64`, so the only
    /// rounding is the final division by `10^S`.
    #[inline]
    pub fn to_f64(self) -> f64 {
        (self.0 as f64) / (const_pow10_u32(S) as f64)
    }
}

// ── Signed/unsigned interop ──────────────────────────────────────────────────

impl<const S: u32> UDecimal32<S> {
    /// Convert to `Decimal32<S>`. Returns `None` when the raw value exceeds `i32::MAX`.
    pub fn as_signed(self) -> Option<crate::Decimal32<S>> {
        i32::try_from(self.0).ok().map(crate::Decimal32::from_raw)
    }
}

/// Extension on `Decimal32<S>` to convert to the unsigned counterpart.
impl<const S: u32> crate::Decimal32<S> {
    /// Convert to `UDecimal32<S>`. Returns `None` for negative values.
    pub fn as_unsigned(self) -> Option<UDecimal32<S>> {
        u32::try_from(self.raw()).ok().map(UDecimal32::from_raw)
    }
}

// ── Width interop ────────────────────────────────────────────────────────────

impl<const S: u32> UDecimal32<S> {
    /// Widen to [`crate::UDecimal64<S>`]. Always lossless.
    #[inline(always)]
    pub const fn widen(self) -> crate::UDecimal64<S> {
        crate::UDecimal64::from_raw(self.0 as u64)
    }
}

impl<const S: u32> From<UDecimal32<S>> for crate::UDecimal64<S> {
    #[inline(always)]
    fn from(d: UDecimal32<S>) -> Self {
        d.widen()
    }
}

/// Extension on `UDecimal64<S>` to convert to the half-width counterpart.
impl<const S: u32> crate::UDecimal64<S> {
    /// Narrow to [`UDecimal32<S>`]. Returns `None` when the raw value exceeds `u32::MAX`.
    ///
    /// ```rust
    /// use scaled_int::{UDecimal32, UDecimal64};
    ///
    /// let d: UDecimal64<2> = "123.45".parse().unwrap();
    /// assert_eq!(d.narrow(), Some(UDecimal32::<2>::from_raw(12345)));
    /// assert_eq!(UDecimal64::<2>::MAX.narrow(), None);
    /// ```
    ///
    /// Rejected at compile time when `S > 9` (the `UDecimal32` scale limit):
    ///
    /// ```compile_fail
    /// use scaled_int::UDecimal64;
    /// let _ = UDecimal64::<12>::from_raw(1).narrow();
    /// ```
    pub fn narrow(self) -> Option<UDecimal32<S>> {
        let _ = UDecimal32::<S>::ONE; // force the S <= 9 assertion at monomorphisation
        u32::try_from(self.raw()).ok().map(UDecimal32::from_raw)
    }
}

// ── Arithmetic trait impls ───────────────────────────────────────────────────

impl<const S: u32> Add for UDecimal32<S> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        self.checked_add(rhs).expect("UDecimal32 addition overflow")
    }
}

/// Subtraction returns `Option<Self>` to prevent silent underflow.
///
/// Use `saturating_sub` to clamp to `ZERO` instead of propagating `None`.
impl<const S: u32> Sub for UDecimal32<S> {
    type Output = Option<Self>;
    #[inline]
    fn sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }
}

impl<const S: u32> Mul for UDecimal32<S> {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        self.checked_mul(rhs)
            .expect("UDecimal32 multiplication overflow")
    }
}

impl<const S: u32> Div for UDecimal32<S> {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        self.checked_div(rhs).expect("UDecimal32 division by zero")
    }
}

// ── Checked / saturating / rounding variants ─────────────────────────────────
//
// All intermediates are `u64` and cannot overflow: (u32::MAX)² < 2^64 and
// u32::MAX × 10^9 < 2^62. No fast/slow path split is needed; the single range check
// is `u32::try_from` on the final result.

impl<const S: u32> UDecimal32<S> {
    /// Returns `None` on overflow.
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    /// Returns `None` on underflow (same behavior as the `Sub` trait).
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    /// Returns `None` on overflow.
    #[inline(always)]
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let product = self.0 as u64 * rhs.0 as u64;
        let result = product / const_pow10_u32(S) as u64;
        Some(Self(result.try_into().ok()?))
    }

    /// Returns `None` on division by zero or result overflow.
    #[inline(always)]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.0 == 0 {
            return None;
        }
        let num = self.0 as u64 * const_pow10_u32(S) as u64;
        let result = num / rhs.0 as u64;
        Some(Self(result.try_into().ok()?))
    }

    /// Clamps to `MAX` on overflow instead of panicking.
    #[inline]
    pub fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Clamps to `ZERO` on underflow instead of wrapping.
    #[inline]
    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Clamps to `MAX` on overflow instead of panicking.
    #[inline]
    pub fn saturating_mul(self, rhs: Self) -> Self {
        self.checked_mul(rhs).unwrap_or(Self::MAX)
    }

    /// Divide using nearest-even (banker's) rounding. Panics on division by zero or overflow.
    #[inline]
    pub fn div_round_nearest_even(self, rhs: Self) -> Self {
        self.div_round_impl::<{ RoundFlag::NEAREST_EVEN }>(rhs)
    }

    /// Divide using nearest, ties away from zero. Panics on division by zero or overflow.
    #[inline]
    pub fn div_round_nearest(self, rhs: Self) -> Self {
        self.div_round_impl::<{ RoundFlag::NEAREST }>(rhs)
    }

    /// Divide by truncating toward zero. Panics on division by zero or overflow.
    #[inline]
    pub fn div_round_zero(self, rhs: Self) -> Self {
        self.div_round_impl::<{ RoundFlag::ZERO }>(rhs)
    }

    /// Divide by rounding toward positive infinity. Panics on division by zero or overflow.
    #[inline]
    pub fn div_round_ceil(self, rhs: Self) -> Self {
        self.div_round_impl::<{ RoundFlag::CEIL }>(rhs)
    }

    /// Divide by rounding toward negative infinity. Panics on division by zero or overflow.
    #[inline]
    pub fn div_round_floor(self, rhs: Self) -> Self {
        self.div_round_impl::<{ RoundFlag::FLOOR }>(rhs)
    }

    /// Divide using nearest-even (banker's) rounding. Returns `None` on division by zero or overflow.
    #[inline]
    pub fn checked_div_round_nearest_even(self, rhs: Self) -> Option<Self> {
        self.checked_div_round_impl::<{ RoundFlag::NEAREST_EVEN }>(rhs)
    }

    /// Divide using nearest, ties away from zero. Returns `None` on division by zero or overflow.
    #[inline]
    pub fn checked_div_round_nearest(self, rhs: Self) -> Option<Self> {
        self.checked_div_round_impl::<{ RoundFlag::NEAREST }>(rhs)
    }

    /// Divide by truncating toward zero. Returns `None` on division by zero or overflow.
    #[inline]
    pub fn checked_div_round_zero(self, rhs: Self) -> Option<Self> {
        self.checked_div_round_impl::<{ RoundFlag::ZERO }>(rhs)
    }

    /// Divide by rounding toward positive infinity. Returns `None` on division by zero or overflow.
    #[inline]
    pub fn checked_div_round_ceil(self, rhs: Self) -> Option<Self> {
        self.checked_div_round_impl::<{ RoundFlag::CEIL }>(rhs)
    }

    /// Divide by rounding toward negative infinity. Returns `None` on division by zero or overflow.
    #[inline]
    pub fn checked_div_round_floor(self, rhs: Self) -> Option<Self> {
        self.checked_div_round_impl::<{ RoundFlag::FLOOR }>(rhs)
    }

    #[inline]
    fn div_round_impl<const MODE: RoundFlagEnum>(self, rhs: Self) -> Self {
        self.checked_div_round_impl::<MODE>(rhs)
            .expect("UDecimal32 div_round: division by zero or overflow")
    }

    #[inline]
    fn checked_div_round_impl<const MODE: RoundFlagEnum>(self, rhs: Self) -> Option<Self> {
        if rhs.0 == 0 {
            return None;
        }
        let num = self.0 as u64 * const_pow10_u32(S) as u64;
        let result = div_round_u64::<MODE>(num, rhs.0 as u64);
        Some(Self(result.try_into().ok()?))
    }

    /// Lossless rescale. Returns `None` if fractional digits would be lost or on overflow.
    pub fn rescale_into<const OUT: u32>(self) -> Option<UDecimal32<OUT>> {
        if OUT > S {
            let factor = const_pow10_u32(OUT - S);
            self.0.checked_mul(factor).map(UDecimal32::from_raw)
        } else if OUT < S {
            let factor = const_pow10_u32(S - OUT);
            if !self.0.is_multiple_of(factor) {
                None
            } else {
                Some(UDecimal32::from_raw(self.0 / factor))
            }
        } else {
            Some(UDecimal32::from_raw(self.0))
        }
    }

    /// Rescale using nearest-even (banker's) rounding. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_nearest_even<const OUT: u32>(self) -> Option<UDecimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::NEAREST_EVEN }>()
    }

    /// Rescale using nearest, ties away from zero. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_nearest<const OUT: u32>(self) -> Option<UDecimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::NEAREST }>()
    }

    /// Rescale by truncating toward zero. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_zero<const OUT: u32>(self) -> Option<UDecimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::ZERO }>()
    }

    /// Rescale by rounding toward positive infinity. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_ceil<const OUT: u32>(self) -> Option<UDecimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::CEIL }>()
    }

    /// Rescale by rounding toward negative infinity. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_floor<const OUT: u32>(self) -> Option<UDecimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::FLOOR }>()
    }

    #[inline]
    fn rescale_round_into_impl<const OUT: u32, const MODE: RoundFlagEnum>(
        self,
    ) -> Option<UDecimal32<OUT>> {
        if OUT > S {
            let factor = const_pow10_u32(OUT - S);
            self.0.checked_mul(factor).map(UDecimal32::from_raw)
        } else if OUT < S {
            let factor = const_pow10_u32(S - OUT) as u64;
            let result = div_round_u64::<MODE>(self.0 as u64, factor);
            u32::try_from(result).ok().map(UDecimal32::from_raw)
        } else {
            Some(UDecimal32::from_raw(self.0))
        }
    }
}

/// Rounding integer division for non-negative u64 values. `den` must be non-zero.
///
/// Same algorithm as `udecimal64::div_round_u128`, specialised to `u64`. `r * 2` cannot
/// overflow: every caller passes `den < 2^32`, so `r < 2^32`.
fn div_round_u64<const MODE: RoundFlagEnum>(num: u64, den: u64) -> u64 {
    debug_assert!(den != 0);
    let q = num / den;
    let r = num % den;
    if r == 0 {
        return q;
    }
    match RoundFlag::from_u8(MODE) {
        RoundFlag::Zero => q,
        RoundFlag::Ceil => q + 1,
        RoundFlag::Floor => q,
        RoundFlag::Nearest => {
            if r * 2 >= den { q + 1 } else { q }
        }
        RoundFlag::NearestEven => {
            let r2 = r * 2;
            if r2 > den {
                q + 1
            } else if r2 == den {
                if !q.is_multiple_of(2) { q + 1 } else { q }
            } else {
                q
            }
        }
    }
}

// ── Display / Debug ──────────────────────────────────────────────────────────

/// Same format as `UDecimal64`: fraction zero-padded to exactly `S` digits (`"1.50"`).
impl<const S: u32> fmt::Display for UDecimal32<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if S == 0 {
            return write!(f, "{}", self.0);
        }
        let divisor = const_pow10_u32(S);
        let integer = self.0 / divisor;
        let frac = self.0 % divisor;
        write!(f, "{}.{:0>width$}", integer, frac, width = S as usize)
    }
}

impl<const S: u32> fmt::Debug for UDecimal32<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UDecimal32<{}>({})", S, self.0)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Decimal32, ParseError, UDecimal64};
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::format;
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::string::ToString;

    // ── Constants / layout ───────────────────────────────────────────────────

    #[test]
    fn one_raw_equals_pow10() {
        assert_eq!(UDecimal32::<4>::ONE.raw(), 10_000);
    }

    #[test]
    fn one_at_max_scale() {
        assert_eq!(UDecimal32::<9>::ONE.raw(), 1_000_000_000);
    }

    #[test]
    fn max_raw_is_u32_max() {
        assert_eq!(UDecimal32::<4>::MAX.raw(), u32::MAX);
    }

    #[test]
    fn zero_raw_is_zero() {
        assert_eq!(UDecimal32::<4>::ZERO.raw(), 0);
    }

    #[test]
    fn size_is_four_bytes() {
        assert_eq!(core::mem::size_of::<UDecimal32<4>>(), 4);
    }

    // ── Parse ────────────────────────────────────────────────────────────────

    #[test]
    fn parse_zero() {
        let d: UDecimal32<4> = "0".parse().unwrap();
        assert_eq!(d, UDecimal32::ZERO);
    }

    #[test]
    fn parse_basic_fractional() {
        let d: UDecimal32<4> = "1.2345".parse().unwrap();
        assert_eq!(d.raw(), 12345);
    }

    #[test]
    fn parse_truncation() {
        let d: UDecimal32<4> = "1.23456".parse().unwrap();
        assert_eq!(d.raw(), 12345);
    }

    #[test]
    fn parse_from_slice() {
        let d = UDecimal32::<4>::from_slice(b"123.4567").unwrap();
        assert_eq!(d.raw(), 1_234_567);
    }

    #[test]
    fn parse_plus_sign_rejected() {
        let r: Result<UDecimal32<2>, _> = "+1.00".parse();
        assert_eq!(r, Err(ParseError::InvalidChar { byte: b'+', pos: 0 }));
    }

    #[test]
    fn parse_minus_sign_rejected() {
        let r: Result<UDecimal32<2>, _> = "-1.00".parse();
        assert_eq!(r, Err(ParseError::InvalidChar { byte: b'-', pos: 0 }));
    }

    #[test]
    fn parse_minus_zero_rejected() {
        let r: Result<UDecimal32<2>, _> = "-0".parse();
        assert_eq!(r, Err(ParseError::InvalidChar { byte: b'-', pos: 0 }));
    }

    #[test]
    fn parse_empty() {
        let r: Result<UDecimal32<2>, _> = "".parse();
        assert_eq!(r, Err(ParseError::Empty));
    }

    #[test]
    fn parse_overflow_many_digits() {
        let r: Result<UDecimal32<2>, _> = "99999999999".parse();
        assert_eq!(r, Err(ParseError::Overflow));
    }

    #[test]
    fn parse_u32_max_boundary() {
        // 42949672.95 at scale 2 == u32::MAX raw
        let d: UDecimal32<2> = "42949672.95".parse().unwrap();
        assert_eq!(d, UDecimal32::MAX);
        let r: Result<UDecimal32<2>, _> = "42949672.96".parse();
        assert_eq!(r, Err(ParseError::Overflow));
    }

    #[test]
    fn parse_above_i32_max_succeeds() {
        // 3 billion fits u32 but not i32
        let d: UDecimal32<0> = "3000000000".parse().unwrap();
        assert_eq!(d.raw(), 3_000_000_000);
        assert_eq!(d.as_signed(), None);
    }

    #[test]
    fn parse_scale_padding_overflow() {
        // "5" fits u32 but 5 * 10^9 does not
        let r: Result<UDecimal32<9>, _> = "5".parse();
        assert_eq!(r, Err(ParseError::Overflow));
        let d: UDecimal32<9> = "4".parse().unwrap();
        assert_eq!(d.raw(), 4_000_000_000);
    }

    #[test]
    fn parse_dot_only_is_empty() {
        let r: Result<UDecimal32<2>, _> = ".".parse();
        assert_eq!(r, Err(ParseError::Empty));
    }

    #[test]
    fn parse_leading_dot() {
        let d: UDecimal32<4> = ".5".parse().unwrap();
        assert_eq!(d.raw(), 5000);
    }

    #[test]
    fn parse_trailing_dot() {
        let d: UDecimal32<4> = "5.".parse().unwrap();
        assert_eq!(d.raw(), 50000);
    }

    #[test]
    fn parse_scientific_notation_rejected() {
        let r: Result<UDecimal32<2>, _> = "1e5".parse();
        assert!(matches!(r, Err(ParseError::InvalidChar { .. })));
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn parse_round_trip() {
        let mut seed: u64 = 0xdeadbeef_cafebabe;
        for _ in 0..10_000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let raw = (seed >> 32) as u32;
            let d = UDecimal32::<4>::from_raw(raw);
            let s = d.to_string();
            let parsed: UDecimal32<4> = s
                .parse()
                .unwrap_or_else(|e| panic!("round-trip parse failed: raw={raw}, s={s:?}, err={e}"));
            assert_eq!(parsed, d, "round-trip mismatch: raw={raw}, s={s:?}");
        }
    }

    // ── Display (same padded format as UDecimal64) ───────────────────────────

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_basic() {
        assert_eq!(UDecimal32::<2>(123).to_string(), "1.23");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_zero_scale() {
        assert_eq!(UDecimal32::<0>(42).to_string(), "42");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_zero() {
        assert_eq!(UDecimal32::<2>(0).to_string(), "0.00");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_fractional_padding() {
        assert_eq!(UDecimal32::<4>(1234567).to_string(), "123.4567");
        assert_eq!(UDecimal32::<4>(1200).to_string(), "0.1200");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_max() {
        assert_eq!(UDecimal32::<2>::MAX.to_string(), "42949672.95");
        assert_eq!(UDecimal32::<9>::MAX.to_string(), "4.294967295");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn debug_format() {
        assert_eq!(format!("{:?}", UDecimal32::<4>(12345)), "UDecimal32<4>(12345)");
    }

    // ── f64 conversions ──────────────────────────────────────────────────────

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_nan_is_zero() {
        assert_eq!(UDecimal32::<2>::from_f64(f64::NAN).raw(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_negative_is_zero() {
        assert_eq!(UDecimal32::<2>::from_f64(-1.5).raw(), 0);
        assert_eq!(UDecimal32::<2>::from_f64(-0.0).raw(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_infinity_clamps_to_max() {
        assert_eq!(UDecimal32::<2>::from_f64(f64::INFINITY).raw(), u32::MAX);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_large_value_clamps_to_max() {
        // 1e12 fits u64 but not u32
        assert_eq!(UDecimal32::<0>::from_f64(1e12).raw(), u32::MAX);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_basic() {
        assert_eq!(UDecimal32::<4>::from_f64(1.2345).raw(), 12345);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_modes() {
        // 123.5 ± f64 noise: strictly between 123 and 124
        assert_eq!(UDecimal32::<2>::from_f64_zero(1.235).raw(), 123);
        assert_eq!(UDecimal32::<2>::from_f64_floor(1.235).raw(), 123);
        assert_eq!(UDecimal32::<2>::from_f64_ceil(1.235).raw(), 124);
        // exact tie at scale 0
        assert_eq!(UDecimal32::<0>::from_f64_nearest(2.5).raw(), 3);
        assert_eq!(UDecimal32::<0>::from_f64_nearest_even(2.5).raw(), 2);
    }

    #[test]
    fn to_f64_basic() {
        assert_eq!(UDecimal32::<4>(12345).to_f64(), 1.2345_f64);
    }

    #[cfg(feature = "std")]
    #[test]
    fn f64_round_trip_full_range() {
        let mut seed: u64 = 12345678901234567;
        for _ in 0..1000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let raw = (seed >> 32) as u32;
            let d = UDecimal32::<4>::from_raw(raw);
            let rt = UDecimal32::<4>::from_f64(d.to_f64());
            assert!(
                rt.raw().abs_diff(d.raw()) <= 1,
                "f64 round-trip failed: raw={raw}, rt={}",
                rt.raw()
            );
        }
    }

    // ── Signed/unsigned interop ──────────────────────────────────────────────

    #[test]
    fn as_signed_small_succeeds() {
        assert_eq!(UDecimal32::<4>(10_000).as_signed(), Some(Decimal32::<4>::from_raw(10_000)));
    }

    #[test]
    fn as_signed_i32_max_succeeds() {
        assert_eq!(UDecimal32::<4>(i32::MAX as u32).as_signed(), Some(Decimal32::<4>::MAX));
    }

    #[test]
    fn as_signed_above_i32_max_returns_none() {
        assert_eq!(UDecimal32::<4>(i32::MAX as u32 + 1).as_signed(), None);
    }

    #[test]
    fn as_unsigned_nonneg_succeeds() {
        assert_eq!(Decimal32::<4>::from_raw(12345).as_unsigned(), Some(UDecimal32::<4>(12345)));
        assert_eq!(Decimal32::<4>::from_raw(0).as_unsigned(), Some(UDecimal32::<4>::ZERO));
    }

    #[test]
    fn as_unsigned_negative_returns_none() {
        assert_eq!(Decimal32::<4>::from_raw(-1).as_unsigned(), None);
    }

    // ── Width interop ────────────────────────────────────────────────────────

    #[test]
    fn widen_preserves_raw() {
        assert_eq!(UDecimal32::<4>(12345).widen(), UDecimal64::<4>::from_raw(12345));
        assert_eq!(UDecimal32::<4>::MAX.widen().raw(), u32::MAX as u64);
    }

    #[test]
    fn widen_via_from() {
        let d: UDecimal64<2> = UDecimal32::<2>(100).into();
        assert_eq!(d.raw(), 100);
    }

    #[test]
    fn narrow_in_range_succeeds() {
        assert_eq!(UDecimal64::<4>::from_raw(12345).narrow(), Some(UDecimal32::<4>(12345)));
        assert_eq!(UDecimal64::<4>::from_raw(u32::MAX as u64).narrow(), Some(UDecimal32::<4>::MAX));
    }

    #[test]
    fn narrow_out_of_range_returns_none() {
        assert_eq!(UDecimal64::<4>::from_raw(u32::MAX as u64 + 1).narrow(), None);
        assert_eq!(UDecimal64::<4>::MAX.narrow(), None);
    }

    #[test]
    fn widen_narrow_round_trip() {
        let d = UDecimal32::<9>(4_000_000_000);
        assert_eq!(d.widen().narrow(), Some(d));
    }

    // ── Addition ─────────────────────────────────────────────────────────────

    #[test]
    fn add_basic() {
        assert_eq!(UDecimal32::<2>(100) + UDecimal32::<2>(50), UDecimal32::<2>(150));
    }

    #[test]
    #[should_panic(expected = "UDecimal32 addition overflow")]
    fn add_overflow_panics() {
        let _ = UDecimal32::<2>::MAX + UDecimal32::<2>(1);
    }

    #[test]
    fn checked_add_overflow_returns_none() {
        assert_eq!(UDecimal32::<2>::MAX.checked_add(UDecimal32::<2>(1)), None);
    }

    #[test]
    fn saturating_add_clamps_to_max() {
        assert_eq!(
            UDecimal32::<2>::MAX.saturating_add(UDecimal32::<2>(1)),
            UDecimal32::<2>::MAX
        );
    }

    // ── Subtraction ──────────────────────────────────────────────────────────

    #[test]
    fn sub_exact() {
        assert_eq!(UDecimal32::<2>(150) - UDecimal32::<2>(50), Some(UDecimal32::<2>(100)));
    }

    #[test]
    fn sub_underflow_returns_none() {
        assert_eq!(UDecimal32::<2>(50) - UDecimal32::<2>(100), None);
    }

    #[test]
    fn saturating_sub_underflow_clamps_to_zero() {
        assert_eq!(
            UDecimal32::<2>::ZERO.saturating_sub(UDecimal32::<2>::ONE),
            UDecimal32::<2>::ZERO
        );
    }

    #[test]
    fn checked_sub_underflow_returns_none() {
        assert_eq!(UDecimal32::<4>(5).checked_sub(UDecimal32::<4>(10)), None);
    }

    // ── Multiplication ───────────────────────────────────────────────────────

    #[test]
    fn mul_same_scale() {
        // 1.0000 × 2.0000 = 2.0000  (raw: 10_000 * 20_000 / 10_000 = 20_000)
        assert_eq!(
            UDecimal32::<4>(10_000) * UDecimal32::<4>(20_000),
            UDecimal32::<4>(20_000)
        );
    }

    #[test]
    fn mul_intermediate_exceeds_u32() {
        // 1000.00 × 1000.00 = 1000000.00; raw product 10^10 overflows u32 but not u64
        assert_eq!(
            UDecimal32::<2>(100_000) * UDecimal32::<2>(100_000),
            UDecimal32::<2>(100_000_000)
        );
    }

    #[test]
    fn mul_max_times_max_is_none() {
        // (u32::MAX)² fits u64; result at scale 0 overflows u32
        assert_eq!(UDecimal32::<0>::MAX.checked_mul(UDecimal32::<0>::MAX), None);
        // at scale 9: 4.29² ≈ 18.4 > 4.29 → None
        assert_eq!(UDecimal32::<9>::MAX.checked_mul(UDecimal32::<9>::MAX), None);
    }

    #[test]
    fn mul_max_times_one_is_max() {
        assert_eq!(UDecimal32::<9>::MAX * UDecimal32::<9>::ONE, UDecimal32::<9>::MAX);
    }

    #[test]
    fn checked_mul_overflow_returns_none() {
        assert_eq!(UDecimal32::<4>::MAX.checked_mul(UDecimal32::<4>(20_000)), None);
    }

    #[test]
    fn saturating_mul_clamps_to_max() {
        assert_eq!(
            UDecimal32::<4>::MAX.saturating_mul(UDecimal32::<4>(20_000)),
            UDecimal32::<4>::MAX
        );
    }

    // ── Division ─────────────────────────────────────────────────────────────

    #[test]
    fn div_same_scale() {
        // 3.0000 / 2.0000 = 1.5000  (raw: 30_000 * 10_000 / 20_000 = 15_000)
        assert_eq!(
            UDecimal32::<4>(30_000) / UDecimal32::<4>(20_000),
            UDecimal32::<4>(15_000)
        );
    }

    #[test]
    fn div_intermediate_exceeds_u32_at_scale_9() {
        // 4.000000000 / 1.000000000: scaled dividend 4×10^18 needs u64
        assert_eq!(
            UDecimal32::<9>(4_000_000_000) / UDecimal32::<9>::ONE,
            UDecimal32::<9>(4_000_000_000)
        );
    }

    #[test]
    fn div_truncates_toward_zero() {
        // 1.00 / 3.00 = 0.33…  raw: (100 * 100) / 300 = 33
        assert_eq!(UDecimal32::<2>(100) / UDecimal32::<2>(300), UDecimal32::<2>(33));
        // 0.10 / 0.03: raw (10 * 100) / 3 = 333
        assert_eq!(UDecimal32::<2>(10) / UDecimal32::<2>(3), UDecimal32::<2>(333));
    }

    #[test]
    fn checked_div_by_zero_returns_none() {
        assert_eq!(UDecimal32::<2>(100).checked_div(UDecimal32::<2>(0)), None);
    }

    #[test]
    fn checked_div_overflow_returns_none() {
        // MAX / 0.01 at scale 2 = MAX * 100 → overflow
        assert_eq!(UDecimal32::<2>::MAX.checked_div(UDecimal32::<2>(1)), None);
    }

    #[test]
    #[should_panic(expected = "UDecimal32 division by zero")]
    fn div_by_zero_panics() {
        let _ = UDecimal32::<2>(100) / UDecimal32::<2>(0);
    }

    #[test]
    fn div_round_ceil_and_floor() {
        // 1.00 / 3.00: 33.33… → ceil = 34, floor = 33 (same as trunc for positives)
        assert_eq!(UDecimal32::<2>(100).div_round_ceil(UDecimal32::<2>(300)), UDecimal32::<2>(34));
        assert_eq!(UDecimal32::<2>(100).div_round_floor(UDecimal32::<2>(300)), UDecimal32::<2>(33));
        assert_eq!(UDecimal32::<2>(100).div_round_zero(UDecimal32::<2>(300)), UDecimal32::<2>(33));
    }

    #[test]
    fn div_round_nearest() {
        // 1.00 / 3.00 at scale 2: 33.33… → Nearest = 33
        assert_eq!(UDecimal32::<2>(100).div_round_nearest(UDecimal32::<2>(300)), UDecimal32::<2>(33));
    }

    #[test]
    fn div_round_nearest_vs_nearest_even_on_tie() {
        // 0.01 / 0.08 at scale 2: 100 / 8 = 12.5 → Nearest = 13; NearestEven = 12
        assert_eq!(UDecimal32::<2>(1).div_round_nearest(UDecimal32::<2>(8)), UDecimal32::<2>(13));
        assert_eq!(UDecimal32::<2>(1).div_round_nearest_even(UDecimal32::<2>(8)), UDecimal32::<2>(12));
        // 0.03 / 0.08: 300 / 8 = 37.5 → NearestEven = 38 (37 is odd)
        assert_eq!(UDecimal32::<2>(3).div_round_nearest_even(UDecimal32::<2>(8)), UDecimal32::<2>(38));
    }

    #[test]
    fn div_round_exact_is_untouched_by_mode() {
        // 3 / 2 at scale 2: (300 * 100) / 200 = 150 exactly
        let (a, b) = (UDecimal32::<2>(300), UDecimal32::<2>(200));
        assert_eq!(a.div_round_nearest_even(b), UDecimal32::<2>(150));
        assert_eq!(a.div_round_nearest(b), UDecimal32::<2>(150));
        assert_eq!(a.div_round_zero(b), UDecimal32::<2>(150));
        assert_eq!(a.div_round_ceil(b), UDecimal32::<2>(150));
        assert_eq!(a.div_round_floor(b), UDecimal32::<2>(150));
    }

    #[test]
    fn checked_div_round_by_zero_returns_none() {
        let (a, z) = (UDecimal32::<2>(100), UDecimal32::<2>(0));
        assert_eq!(a.checked_div_round_nearest_even(z), None);
        assert_eq!(a.checked_div_round_nearest(z), None);
        assert_eq!(a.checked_div_round_zero(z), None);
        assert_eq!(a.checked_div_round_ceil(z), None);
        assert_eq!(a.checked_div_round_floor(z), None);
    }

    // ── Rescaling ────────────────────────────────────────────────────────────

    #[test]
    fn rescale_into_upscale() {
        // 1.23 at scale 2 → scale 6: raw 1_230_000
        let result: Option<UDecimal32<6>> = UDecimal32::<2>(123).rescale_into();
        assert_eq!(result, Some(UDecimal32::<6>(1_230_000)));
    }

    #[test]
    fn rescale_into_upscale_overflow_returns_none() {
        let result: Option<UDecimal32<1>> = UDecimal32::<0>(1_000_000_000).rescale_into();
        assert_eq!(result, None);
    }

    #[test]
    fn rescale_into_downscale_exact() {
        // 1.20 (raw 120 at scale 2) → scale 1: raw 12
        let result: Option<UDecimal32<1>> = UDecimal32::<2>(120).rescale_into();
        assert_eq!(result, Some(UDecimal32::<1>(12)));
    }

    #[test]
    fn rescale_into_downscale_lossy_returns_none() {
        // 1.23 cannot be represented exactly at scale 1
        let result: Option<UDecimal32<1>> = UDecimal32::<2>(123).rescale_into();
        assert_eq!(result, None);
    }

    #[test]
    fn rescale_same_scale_is_identity() {
        let d = UDecimal32::<4>(12345);
        let result: Option<UDecimal32<4>> = d.rescale_into();
        assert_eq!(result, Some(d));
    }

    #[test]
    fn rescale_round_into_downscale() {
        // 1.25 at scale 2 → scale 1: Nearest 1.3 (raw 13); NearestEven 1.2 (raw 12)
        assert_eq!(UDecimal32::<2>(125).rescale_round_into_nearest::<1>(), Some(UDecimal32::<1>(13)));
        assert_eq!(UDecimal32::<2>(125).rescale_round_into_nearest_even::<1>(), Some(UDecimal32::<1>(12)));
        // 1.23 → truncate 1.2, ceil 1.3, floor 1.2
        assert_eq!(UDecimal32::<2>(123).rescale_round_into_zero::<1>(), Some(UDecimal32::<1>(12)));
        assert_eq!(UDecimal32::<2>(123).rescale_round_into_ceil::<1>(), Some(UDecimal32::<1>(13)));
        assert_eq!(UDecimal32::<2>(123).rescale_round_into_floor::<1>(), Some(UDecimal32::<1>(12)));
    }

    #[test]
    fn rescale_round_into_max_to_scale_0() {
        // MAX at scale 9 = 4.294967295 → scale 0 with ceil: 5
        assert_eq!(UDecimal32::<9>::MAX.rescale_round_into_ceil::<0>(), Some(UDecimal32::<0>(5)));
    }

    // ── Ordering ─────────────────────────────────────────────────────────────

    #[test]
    fn ordering_is_numeric() {
        assert!(UDecimal32::<4>(100) < UDecimal32::<4>(200));
        assert!(UDecimal32::<4>(0) < UDecimal32::<4>(1));
        assert_eq!(UDecimal32::<4>(50), UDecimal32::<4>(50));
    }

    // ── Cross-check against UDecimal64 ───────────────────────────────────────

    #[test]
    fn arithmetic_agrees_with_udecimal64_when_in_range() {
        let mut seed: u64 = 0x0f1e_2d3c_4b5a_6978;
        for _ in 0..5_000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let a = (seed >> 32) as u32;
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let b = ((seed >> 40) as u32) | 1;

            let a32 = UDecimal32::<4>::from_raw(a);
            let b32 = UDecimal32::<4>::from_raw(b);
            let a64 = a32.widen();
            let b64 = b32.widen();

            assert_eq!(a32.checked_mul(b32), a64.checked_mul(b64).and_then(UDecimal64::narrow));
            assert_eq!(a32.checked_div(b32), a64.checked_div(b64).and_then(UDecimal64::narrow));
            assert_eq!(a32.checked_sub(b32), a64.checked_sub(b64).and_then(UDecimal64::narrow));

            assert_eq!(
                a32.checked_div_round_nearest_even(b32),
                a64.checked_div_round_nearest_even(b64).and_then(UDecimal64::narrow)
            );
            assert_eq!(
                a32.checked_div_round_nearest(b32),
                a64.checked_div_round_nearest(b64).and_then(UDecimal64::narrow)
            );
            assert_eq!(
                a32.checked_div_round_zero(b32),
                a64.checked_div_round_zero(b64).and_then(UDecimal64::narrow)
            );
            assert_eq!(
                a32.checked_div_round_ceil(b32),
                a64.checked_div_round_ceil(b64).and_then(UDecimal64::narrow)
            );
            assert_eq!(
                a32.checked_div_round_floor(b32),
                a64.checked_div_round_floor(b64).and_then(UDecimal64::narrow)
            );

            assert_eq!(
                a32.rescale_round_into_nearest_even::<1>(),
                a64.rescale_round_into_nearest_even::<1>().and_then(UDecimal64::narrow)
            );
            assert_eq!(
                a32.rescale_round_into_nearest::<1>(),
                a64.rescale_round_into_nearest::<1>().and_then(UDecimal64::narrow)
            );
            assert_eq!(
                a32.rescale_round_into_ceil::<1>(),
                a64.rescale_round_into_ceil::<1>().and_then(UDecimal64::narrow)
            );
            assert_eq!(a32.rescale_into::<1>(), a64.rescale_into::<1>().and_then(UDecimal64::narrow));

            #[cfg(any(feature = "std", feature = "alloc"))]
            assert_eq!(a32.to_string(), a64.to_string());
        }
    }
}
