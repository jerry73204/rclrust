use super::{primitives::*, sequences::*};

macro_rules! define_enum_from {
    ($into_t:ty, $from_t:ty, $path:path) => {
        impl From<$from_t> for $into_t {
            fn from(t: $from_t) -> Self {
                $path(t)
            }
        }
    };
}

/// A type which is available for member
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberType {
    NestableType(NestableType),
    Array(Array),
    Sequence(Sequence),
    BoundedSequence(BoundedSequence),
}

define_enum_from!(MemberType, NestableType, Self::NestableType);
define_enum_from!(MemberType, Array, Self::Array);
define_enum_from!(MemberType, Sequence, Self::Sequence);
define_enum_from!(MemberType, BoundedSequence, Self::BoundedSequence);

impl From<BasicType> for MemberType {
    fn from(t: BasicType) -> Self {
        Self::NestableType(NestableType::BasicType(t))
    }
}

impl From<NamedType> for MemberType {
    fn from(t: NamedType) -> Self {
        Self::NestableType(NestableType::NamedType(t))
    }
}

impl From<NamespacedType> for MemberType {
    fn from(t: NamespacedType) -> Self {
        Self::NestableType(NestableType::NamespacedType(t))
    }
}

impl From<GenericString> for MemberType {
    fn from(t: GenericString) -> Self {
        Self::NestableType(NestableType::GenericString(t))
    }
}
