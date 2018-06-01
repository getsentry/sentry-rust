/// Helper macro to implement string based serialization.
///
/// If a type implements `FromStr` and `Display` then this automatically
/// implements a serializer/deserializer for that type that dispatches
/// appropriately.
macro_rules! impl_str_serialization {
    ($type:ty) => {
        #[cfg(feature = "with_serde")]
        impl ::serde::ser::Serialize for $type {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::ser::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        #[cfg(feature = "with_serde")]
        impl<'de> ::serde::de::Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                <&str>::deserialize(deserializer)?
                    .parse()
                    .map_err(::serde::de::Error::custom)
            }
        }
    };
}

/// Helper macro to implement serialization from both numeric values or their
/// hex representation as string.
///
/// This implements `Serialize`, `Deserialize`, `Display` and `FromStr`.
/// Serialization will always use a `"0xbeef"` representation of the value.
/// Deserialization supports raw numbers as well as string representations
/// in hex and base10.
macro_rules! impl_serde_hex {
    ($type:ident, $num:ident) => {
        impl ::serde::ser::Serialize for $type {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::ser::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> ::serde::de::Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<$type, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                #[derive(Deserialize)]
                #[serde(untagged)]
                enum Repr {
                    Str(String),
                    Uint($num),
                }

                Ok(match Repr::deserialize(deserializer)? {
                    Repr::Str(s) => s.parse().map_err(D::Error::custom)?,
                    Repr::Uint(val) => $type(val),
                })
            }
        }

        impl str::FromStr for $type {
            type Err = ParseIntError;

            fn from_str(s: &str) -> Result<$type, ParseIntError> {
                if s.starts_with("0x") || s.starts_with("0X") {
                    $num::from_str_radix(&s[2..], 16).map($type)
                } else {
                    $num::from_str_radix(&s, 10).map($type)
                }
            }
        }

        impl fmt::Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{:#x}", self.0)
            }
        }
    };
}
