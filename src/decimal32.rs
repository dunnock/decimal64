use core::fmt::{self, Write};
use core::ops::{Add, Div, Mul, Neg, Sub};
use core::str::FromStr;

use crate::{RoundFlag, RoundFlagEnum};

#[inline(always)]
const fn const_pow10_i32(s: u32) -> i32 {
    assert!(s <= 9, "Decimal32 scale must be <= 9");
    let mut result: i32 = 1;
    let mut i = 0u32;
    while i < s {
        result *= 10;
        i += 1;
    }
    result
}

/// Fixed-scale 32-bit signed decimal.
///
/// The raw value is an `i32` whose unit is `10^(-S)`.
/// Scale `S` is a compile-time const; no runtime overhead.
///
/// This is the half-width sibling of [`crate::Decimal64`]: same API and semantics,
/// half the storage, and every intermediate fits in native `i64` (no 128-bit path).
/// Use it for dense storage of small-magnitude quantities.
///
/// # Representation
///
/// `"1.23"` at scale 2 is stored as `123i32`; `"1.2345"` at scale 4 is `12345i32`.
///
/// # Scale limit
///
/// `S` must be ≤ 9; larger values overflow `ONE = 10^S` and are rejected at compile time.
///
/// # Range
///
/// `|value| ≤ 2147483647 / 10^S` — e.g. `±21474836.47` at scale 2, `±2.147483647` at scale 9.
///
/// ```rust
/// use scaled_int::Decimal32;
///
/// let price: Decimal32<4> = "123.4567".parse().unwrap();
/// let qty: Decimal32<4> = "10.0000".parse().unwrap();
/// assert_eq!((price * qty).to_string(), "1234.567");
/// assert_eq!(core::mem::size_of::<Decimal32<4>>(), 4);
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Decimal32<const S: u32>(i32);

impl<const S: u32> Decimal32<S> {
    /// The scale parameter `S`.
    pub const SCALE: u32 = S;
    /// Additive identity: `0`.
    pub const ZERO: Self = Self(0);
    /// Multiplicative identity: `1.0` stored as `10^S`.
    pub const ONE: Self = Self(const_pow10_i32(S));
    /// Largest representable value (`i32::MAX` raw).
    pub const MAX: Self = Self(i32::MAX);
    /// Smallest representable value (`i32::MIN` raw).
    pub const MIN: Self = Self(i32::MIN);

    /// Wrap a raw `i32` without any scaling — caller manages the invariant.
    #[inline(always)]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// Return the raw `i32` storage value (the mathematical value × `10^S`).
    #[inline(always)]
    pub const fn raw(self) -> i32 {
        self.0
    }
}

// ── Parse ────────────────────────────────────────────────────────────────────

impl<const S: u32> Decimal32<S> {
    /// Parse a decimal string. Extra fractional digits beyond `S` are silently truncated.
    ///
    /// Equivalent to `s.parse::<Decimal32<S>>()`.
    #[inline]
    pub fn parse(s: &str) -> Result<Self, crate::ParseError> {
        Self::from_slice(s.as_bytes())
    }

    /// Parse decimal bytes. Extra fractional digits beyond `S` are silently truncated.
    #[inline]
    pub fn from_slice(bytes: &[u8]) -> Result<Self, crate::ParseError> {
        crate::parse32::parse_slice::<S>(bytes)
    }
}

impl<const S: u32> FromStr for Decimal32<S> {
    type Err = crate::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_slice(s.as_bytes())
    }
}

// ── f64 conversions ──────────────────────────────────────────────────────────

impl<const S: u32> Decimal32<S> {
    /// Convert from `f64` using nearest-even (banker's) rounding.
    ///
    /// `NaN` maps to `ZERO`; overflow clamps to `MAX`/`MIN`.
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
        if x.is_nan() {
            return Self::ZERO;
        }
        let scaled = x * (const_pow10_i32(S) as f64);
        let rounded = match RoundFlag::from_u8(MODE) {
            RoundFlag::NearestEven => scaled.round_ties_even(),
            RoundFlag::Nearest => scaled.round(),
            RoundFlag::Zero => scaled.trunc(),
            RoundFlag::Ceil => scaled.ceil(),
            RoundFlag::Floor => scaled.floor(),
        };
        // i32 bounds are exactly representable in f64, so the clamp is exact.
        let clamped = rounded.clamp(i32::MIN as f64, i32::MAX as f64);
        Self(clamped as i32)
    }

    /// Convert to `f64`. Every `i32` is exactly representable in `f64`, so the only
    /// rounding is the final division by `10^S`.
    #[inline]
    pub fn to_f64(self) -> f64 {
        (self.0 as f64) / (const_pow10_i32(S) as f64)
    }
}

