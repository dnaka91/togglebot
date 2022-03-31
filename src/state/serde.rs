pub use serde::{Deserialize, Serialize};

pub mod pair_time_hms {
    use std::fmt;

    use serde::{ser::SerializeTuple, Deserialize, Deserializer, Serialize, Serializer};
    use time::Time;

    time::serde::format_description!(time_hms, Time, "[hour]:[minute]:[second]");

    #[derive(Serialize, Deserialize)]
    #[serde(transparent)]
    struct Wrapper(#[serde(with = "time_hms")] Time);

    pub fn serialize<S>(value: &(Time, Time), serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(2)?;
        tup.serialize_element(&Wrapper(value.0))?;
        tup.serialize_element(&Wrapper(value.1))?;
        tup.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<(Time, Time), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple(2, Visitor)
    }

    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = (Time, Time);

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("pair of time values formatter as `HH:MM:SS`")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
            A::Error: serde::de::Error,
        {
            let first = seq
                .next_element::<Wrapper>()?
                .ok_or_else(|| <A::Error as serde::de::Error>::custom("first value missing"))?;
            let second = seq
                .next_element::<Wrapper>()?
                .ok_or_else(|| <A::Error as serde::de::Error>::custom("second value missing"))?;

            if seq.next_element::<Wrapper>()?.is_some() {
                return Err(<A::Error as serde::de::Error>::custom("third value found"));
            }

            Ok((first.0, second.0))
        }
    }
}

pub mod weekdays {
    use std::fmt;

    use serde::{ser::SerializeSeq, Deserializer, Serializer};
    use time::Weekday;

    use super::super::HashSet;

    pub fn serialize<S>(value: &HashSet<Weekday>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(value.len()))?;

        for element in value {
            seq.serialize_element(match *element {
                Weekday::Monday => "Mon",
                Weekday::Tuesday => "Tue",
                Weekday::Wednesday => "Wed",
                Weekday::Thursday => "Thu",
                Weekday::Friday => "Fri",
                Weekday::Saturday => "Sat",
                Weekday::Sunday => "Sun",
            })?;
        }

        seq.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashSet<Weekday>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(Visitor)
    }

    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = HashSet<Weekday>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("weekday formatted in short form (like `Mon` for `Monday`)")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut weekdays = HashSet::default();

            while let Some(element) = seq.next_element::<&str>()? {
                weekdays.insert(match element {
                    "Mon" => Weekday::Monday,
                    "Tue" => Weekday::Tuesday,
                    "Wed" => Weekday::Wednesday,
                    "Thu" => Weekday::Thursday,
                    "Fri" => Weekday::Friday,
                    "Sat" => Weekday::Saturday,
                    "Sun" => Weekday::Sunday,
                    _ => {
                        return Err(<A::Error as serde::de::Error>::custom(format!(
                            "unknown weekday `{element}"
                        )))
                    }
                });
            }

            Ok(weekdays)
        }
    }
}
