//! Fixed-scale 64-bit and 32-bit decimals for Rust.
//!
//! Provides [`Decimal64<S>`], a `repr(transparent)` wrapper around `i64` where
//! `S` is a compile-time const scale: the raw integer value represents the
//! mathematical value multiplied by `10^S`. Four types share the same API:
//!
//! | Type              | Storage | Max scale | Max value (raw)            |
//! |-------------------|---------|-----------|----------------------------|
//! | [`Decimal64<S>`]  | `i64`   | 18        | ±9 223 372 036 854 775 807 |
//! | [`UDecimal64<S>`] | `u64`   | 18        | 18 446 744 073 709 551 615 |
//! | [`Decimal32<S>`]  | `i32`   | 9         | ±2 147 483 647             |
//! | [`UDecimal32<S>`] | `u32`   | 9         | 4 294 967 295              |
//!
//! The 32-bit types use native 64-bit intermediates for `mul`/`div` (no 128-bit
//! path at all) and convert losslessly to their 64-bit siblings via `widen()` /
//! `From`; the reverse direction is `narrow() -> Option<_>`.
//!
//! # Quick start
//!
//! ```rust
//! use scaled_int::Decimal64;
//!
//! let price: Decimal64<4> = "123.4567".parse().unwrap();
//! let qty: Decimal64<4> = "10.0000".parse().unwrap();
//! let total = price * qty;
//! assert_eq!(total.to_string(), "1234.567");
//! ```
//!
//! See `docs/design.md` for the full design rationale, API surface, and
//! arithmetic rules; `docs/udecimal64-design.md` and `docs/decimal32-design.md`
//! for the unsigned and 32-bit variants.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod decimal32;
pub mod decimal64;
pub(crate) mod parse;
pub(crate) mod parse32;
pub(crate) mod parse_unsigned;
pub(crate) mod parse_unsigned32;
pub mod scientific;
pub mod udecimal32;
pub mod udecimal64;
pub use decimal32::Decimal32;
pub use decimal64::Decimal64;
pub use scientific::Scientific;
pub use udecimal32::UDecimal32;
pub use udecimal64::UDecimal64;

#[cfg(feature = "serde")]
pub mod serde_as;
#[cfg(feature = "serde")]
pub mod serde_impls;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RoundFlag {
    /// Truncate toward zero (same as integer division).
    Zero = 0,
    /// IEEE 754 default: round to nearest, ties go to the even digit (banker's rounding).
    NearestEven = 2,
    /// Round to nearest, ties go away from zero.
    Nearest = 1,
    /// Round toward positive infinity (ceiling).
    Ceil = 3,
    /// Round toward negative infinity (floor).
    Floor = 4,
}
pub(crate) type RoundFlagEnum = u8;
impl RoundFlag {
    pub(crate) const ZERO: u8 = 0;
    pub(crate) const NEAREST: u8 = 1;
    pub(crate) const NEAREST_EVEN: u8 = 2;
    pub(crate) const CEIL: u8 = 3;
    pub(crate) const FLOOR: u8 = 4;

    pub(crate) const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Zero,
            1 => Self::Nearest,
            2 => Self::NearestEven,
            3 => Self::Ceil,
            4 => Self::Floor,
            _ => panic!("Unknown variant for RoundFlag enum"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Input was empty, or contained only a sign or a bare dot.
    Empty,
    /// A byte that is not a digit, sign, or dot was encountered.
    InvalidChar { byte: u8, pos: usize },
    /// The accumulated value exceeded the storage range (`i64`, `u64`, `i32`, or `u32`)
    /// of the target type at this scale.
    Overflow,
    /// Nonzero value combined with a negative exponent so extreme that no
    /// representable non-zero value exists (returned by [`Scientific`] parsing).
    Underflow,
    /// Reserved for a future strict-mode parse; currently unused (extra digits are truncated).
    TooManyFractional { got: u32, max: u32 },
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty or missing digits"),
            ParseError::InvalidChar { byte, pos } => {
                write!(
                    f,
                    "invalid character {:?} at position {}",
                    *byte as char, pos
                )
            }
            ParseError::Overflow => write!(f, "numeric overflow"),
            ParseError::Underflow => {
                write!(f, "value too small to represent (underflow)")
            }
            ParseError::TooManyFractional { got, max } => {
                write!(f, "too many fractional digits: got {}, max {}", got, max)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}