// ── Width interop ────────────────────────────────────────────────────────────

impl<const S: u32> Decimal32<S> {
    /// Widen to [`crate::Decimal64<S>`]. Always lossless.
    #[inline(always)]
    pub const fn widen(self) -> crate::Decimal64<S> {
        crate::Decimal64::from_raw(self.0 as i64)
    }
}

impl<const S: u32> From<Decimal32<S>> for crate::Decimal64<S> {
    #[inline(always)]
    fn from(d: Decimal32<S>) -> Self {
        d.widen()
    }
}

/// Extension on `Decimal64<S>` to convert to the half-width counterpart.
impl<const S: u32> crate::Decimal64<S> {
    /// Narrow to [`Decimal32<S>`]. Returns `None` when the raw value is outside `i32` range.
    ///
    /// ```rust
    /// use scaled_int::{Decimal32, Decimal64};
    ///
    /// let d: Decimal64<2> = "123.45".parse().unwrap();
    /// assert_eq!(d.narrow(), Some(Decimal32::<2>::from_raw(12345)));
    /// assert_eq!(Decimal64::<2>::MAX.narrow(), None);
    /// ```
    ///
    /// Rejected at compile time when `S > 9` (the `Decimal32` scale limit):
    ///
    /// ```compile_fail
    /// use scaled_int::Decimal64;
    /// let _ = Decimal64::<12>::from_raw(1).narrow();
    /// ```
    pub fn narrow(self) -> Option<Decimal32<S>> {
        let _ = Decimal32::<S>::ONE; // force the S <= 9 assertion at monomorphisation
        i32::try_from(self.raw()).ok().map(Decimal32::from_raw)
    }
}

// ── Display / Debug ──────────────────────────────────────────────────────────

/// Same format as `Decimal64`: integer part, then a `.` and the fraction with trailing
/// zeros trimmed (`"1.5"`, `"-0.25"`, `"42"`).
impl<const S: u32> fmt::Display for Decimal32<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scale = const_pow10_i32(S);
        let int_part = self.0 / scale;
        if self.0 < 0 && int_part == 0 {
            f.write_char('-')?;
        }

        let mut buffer = itoa::Buffer::new();
        f.write_str(buffer.format(int_part))?;

        let mut frac = self.0.unsigned_abs() % scale as u32;
        if frac > 0 {
            f.write_char('.')?;
        }
        let mut divisor = scale as u32;
        while frac > 0 {
            divisor /= 10;
            let digit = b'0' + (frac / divisor) as u8;
            frac %= divisor;
            f.write_char(digit as char)?;
        }

        Ok(())
    }
}

impl<const S: u32> fmt::Debug for Decimal32<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Decimal32<{}>({})", S, self.0)
    }
}

// ── Arithmetic trait impls ───────────────────────────────────────────────────

impl<const S: u32> Add for Decimal32<S> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(
            self.0
                .checked_add(rhs.0)
                .expect("Decimal32 addition overflow"),
        )
    }
}

impl<const S: u32> Sub for Decimal32<S> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(
            self.0
                .checked_sub(rhs.0)
                .expect("Decimal32 subtraction overflow"),
        )
    }
}

impl<const S: u32> Neg for Decimal32<S> {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self(self.0.checked_neg().expect("Decimal32 negation overflow"))
    }
}

impl<const S: u32> Mul for Decimal32<S> {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        self.checked_mul(rhs)
            .expect("Decimal32 multiplication overflow")
    }
}

impl<const S: u32> Div for Decimal32<S> {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        self.checked_div(rhs).expect("Decimal32 division by zero")
    }
}

// ── Checked / saturating / rounding variants ─────────────────────────────────
//
// All intermediates are `i64` and cannot overflow: |i32|² < 2^62 and |i32| × 10^9 < 2^61.
// Unlike `Decimal64` there is no fast/slow path split — the single path is the fast path,
// and the only range check is `i32::try_from` on the final result.

