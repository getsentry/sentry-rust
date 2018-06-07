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
                <::std::borrow::Cow<str>>::deserialize(deserializer)?
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
                enum Repr<'a> {
                    #[serde(borrow)]
                    Str(::std::borrow::Cow<'a, str>),
                    Uint($num),
                }

                Ok(match Repr::deserialize(deserializer)? {
                    Repr::Str(s) => s.parse().map_err(::serde::de::Error::custom)?,
                    Repr::Uint(val) => $type(val),
                })
            }
        }

        impl ::std::str::FromStr for $type {
            type Err = ::std::num::ParseIntError;

            fn from_str(s: &str) -> Result<$type, ::std::num::ParseIntError> {
                if s.starts_with("0x") || s.starts_with("0X") {
                    $num::from_str_radix(&s[2..], 16).map($type)
                } else {
                    $num::from_str_radix(&s, 10).map($type)
                }
            }
        }

        impl ::std::fmt::Display for $type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "{:#x}", self.0)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::io::Cursor;
    use std::str::FromStr;

    use serde_json;

    struct Test;

    impl FromStr for Test {
        type Err = &'static str;

        fn from_str(string: &str) -> Result<Self, Self::Err> {
            match string {
                "test" => Ok(Test),
                _ => Err("failed"),
            }
        }
    }

    impl fmt::Display for Test {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "test")
        }
    }

    impl_str_serialization!(Test);

    #[test]
    fn test_serialize_string() {
        assert_eq!("\"test\"", serde_json::to_string(&Test).unwrap());
    }

    #[test]
    fn test_deserialize() {
        assert!(serde_json::from_str::<Test>("\"test\"").is_ok());
    }

    #[test]
    fn test_deserialize_owned() {
        assert!(serde_json::from_reader::<_, Test>(Cursor::new("\"test\"")).is_ok());
    }

    #[derive(Debug, PartialEq)]
    struct Hex(u32);

    impl_serde_hex!(Hex, u32);

    #[test]
    fn test_hex_to_string() {
        assert_eq!("0x0", &Hex(0).to_string());
        assert_eq!("0x2a", &Hex(42).to_string());
    }

    #[test]
    fn test_hex_serialize() {
        assert_eq!("\"0x0\"", serde_json::to_string(&Hex(0)).unwrap());
        assert_eq!("\"0x2a\"", serde_json::to_string(&Hex(42)).unwrap());
    }

    #[test]
    fn test_hex_from_string() {
        assert_eq!(Hex(0), "0".parse().unwrap());
        assert_eq!(Hex(42), "42".parse().unwrap());
        assert_eq!(Hex(42), "0x2a".parse().unwrap());
        assert_eq!(Hex(42), "0X2A".parse().unwrap());
    }

    #[test]
    fn test_hex_deserialize() {
        assert_eq!(Hex(0), serde_json::from_str("\"0\"").unwrap());
        assert_eq!(Hex(42), serde_json::from_str("\"42\"").unwrap());
        assert_eq!(Hex(42), serde_json::from_str("\"0x2a\"").unwrap());
        assert_eq!(Hex(42), serde_json::from_str("\"0X2A\"").unwrap());
    }

    #[test]
    fn test_hex_deserialize_owned() {
        assert_eq!(
            Hex(0),
            serde_json::from_reader(Cursor::new("\"0\"")).unwrap()
        );
        assert_eq!(
            Hex(42),
            serde_json::from_reader(Cursor::new("\"42\"")).unwrap()
        );
        assert_eq!(
            Hex(42),
            serde_json::from_reader(Cursor::new("\"0x2a\"")).unwrap()
        );
        assert_eq!(
            Hex(42),
            serde_json::from_reader(Cursor::new("\"0X2A\"")).unwrap()
        );
    }
}
