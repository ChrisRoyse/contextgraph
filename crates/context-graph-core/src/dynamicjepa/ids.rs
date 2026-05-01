use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

pub fn validate_string_id(value: &str, field: &str) -> DynamicJepaResult<()> {
    let bytes = value.as_bytes();
    if !(3..=128).contains(&bytes.len()) {
        return Err(DynamicJepaError::validation(
            field,
            format!("id length must be 3..=128 bytes, got {}", bytes.len()),
            "use a lowercase ASCII id matching ^[a-z][a-z0-9_.-]{2,127}$",
        ));
    }
    if !bytes[0].is_ascii_lowercase() {
        return Err(DynamicJepaError::validation(
            field,
            format!("id must start with lowercase ASCII letter, got {value:?}"),
            "start the id with a-z",
        ));
    }
    for b in &bytes[1..] {
        let ok = b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(*b, b'_' | b'-' | b'.');
        if !ok {
            return Err(DynamicJepaError::validation(
                field,
                format!("id contains invalid byte {b} in {value:?}"),
                "use only lowercase ASCII letters, digits, underscore, dash, and dot",
            ));
        }
    }
    Ok(())
}

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> DynamicJepaResult<Self> {
                let value = value.into();
                validate_string_id(&value, stringify!($name))?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn validate(&self) -> DynamicJepaResult<()> {
                validate_string_id(&self.0, stringify!($name))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = DynamicJepaError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }
    };
}

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new(id: Uuid) -> DynamicJepaResult<Self> {
                if id.is_nil() {
                    return Err(DynamicJepaError::validation(
                        stringify!($name),
                        "uuid id must not be nil",
                        "generate a UUID once at the writer boundary and persist that value",
                    ));
                }
                Ok(Self(id))
            }

            pub fn new_v4() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }

            pub fn into_bytes(self) -> [u8; 16] {
                *self.0.as_bytes()
            }

            pub fn validate(&self) -> DynamicJepaResult<()> {
                if self.0.is_nil() {
                    return Err(DynamicJepaError::validation(
                        stringify!($name),
                        "uuid id must not be nil",
                        "generate a UUID once at the writer boundary and persist that value",
                    ));
                }
                Ok(())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }
    };
}

string_id!(DomainPackId);
string_id!(InstrumentId);
string_id!(AdapterId);

uuid_id!(EventId);
uuid_id!(StateId);
uuid_id!(ActionId);
uuid_id!(OutcomeId);
uuid_id!(TransitionId);
uuid_id!(PanelId);
uuid_id!(BindingId);
uuid_id!(TrajectoryId);
uuid_id!(DatasetShardId);
uuid_id!(DatasetId);
uuid_id!(TrainingRunId);
uuid_id!(ModelArtifactId);
uuid_id!(PredictionId);
uuid_id!(SkillId);
uuid_id!(PlanTraceId);
uuid_id!(GuardDecisionId);
uuid_id!(SurpriseEventId);
uuid_id!(VerificationRunId);
uuid_id!(PairwiseReadingId);
uuid_id!(ConstellationId);
uuid_id!(ThresholdCalibrationId);