impl<const S: u32> Decimal32<S> {
    /// Returns `None` on overflow.
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    /// Returns `None` on overflow.
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    /// Returns `None` on overflow.
    #[inline(always)]
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let product = self.0 as i64 * rhs.0 as i64;
        let result = product / const_pow10_i32(S) as i64;
        Some(Self(result.try_into().ok()?))
    }

    /// Returns `None` on division by zero or overflow.
    #[inline(always)]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.0 == 0 {
            return None;
        }
        let num = self.0 as i64 * const_pow10_i32(S) as i64;
        let result = num / rhs.0 as i64;
        Some(Self(result.try_into().ok()?))
    }

    /// Clamps to `MAX`/`MIN` on overflow instead of panicking.
    #[inline]
    pub fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Clamps to `MAX`/`MIN` on overflow instead of panicking.
    #[inline]
    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Clamps to `MAX`/`MIN` on overflow instead of panicking.
    #[inline]
    pub fn saturating_mul(self, rhs: Self) -> Self {
        let product = self.0 as i64 * rhs.0 as i64;
        let result = product / const_pow10_i32(S) as i64;
        Self(result.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
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
            .expect("Decimal32 div_round: division by zero or overflow")
    }

    #[inline]
    fn checked_div_round_impl<const MODE: RoundFlagEnum>(self, rhs: Self) -> Option<Self> {
        if rhs.0 == 0 {
            return None;
        }
        let num = self.0 as i64 * const_pow10_i32(S) as i64;
        let result = div_round_i64::<MODE>(num, rhs.0 as i64);
        Some(Self(result.try_into().ok()?))
    }

    /// Lossless rescale. Returns `None` if fractional digits would be lost or on overflow.
    pub fn rescale_into<const OUT: u32>(self) -> Option<Decimal32<OUT>> {
        if OUT > S {
            let factor = const_pow10_i32(OUT - S);
            self.0.checked_mul(factor).map(Decimal32::from_raw)
        } else if OUT < S {
            let factor = const_pow10_i32(S - OUT);
            if self.0 % factor != 0 {
                None
            } else {
                Some(Decimal32::from_raw(self.0 / factor))
            }
        } else {
            Some(Decimal32::from_raw(self.0))
        }
    }

    /// Rescale using nearest-even (banker's) rounding. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_nearest_even<const OUT: u32>(self) -> Option<Decimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::NEAREST_EVEN }>()
    }

    /// Rescale using nearest, ties away from zero. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_nearest<const OUT: u32>(self) -> Option<Decimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::NEAREST }>()
    }

    /// Rescale by truncating toward zero. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_zero<const OUT: u32>(self) -> Option<Decimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::ZERO }>()
    }

    /// Rescale by rounding toward positive infinity. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_ceil<const OUT: u32>(self) -> Option<Decimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::CEIL }>()
    }

    /// Rescale by rounding toward negative infinity. Returns `None` only on overflow.
    #[inline]
    pub fn rescale_round_into_floor<const OUT: u32>(self) -> Option<Decimal32<OUT>> {
        self.rescale_round_into_impl::<OUT, { RoundFlag::FLOOR }>()
    }

    #[inline]
    fn rescale_round_into_impl<const OUT: u32, const MODE: RoundFlagEnum>(
        self,
    ) -> Option<Decimal32<OUT>> {
        if OUT > S {
            let factor = const_pow10_i32(OUT - S);
            self.0.checked_mul(factor).map(Decimal32::from_raw)
        } else if OUT < S {
            let factor = const_pow10_i32(S - OUT) as i64;
            let result = div_round_i64::<MODE>(self.0 as i64, factor);
            i32::try_from(result).ok().map(Decimal32::from_raw)
        } else {
            Some(Decimal32::from_raw(self.0))
        }
    }
}

