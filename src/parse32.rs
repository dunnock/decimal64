use crate::{Decimal32, ParseError};

/// Signed 32-bit variant of [`crate::parse::parse`]: identical grammar, `i32` accumulator.
pub(crate) fn parse<const S: u32>(s: &str) -> Result<Decimal32<S>, ParseError> {
    parse_slice::<S>(s.as_bytes())
}

pub(crate) fn parse_slice<const S: u32>(bytes: &[u8]) -> Result<Decimal32<S>, ParseError> {
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

    let mut acc: i32 = 0;
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
            .and_then(|v| v.checked_add(d as i32))
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
                    .and_then(|v| v.checked_add(d as i32))
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

    Ok(Decimal32::from_raw(if negative { -acc } else { acc }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::string::ToString;

    #[test]
    fn parse_zero() {
        let d: Decimal32<4> = "0".parse().unwrap();
        assert_eq!(d, Decimal32::ZERO);
    }

    #[test]
    fn parse_basic_fractional() {
        let d: Decimal32<4> = "1.2345".parse().unwrap();
        assert_eq!(d.raw(), 12345);
    }

    #[test]
    fn parse_negative() {
        let d: Decimal32<2> = "-99.99".parse().unwrap();
        assert_eq!(d.raw(), -9999);
    }

    #[test]
    fn parse_truncation() {
        // extra fractional digit silently truncated toward zero
        let d: Decimal32<4> = "1.23456".parse().unwrap();
        assert_eq!(d.raw(), 12345);
    }

    #[test]
    fn parse_scientific_notation_rejected() {
        let r: Result<Decimal32<2>, _> = "1e5".parse();
        assert!(matches!(r, Err(ParseError::InvalidChar { .. })));
    }

    #[test]
    fn parse_empty() {
        let r: Result<Decimal32<2>, _> = "".parse();
        assert_eq!(r, Err(ParseError::Empty));
    }

    #[test]
    fn parse_overflow_many_digits() {
        let r: Result<Decimal32<2>, _> = "99999999999".parse();
        assert_eq!(r, Err(ParseError::Overflow));
    }

    #[test]
    fn parse_i32_max_boundary() {
        // 21474836.47 at scale 2 == i32::MAX raw
        let d: Decimal32<2> = "21474836.47".parse().unwrap();
        assert_eq!(d.raw(), i32::MAX);
        let r: Result<Decimal32<2>, _> = "21474836.48".parse();
        assert_eq!(r, Err(ParseError::Overflow));
    }

    #[test]
    fn parse_fits_i64_but_not_i32_is_overflow() {
        // 3 billion fits i64 comfortably but exceeds i32::MAX at scale 0
        let r: Result<Decimal32<0>, _> = "3000000000".parse();
        assert_eq!(r, Err(ParseError::Overflow));
    }

    #[test]
    fn parse_scale_padding_overflow() {
        // "1000" fits i32 but 1000 * 10^9 does not
        let r: Result<Decimal32<9>, _> = "1000".parse();
        assert_eq!(r, Err(ParseError::Overflow));
    }

    #[test]
    fn parse_dot_only_is_empty() {
        let r: Result<Decimal32<2>, _> = ".".parse();
        assert_eq!(r, Err(ParseError::Empty));
    }

    #[test]
    fn parse_leading_dot() {
        // ".5" == 0.5000 at scale 4
        let d: Decimal32<4> = ".5".parse().unwrap();
        assert_eq!(d.raw(), 5000);
    }

    #[test]
    fn parse_trailing_dot() {
        // "5." == 5.0000 at scale 4
        let d: Decimal32<4> = "5.".parse().unwrap();
        assert_eq!(d.raw(), 50000);
    }

    #[test]
    fn parse_plus_sign() {
        let d: Decimal32<2> = "+1.00".parse().unwrap();
        assert_eq!(d.raw(), 100);
    }

    #[test]
    fn parse_from_slice() {
        let d = Decimal32::<4>::from_slice(b"123.4567").unwrap();
        assert_eq!(d.raw(), 1_234_567);
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn round_trip() {
        // LCG for deterministic pseudo-random values; skip i32::MIN (abs overflows i32)
        let mut seed: u64 = 0xdeadbeef_cafebabe;
        let mut count = 0;
        while count < 10_000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let raw = (seed >> 32) as i32;
            if raw == i32::MIN {
                continue;
            }
            let d = Decimal32::<4>::from_raw(raw);
            let s = d.to_string();
            let parsed: Decimal32<4> = s
                .parse()
                .unwrap_or_else(|e| panic!("round-trip parse failed: raw={raw}, s={s:?}, err={e}"));
            assert_eq!(parsed, d, "round-trip mismatch: raw={raw}, s={s:?}");
            count += 1;
        }
    }
}
