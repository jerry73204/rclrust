use super::{primitives::*, ConstantType, MemberType};

/// A member of a structure
#[derive(Debug, Clone)]
pub struct Member {
    /// The name of the member
    pub name: String,
    /// The type of the member
    pub r#type: MemberType,
    /// The default value of the member (optional)
    pub default: Option<Vec<String>>,
}

impl Member {
    fn dummy() -> Self {
        Self {
            name: "structure_needs_at_least_one_member".into(),
            r#type: BasicType::U8.into(),
            default: None,
        }
    }
}

/// A constant definition
#[derive(Debug, Clone)]
pub struct Constant {
    /// The name of the constant
    pub name: String,
    /// The type of the constant
    pub r#type: ConstantType,
    /// The value of the constant
    pub value: Vec<String>,
}

/// A message definition
#[derive(Debug, Clone)]
pub struct Message {
    /// The package name
    pub package: String,
    /// The name of the message
    pub name: String,
    /// The list of the members
    pub members: Vec<Member>,
    /// The list of the constants
    pub constants: Vec<Constant>,
}
