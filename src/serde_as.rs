//! Adapter modules for `#[serde(with = "...")]` to serialize `Decimal64<S>` / `UDecimal64<S>`
//! / `Decimal32<S>` / `UDecimal32<S>` as their raw integer values instead of decimal strings.

/// Serialize/deserialize `Decimal64<S>` as a raw `i64`.
pub mod raw_i64 {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::Decimal64;

    pub fn serialize<const S: u32, Ser: Serializer>(
        val: &Decimal64<S>,
        ser: Ser,
    ) -> Result<Ser::Ok, Ser::Error> {
        ser.serialize_i64(val.raw())
    }

    pub fn deserialize<'de, const S: u32, D: Deserializer<'de>>(
        de: D,
    ) -> Result<Decimal64<S>, D::Error> {
        let raw = i64::deserialize(de)?;
        Ok(Decimal64::from_raw(raw))
    }
}

/// Serialize/deserialize `UDecimal64<S>` as a raw `u64`.
pub mod raw_u64 {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::UDecimal64;

    pub fn serialize<const S: u32, Ser: Serializer>(
        val: &UDecimal64<S>,
        ser: Ser,
    ) -> Result<Ser::Ok, Ser::Error> {
        ser.serialize_u64(val.raw())
    }

    pub fn deserialize<'de, const S: u32, D: Deserializer<'de>>(
        de: D,
    ) -> Result<UDecimal64<S>, D::Error> {
        let raw = u64::deserialize(de)?;
        Ok(UDecimal64::from_raw(raw))
    }
}

/// Serialize/deserialize `Decimal32<S>` as a raw `i32`.
pub mod raw_i32 {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::Decimal32;

    pub fn serialize<const S: u32, Ser: Serializer>(
        val: &Decimal32<S>,
        ser: Ser,
    ) -> Result<Ser::Ok, Ser::Error> {
        ser.serialize_i32(val.raw())
    }

    pub fn deserialize<'de, const S: u32, D: Deserializer<'de>>(
        de: D,
    ) -> Result<Decimal32<S>, D::Error> {
        let raw = i32::deserialize(de)?;
        Ok(Decimal32::from_raw(raw))
    }
}

/// Serialize/deserialize `UDecimal32<S>` as a raw `u32`.
pub mod raw_u32 {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::UDecimal32;

    pub fn serialize<const S: u32, Ser: Serializer>(
        val: &UDecimal32<S>,
        ser: Ser,
    ) -> Result<Ser::Ok, Ser::Error> {
        ser.serialize_u32(val.raw())
    }

    pub fn deserialize<'de, const S: u32, D: Deserializer<'de>>(
        de: D,
    ) -> Result<UDecimal32<S>, D::Error> {
        let raw = u32::deserialize(de)?;
        Ok(UDecimal32::from_raw(raw))
    }
}