/// Integer division with rounding. `den` must be non-zero.
/// Uses truncating division as the base; applies `MODE` to adjust.
///
/// Same algorithm as `decimal64::div_round_i128`, specialised to `i64`.
fn div_round_i64<const MODE: RoundFlagEnum>(num: i64, den: i64) -> i64 {
    debug_assert!(den != 0);
    let q = num / den;
    let r = num % den; // same sign as num (Rust truncates toward zero)

    if r == 0 {
        return q;
    }

    match RoundFlag::from_u8(MODE) {
        RoundFlag::Zero => q,
        RoundFlag::Ceil => {
            // ceil: add 1 when the fractional part is positive (r and den same sign)
            if (r > 0) == (den > 0) { q + 1 } else { q }
        }
        RoundFlag::Floor => {
            // floor: subtract 1 when the fractional part is negative (r and den opposite sign)
            if (r > 0) != (den > 0) { q - 1 } else { q }
        }
        RoundFlag::Nearest => {
            // half away from zero
            let abs_2r = r.unsigned_abs().saturating_mul(2);
            let abs_d = den.unsigned_abs();
            if abs_2r >= abs_d {
                if (r > 0) == (den > 0) { q + 1 } else { q - 1 }
            } else {
                q
            }
        }
        RoundFlag::NearestEven => {
            // banker's rounding
            let abs_2r = r.unsigned_abs().saturating_mul(2);
            let abs_d = den.unsigned_abs();
            if abs_2r > abs_d {
                if (r > 0) == (den > 0) { q + 1 } else { q - 1 }
            } else if abs_2r == abs_d {
                if q % 2 != 0 {
                    if (r > 0) == (den > 0) { q + 1 } else { q - 1 }
                } else {
                    q
                }
            } else {
                q
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Decimal64;
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::format;
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::string::ToString;

    // ── Constants / layout ───────────────────────────────────────────────────

    #[test]
    fn one_raw_equals_pow10() {
        assert_eq!(Decimal32::<4>::ONE.raw(), 10_000);
    }

    #[test]
    fn one_at_max_scale() {
        assert_eq!(Decimal32::<9>::ONE.raw(), 1_000_000_000);
    }

    #[test]
    fn max_min_raw() {
        assert_eq!(Decimal32::<4>::MAX.raw(), i32::MAX);
        assert_eq!(Decimal32::<4>::MIN.raw(), i32::MIN);
    }

    #[test]
    fn size_is_four_bytes() {
        assert_eq!(core::mem::size_of::<Decimal32<4>>(), 4);
        assert_eq!(core::mem::size_of::<Option<Decimal32<4>>>(), 8);
    }

    #[test]
    fn negative_less_than_zero() {
        assert!(Decimal32::<2>(-100) < Decimal32::<2>(0));
    }

    // ── Display (same trimming format as Decimal64) ──────────────────────────

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_basic() {
        assert_eq!(Decimal32::<2>(123).to_string(), "1.23");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_zero_scale() {
        assert_eq!(Decimal32::<0>(42).to_string(), "42");
        assert_eq!(Decimal32::<0>(-42).to_string(), "-42");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_zero() {
        assert_eq!(Decimal32::<2>(0).to_string(), "0");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_negative() {
        assert_eq!(Decimal32::<2>(-100).to_string(), "-1");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_negative_fraction_less_than_one() {
        assert_eq!(Decimal32::<4>(-5000).to_string(), "-0.5");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_truncates_trailing_zeros() {
        assert_eq!(Decimal32::<4>(1200).to_string(), "0.12");
        assert_eq!(Decimal32::<4>(1020).to_string(), "0.102");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_fractional_padding() {
        assert_eq!(Decimal32::<4>(1234567).to_string(), "123.4567");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_min_does_not_overflow() {
        // unsigned_abs() handles i32::MIN correctly
        assert_eq!(Decimal32::<2>::MIN.to_string(), "-21474836.48");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn display_max_scale() {
        assert_eq!(Decimal32::<9>::MAX.to_string(), "2.147483647");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn debug_format() {
        assert_eq!(format!("{:?}", Decimal32::<4>(12345)), "Decimal32<4>(12345)");
    }

    // ── f64 conversions ──────────────────────────────────────────────────────

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_nearest_even_1_005() {
        // 1.005 in f64 is actually ~1.00499999..., so 1.005 * 100 < 100.5
        assert_eq!(Decimal32::<2>::from_f64(1.005).raw(), 100);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_rounded_four_scale() {
        // 1.23456789 * 10000 = 12345.6789 → rounds to 12346
        assert_eq!(Decimal32::<4>::from_f64(1.23456789).raw(), 12346);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_nan_is_zero() {
        assert_eq!(Decimal32::<2>::from_f64(f64::NAN).raw(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_infinity_clamps() {
        assert_eq!(Decimal32::<2>::from_f64(f64::INFINITY).raw(), i32::MAX);
        assert_eq!(Decimal32::<2>::from_f64(f64::NEG_INFINITY).raw(), i32::MIN);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_large_value_clamps() {
        // 1e12 fits i64 but not i32
        assert_eq!(Decimal32::<0>::from_f64(1e12).raw(), i32::MAX);
        assert_eq!(Decimal32::<0>::from_f64(-1e12).raw(), i32::MIN);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_f64_modes() {
        // -123.5 ± f64 noise: strictly between -124 and -123
        assert_eq!(Decimal32::<2>::from_f64_zero(-1.235).raw(), -123);
        assert_eq!(Decimal32::<2>::from_f64_ceil(-1.235).raw(), -123);
        assert_eq!(Decimal32::<2>::from_f64_floor(-1.235).raw(), -124);
        // exact ties at scale 0
        assert_eq!(Decimal32::<0>::from_f64_nearest(2.5).raw(), 3);
        assert_eq!(Decimal32::<0>::from_f64_nearest_even(2.5).raw(), 2);
        assert_eq!(Decimal32::<0>::from_f64_nearest(-2.5).raw(), -3);
        assert_eq!(Decimal32::<0>::from_f64_nearest_even(-2.5).raw(), -2);
    }

    #[test]
    fn to_f64_basic() {
        assert_eq!(Decimal32::<4>(12345).to_f64(), 1.2345_f64);
    }

    #[cfg(feature = "std")]
    #[test]
    fn f64_round_trip_full_range() {
        // Every i32 is exact in f64; the only rounding is the final division, so the
        // round trip stays within one raw unit across the whole i32 range.
        let mut seed: u64 = 12345678901234567;
        for _ in 0..1000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let raw = (seed >> 32) as i32;
            let d = Decimal32::<4>::from_raw(raw);
            let rt = Decimal32::<4>::from_f64(d.to_f64());
            assert!(
                (rt.raw() as i64 - d.raw() as i64).abs() <= 1,
                "round-trip failed: raw={}, rt={}",
                raw,
                rt.raw()
            );
        }
    }

    // ── Width interop ────────────────────────────────────────────────────────

    #[test]
    fn widen_preserves_raw() {
        assert_eq!(Decimal32::<4>(12345).widen(), Decimal64::<4>::from_raw(12345));
        assert_eq!(Decimal32::<4>(-12345).widen(), Decimal64::<4>::from_raw(-12345));
        assert_eq!(Decimal32::<4>::MIN.widen().raw(), i32::MIN as i64);
    }

    #[test]
    fn widen_via_from() {
        let d: Decimal64<2> = Decimal32::<2>(-100).into();
        assert_eq!(d.raw(), -100);
    }

    #[test]
    fn narrow_in_range_succeeds() {
        assert_eq!(Decimal64::<4>::from_raw(12345).narrow(), Some(Decimal32::<4>(12345)));
        assert_eq!(Decimal64::<4>::from_raw(-12345).narrow(), Some(Decimal32::<4>(-12345)));
        assert_eq!(Decimal64::<4>::from_raw(i32::MAX as i64).narrow(), Some(Decimal32::<4>::MAX));
        assert_eq!(Decimal64::<4>::from_raw(i32::MIN as i64).narrow(), Some(Decimal32::<4>::MIN));
    }

    #[test]
    fn narrow_out_of_range_returns_none() {
        assert_eq!(Decimal64::<4>::from_raw(i32::MAX as i64 + 1).narrow(), None);
        assert_eq!(Decimal64::<4>::from_raw(i32::MIN as i64 - 1).narrow(), None);
        assert_eq!(Decimal64::<4>::MAX.narrow(), None);
    }

    #[test]
    fn widen_narrow_round_trip() {
        let d = Decimal32::<9>(-1_234_567_890);
        assert_eq!(d.widen().narrow(), Some(d));
    }

    // ── Addition / subtraction / negation ────────────────────────────────────

    #[test]
    fn add_basic() {
        assert_eq!(Decimal32::<2>(100) + Decimal32::<2>(50), Decimal32::<2>(150));
    }

    #[test]
    #[should_panic(expected = "Decimal32 addition overflow")]
    fn add_overflow_panics() {
        let _ = Decimal32::<2>::MAX + Decimal32::<2>(1);
    }

    #[test]
    fn sub_basic() {
        assert_eq!(Decimal32::<2>(100) - Decimal32::<2>(150), Decimal32::<2>(-50));
    }

    #[test]
    fn neg_basic() {
        assert_eq!(-Decimal32::<2>(100), Decimal32::<2>(-100));
    }

    #[test]
    #[should_panic]
    fn neg_min_panics() {
        let _ = -Decimal32::<2>::MIN;
    }

    #[test]
    fn checked_add_overflow_returns_none() {
        assert_eq!(Decimal32::<2>::MAX.checked_add(Decimal32::<2>(1)), None);
    }

    #[test]
    fn checked_sub_overflow_returns_none() {
        assert_eq!(Decimal32::<2>::MIN.checked_sub(Decimal32::<2>(1)), None);
    }

    #[test]
    fn saturating_add_clamps_to_max() {
        assert_eq!(
            Decimal32::<2>::MAX.saturating_add(Decimal32::<2>(1)),
            Decimal32::<2>::MAX
        );
    }

    #[test]
    fn saturating_sub_clamps_to_min() {
        assert_eq!(
            Decimal32::<2>::MIN.saturating_sub(Decimal32::<2>(1)),
            Decimal32::<2>::MIN
        );
    }

    // ── Multiplication ───────────────────────────────────────────────────────

    #[test]
    fn mul_same_scale() {
        // 1.0000 × 2.0000 = 2.0000  (raw: 10_000 * 20_000 / 10_000 = 20_000)
        assert_eq!(
            Decimal32::<4>(10_000) * Decimal32::<4>(20_000),
            Decimal32::<4>(20_000)
        );
    }

    #[test]
    fn mul_intermediate_exceeds_i32() {
        // 1000.00 × 1000.00 = 1000000.00; raw product 10^10 overflows i32 but not i64
        assert_eq!(
            Decimal32::<2>(100_000) * Decimal32::<2>(100_000),
            Decimal32::<2>(100_000_000)
        );
    }

    #[test]
    fn mul_max_times_max_at_scale_9_is_none() {
        // 2.147483647² ≈ 4.61 > 2.147483647 → overflow; the i64 product itself is fine
        assert_eq!(Decimal32::<9>::MAX.checked_mul(Decimal32::<9>::MAX), None);
    }

    #[test]
    fn mul_min_times_min_is_none() {
        // (-2^31)² = 2^62 fits i64; result at scale 0 still overflows i32
        assert_eq!(Decimal32::<0>::MIN.checked_mul(Decimal32::<0>::MIN), None);
    }

    #[test]
    fn mul_negative() {
        // -1.50 × 2.00 = -3.00
        assert_eq!(Decimal32::<2>(-150) * Decimal32::<2>(200), Decimal32::<2>(-300));
    }

    #[test]
    fn mul_truncates_toward_zero() {
        // 0.05 × 0.05 = 0.0025 → truncates to 0.00 for both signs
        assert_eq!(Decimal32::<2>(5) * Decimal32::<2>(5), Decimal32::<2>(0));
        assert_eq!(Decimal32::<2>(-5) * Decimal32::<2>(5), Decimal32::<2>(0));
    }

    #[test]
    fn mul_checked_overflow_returns_none() {
        assert_eq!(Decimal32::<4>::MAX.checked_mul(Decimal32::<4>(20_000)), None);
    }

    #[test]
    fn saturating_mul_clamps() {
        assert_eq!(
            Decimal32::<4>::MAX.saturating_mul(Decimal32::<4>(20_000)),
            Decimal32::<4>::MAX
        );
        assert_eq!(
            Decimal32::<4>::MAX.saturating_mul(Decimal32::<4>(-20_000)),
            Decimal32::<4>::MIN
        );
    }

    // ── Division ─────────────────────────────────────────────────────────────

    #[test]
    fn div_same_scale() {
        // 3.0000 / 2.0000 = 1.5000  (raw: 30_000 * 10_000 / 20_000 = 15_000)
        assert_eq!(
            Decimal32::<4>(30_000) / Decimal32::<4>(20_000),
            Decimal32::<4>(15_000)
        );
    }

    #[test]
    fn div_intermediate_exceeds_i32_at_scale_9() {
        // 1.000000000 / 0.500000000 = 2.000000000; scaled dividend 10^18 needs i64
        assert_eq!(
            Decimal32::<9>(1_000_000_000) / Decimal32::<9>(500_000_000),
            Decimal32::<9>(2_000_000_000)
        );
    }

    #[test]
    fn div_truncates_toward_zero() {
        // 0.10 / 0.03 = 3.333…  raw: (10 * 100) / 3 = 333
        assert_eq!(Decimal32::<2>(10) / Decimal32::<2>(3), Decimal32::<2>(333));
    }

    #[test]
    fn div_truncates_negative_toward_zero() {
        // -0.10 / 0.03 = -3.333…  raw: (-10 * 100) / 3 = -333
        assert_eq!(Decimal32::<2>(-10) / Decimal32::<2>(3), Decimal32::<2>(-333));
    }

    #[test]
    fn checked_div_by_zero_returns_none() {
        assert_eq!(Decimal32::<2>(100).checked_div(Decimal32::<2>(0)), None);
    }

    #[test]
    fn checked_div_overflow_returns_none() {
        // MAX / 0.01 at scale 2 = MAX * 100 → overflow
        assert_eq!(Decimal32::<2>::MAX.checked_div(Decimal32::<2>(1)), None);
    }

    #[test]
    #[should_panic(expected = "Decimal32 division by zero")]
    fn div_by_zero_panics() {
        let _ = Decimal32::<2>(100) / Decimal32::<2>(0);
    }

    #[test]
    fn div_round_nearest() {
        // 1.0 / 3.0 at scale 2: 33.33… → Nearest = 33
        assert_eq!(
            Decimal32::<2>(100).div_round_nearest(Decimal32::<2>(300)),
            Decimal32::<2>(33)
        );
    }

    #[test]
    fn div_round_nearest_vs_nearest_even_on_tie() {
        // 0.01 / 0.08 at scale 2: (1 * 100) / 8 = 12.5 → Nearest = 13; NearestEven = 12
        assert_eq!(Decimal32::<2>(1).div_round_nearest(Decimal32::<2>(8)), Decimal32::<2>(13));
        assert_eq!(Decimal32::<2>(1).div_round_nearest_even(Decimal32::<2>(8)), Decimal32::<2>(12));
        assert_eq!(Decimal32::<2>(-1).div_round_nearest(Decimal32::<2>(8)), Decimal32::<2>(-13));
        assert_eq!(Decimal32::<2>(-1).div_round_nearest_even(Decimal32::<2>(8)), Decimal32::<2>(-12));
        // 0.03 / 0.08: 300 / 8 = 37.5 → NearestEven = 38 (37 is odd)
        assert_eq!(Decimal32::<2>(3).div_round_nearest_even(Decimal32::<2>(8)), Decimal32::<2>(38));
    }

    #[test]
    fn div_round_ceil() {
        // 1.0 / 3.0 at scale 2: 33.33… → ceil = 34
        assert_eq!(Decimal32::<2>(100).div_round_ceil(Decimal32::<2>(300)), Decimal32::<2>(34));
        // -1.0 / 3.0: -33.33… → ceil = -33
        assert_eq!(Decimal32::<2>(-100).div_round_ceil(Decimal32::<2>(300)), Decimal32::<2>(-33));
    }

    #[test]
    fn div_round_floor() {
        // -1.0 / 3.0 at scale 2: -33.33… → floor = -34
        assert_eq!(Decimal32::<2>(-100).div_round_floor(Decimal32::<2>(300)), Decimal32::<2>(-34));
        // 1.0 / -3.0: -33.33… → floor = -34 (negative divisor path)
        assert_eq!(Decimal32::<2>(100).div_round_floor(Decimal32::<2>(-300)), Decimal32::<2>(-34));
    }

    #[test]
    fn div_round_zero_matches_div() {
        assert_eq!(
            Decimal32::<2>(-100).div_round_zero(Decimal32::<2>(300)),
            Decimal32::<2>(-100) / Decimal32::<2>(300)
        );
    }

    #[test]
    fn div_round_exact_is_untouched_by_mode() {
        // 0.05 / 0.10 at scale 2: (5 * 100) / 10 = 50 exactly
        let (a, b) = (Decimal32::<2>(5), Decimal32::<2>(10));
        assert_eq!(a.div_round_nearest_even(b), Decimal32::<2>(50));
        assert_eq!(a.div_round_nearest(b), Decimal32::<2>(50));
        assert_eq!(a.div_round_zero(b), Decimal32::<2>(50));
        assert_eq!(a.div_round_ceil(b), Decimal32::<2>(50));
        assert_eq!(a.div_round_floor(b), Decimal32::<2>(50));
    }

    #[test]
    fn checked_div_round_by_zero_returns_none() {
        let (a, z) = (Decimal32::<2>(100), Decimal32::<2>(0));
        assert_eq!(a.checked_div_round_nearest_even(z), None);
        assert_eq!(a.checked_div_round_nearest(z), None);
        assert_eq!(a.checked_div_round_zero(z), None);
        assert_eq!(a.checked_div_round_ceil(z), None);
        assert_eq!(a.checked_div_round_floor(z), None);
    }

    #[test]
    fn checked_div_round_overflow_returns_none() {
        assert_eq!(Decimal32::<2>::MAX.checked_div_round_nearest(Decimal32::<2>(1)), None);
    }

    // ── Rescaling ────────────────────────────────────────────────────────────

    #[test]
    fn rescale_into_upscale() {
        // 1.23 at scale 2 → scale 6: raw 1_230_000
        let result: Option<Decimal32<6>> = Decimal32::<2>(123).rescale_into();
        assert_eq!(result, Some(Decimal32::<6>(1_230_000)));
    }

    #[test]
    fn rescale_into_upscale_overflow_returns_none() {
        // 2_000_000_000 at scale 0 → scale 1 needs 2×10^10, overflow
        let result: Option<Decimal32<1>> = Decimal32::<0>(2_000_000_000).rescale_into();
        assert_eq!(result, None);
    }

    #[test]
    fn rescale_into_downscale_lossy_returns_none() {
        // 1.23 cannot be represented exactly at scale 1
        let result: Option<Decimal32<1>> = Decimal32::<2>(123).rescale_into();
        assert_eq!(result, None);
    }

    #[test]
    fn rescale_into_downscale_exact() {
        // 1.20 (raw 120 at scale 2) → scale 1: raw 12 = 1.2 (exact)
        let result: Option<Decimal32<1>> = Decimal32::<2>(120).rescale_into();
        assert_eq!(result, Some(Decimal32::<1>(12)));
        let result: Option<Decimal32<1>> = Decimal32::<2>(-120).rescale_into();
        assert_eq!(result, Some(Decimal32::<1>(-12)));
    }

    #[test]
    fn rescale_round_into_downscale() {
        // 1.23 at scale 2 → scale 1 with Nearest: 1.2 (raw 12)
        assert_eq!(
            Decimal32::<2>(123).rescale_round_into_nearest::<1>(),
            Some(Decimal32::<1>(12))
        );
        // 1.25 → 1.3 (raw 13)
        assert_eq!(
            Decimal32::<2>(125).rescale_round_into_nearest::<1>(),
            Some(Decimal32::<1>(13))
        );
        // 1.25 → NearestEven → 1.2 (12 is even)
        assert_eq!(
            Decimal32::<2>(125).rescale_round_into_nearest_even::<1>(),
            Some(Decimal32::<1>(12))
        );
        // truncate / ceil / floor on 1.23
        assert_eq!(Decimal32::<2>(123).rescale_round_into_zero::<1>(), Some(Decimal32::<1>(12)));
        assert_eq!(Decimal32::<2>(123).rescale_round_into_ceil::<1>(), Some(Decimal32::<1>(13)));
        assert_eq!(Decimal32::<2>(-123).rescale_round_into_floor::<1>(), Some(Decimal32::<1>(-13)));
    }

    #[test]
    fn rescale_round_into_min_to_scale_0() {
        // MIN at scale 9 → scale 0 with floor: -2.147483648 → -3
        assert_eq!(
            Decimal32::<9>::MIN.rescale_round_into_floor::<0>(),
            Some(Decimal32::<0>(-3))
        );
    }

    #[test]
    fn rescale_round_into_upscale_overflow_returns_none() {
        assert_eq!(Decimal32::<0>(2_000_000_000).rescale_round_into_nearest::<1>(), None);
    }

    #[test]
    fn rescale_same_scale_is_identity() {
        let d = Decimal32::<4>(12345);
        let result: Option<Decimal32<4>> = d.rescale_into();
        assert_eq!(result, Some(d));
    }

    // ── Cross-check against Decimal64 ────────────────────────────────────────

    #[test]
    fn arithmetic_agrees_with_decimal64_when_in_range() {
        // For inputs in i32 range, Decimal32 must produce exactly what Decimal64 produces,
        // narrowed (only the intermediate width differs).
        let mut seed: u64 = 0x1234_5678_9abc_def0;
        for _ in 0..5_000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let a = (seed >> 32) as i32;
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            // keep divisors small-ish so quotients frequently fit
            let b = ((seed >> 40) as i32) | 1;

            let a32 = Decimal32::<4>::from_raw(a);
            let b32 = Decimal32::<4>::from_raw(b);
            let a64 = a32.widen();
            let b64 = b32.widen();

            assert_eq!(a32.checked_mul(b32), a64.checked_mul(b64).and_then(Decimal64::narrow));
            assert_eq!(a32.checked_div(b32), a64.checked_div(b64).and_then(Decimal64::narrow));
            // 64-bit saturating_mul never saturates for i32 inputs; clamp its result to i32
            let sat64 = a64.saturating_mul(b64).raw().clamp(i32::MIN as i64, i32::MAX as i64);
            assert_eq!(a32.saturating_mul(b32).raw() as i64, sat64);

            assert_eq!(
                a32.checked_div_round_nearest_even(b32),
                a64.checked_div_round_nearest_even(b64).and_then(Decimal64::narrow)
            );
            assert_eq!(
                a32.checked_div_round_nearest(b32),
                a64.checked_div_round_nearest(b64).and_then(Decimal64::narrow)
            );
            assert_eq!(
                a32.checked_div_round_zero(b32),
                a64.checked_div_round_zero(b64).and_then(Decimal64::narrow)
            );
            assert_eq!(
                a32.checked_div_round_ceil(b32),
                a64.checked_div_round_ceil(b64).and_then(Decimal64::narrow)
            );
            assert_eq!(
                a32.checked_div_round_floor(b32),
                a64.checked_div_round_floor(b64).and_then(Decimal64::narrow)
            );

            assert_eq!(
                a32.rescale_round_into_nearest_even::<1>(),
                a64.rescale_round_into_nearest_even::<1>().and_then(Decimal64::narrow)
            );
            assert_eq!(
                a32.rescale_round_into_nearest::<1>(),
                a64.rescale_round_into_nearest::<1>().and_then(Decimal64::narrow)
            );
            assert_eq!(
                a32.rescale_round_into_zero::<1>(),
                a64.rescale_round_into_zero::<1>().and_then(Decimal64::narrow)
            );
            assert_eq!(
                a32.rescale_round_into_ceil::<1>(),
                a64.rescale_round_into_ceil::<1>().and_then(Decimal64::narrow)
            );
            assert_eq!(
                a32.rescale_round_into_floor::<1>(),
                a64.rescale_round_into_floor::<1>().and_then(Decimal64::narrow)
            );
            assert_eq!(a32.rescale_into::<1>(), a64.rescale_into::<1>().and_then(Decimal64::narrow));

            #[cfg(any(feature = "std", feature = "alloc"))]
            assert_eq!(a32.to_string(), a64.to_string());
        }
    }
}
