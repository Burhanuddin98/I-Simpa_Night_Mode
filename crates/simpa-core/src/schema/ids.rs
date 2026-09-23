//! Typed UUID ids, one type per kind of entity so they cannot be mixed up.

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! uuid_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        ///
        /// In JSON: the hyphenated lowercase UUID string.
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// A new random (version 4) id.
            pub fn random() -> Self {
                $name(Uuid::new_v4())
            }

            /// A fixed id, for fixtures and tests.
            pub const fn from_u128(value: u128) -> Self {
                $name(Uuid::from_u128(value))
            }

            pub const fn uuid(self) -> Uuid {
                self.0
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Uuid {
                id.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl JsonSchema for $name {
            fn inline_schema() -> bool {
                true
            }

            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!({ "type": "string", "format": "uuid" })
            }
        }
    };
}

uuid_id!(
    /// Identifies a project.
    ProjectId
);
uuid_id!(
    /// Identifies a [`SurfaceGroup`](super::SurfaceGroup).
    GroupId
);
uuid_id!(
    /// Identifies a [`Material`](super::Material).
    MaterialId
);
uuid_id!(
    /// Identifies a [`Source`](super::Source).
    SourceId
);
uuid_id!(
    /// Identifies a [`PointReceiver`](super::PointReceiver).
    PointReceiverId
);
uuid_id!(
    /// Identifies a [`SurfaceReceiver`](super::SurfaceReceiver).
    SurfaceReceiverId
);
uuid_id!(
    /// Identifies a [`FittingZone`](super::FittingZone).
    FittingZoneId
);
uuid_id!(
    /// Identifies a [`Variant`](super::Variant).
    VariantId
);
