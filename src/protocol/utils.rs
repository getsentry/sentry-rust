pub mod ts_seconds_float {
    use std::fmt;
    use serde::{ser, de};
    use chrono::{DateTime, Utc, TimeZone};

    pub fn deserialize<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
        where D: de::Deserializer<'de>
    {
        Ok(d.deserialize_any(SecondsTimestampVisitor)
           .map(|dt| dt.with_timezone(&Utc))?)
    }

    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer
    {
        if dt.timestamp_subsec_nanos() == 0 {
            serializer.serialize_i64(dt.timestamp())
        } else {
            serializer.serialize_f64(
                (dt.timestamp() as f64) +
                ((dt.timestamp_subsec_micros() as f64) / 1_000_000f64)
            )
        }
    }

    struct SecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for SecondsTimestampVisitor {
        type Value = DateTime<Utc>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result
        {
            write!(formatter, "a unix timestamp in seconds")
        }

        fn visit_f64<E>(self, value: f64) -> Result<DateTime<Utc>, E>
            where E: de::Error
        {
            let secs = value as i64;
            let micros = (value.fract() * 1_000_000f64) as u32;
            Ok(Utc.timestamp_opt(secs, micros * 1000).unwrap())
        }

        fn visit_i64<E>(self, value: i64) -> Result<DateTime<Utc>, E>
            where E: de::Error
        {
            Ok(Utc.timestamp_opt(value, 0).unwrap())
        }

        fn visit_u64<E>(self, value: u64) -> Result<DateTime<Utc>, E>
            where E: de::Error
        {
            Ok(Utc.timestamp_opt(value as i64, 0).unwrap())
        }
    }
}
