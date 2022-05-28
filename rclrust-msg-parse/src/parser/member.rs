use anyhow::{ensure, Result};
use nom::{
    bytes::complete::is_not,
    character::complete::{space0, space1},
    combinator::{eof, opt, recognize},
    multi::separated_list1,
    sequence::{preceded, tuple},
};

use super::{error::RclMsgError, ident, literal, types};
use crate::types::{primitives::NestableType, Member, MemberType};

fn nestable_type_default(nestable_type: NestableType, default: &str) -> Result<Vec<String>> {
    use NestableType as N;
    use RclMsgError as E;

    Ok(match nestable_type {
        N::BasicType(t) => {
            let (rest, default) = literal::get_basic_type_literal_parser(t)(default)
                .map_err(|_| E::ParseDefaultValueError(default.into()))?;
            ensure!(rest.is_empty());
            vec![default]
        }
        N::GenericString(t) => {
            let (rest, default) = literal::get_string_literal_parser(t)(default)
                .map_err(|_| E::ParseDefaultValueError(default.into()))?;
            ensure!(rest.is_empty());
            vec![default]
        }
        N::NamedType(t) => return Err(E::InvalidDefaultError(t.to_string()).into()),
        N::NamespacedType(t) => return Err(E::InvalidDefaultError(t.to_string()).into()),
    })
}

fn array_type_default(value_type: NestableType, default: &str) -> Result<Vec<String>> {
    use NestableType as N;
    use RclMsgError as E;

    Ok(match value_type {
        N::BasicType(t) => {
            let (rest, default) = literal::basic_type_sequence(t, default)
                .map_err(|_| E::ParseDefaultValueError(default.into()))?;
            ensure!(rest.is_empty());
            default
        }
        N::NamedType(t) => return Err(E::InvalidDefaultError(t.to_string()).into()),
        N::NamespacedType(t) => return Err(E::InvalidDefaultError(t.to_string()).into()),
        N::GenericString(_) => {
            let (rest, default) = literal::string_literal_sequence(default)
                .map_err(|_| E::ParseDefaultValueError(default.into()))?;
            ensure!(rest.is_empty());
            default
        }
    })
}

fn validate_default(r#type: MemberType, default: &str) -> Result<Vec<String>> {
    use MemberType as M;

    Ok(match r#type {
        M::NestableType(t) => nestable_type_default(t, default)?,
        M::Array(t) => {
            let default = array_type_default(t.value_type, default)?;
            ensure!(default.len() == t.size);
            default
        }
        M::Sequence(t) => array_type_default(t.value_type, default)?,
        M::BoundedSequence(t) => {
            let default = array_type_default(t.value_type, default)?;
            ensure!(default.len() <= t.max_size);
            default
        }
    })
}

pub fn member_def(line: &str) -> Result<Member> {
    let (_, (r#type, _, name, default, _, _)) = tuple((
        types::parse_member_type,
        space1,
        ident::member_name,
        opt(preceded(
            space1,
            recognize(separated_list1(space1, is_not(" \t"))),
        )),
        space0,
        eof,
    ))(line)
    .map_err(|e| RclMsgError::ParseMemberError {
        input: line.into(),
        reason: e.to_string(),
    })?;

    Ok(Member {
        name: name.into(),
        r#type: r#type.clone(),
        default: match default {
            Some(v) => Some(validate_default(r#type, v)?),
            None => None,
        },
    })
}

#[cfg(test)]
mod test {
    use anyhow::Result;

    use super::*;
    use crate::types::primitives::BasicType;

    #[test]
    fn parse_member_def() -> Result<()> {
        let result = member_def("int32 aaa")?;
        assert_eq!(result.name, "aaa");
        assert_eq!(result.r#type, BasicType::I32.into());
        Ok(())
    }

    #[test]
    fn parse_member_def_with_default() -> Result<()> {
        let result = member_def("int32 aaa 30")?;
        assert_eq!(result.name, "aaa");
        assert_eq!(result.r#type, BasicType::I32.into());
        assert_eq!(result.default, Some(vec!["30".into()]));
        Ok(())
    }

    #[test]
    fn parse_member_def_with_invalid_default() -> Result<()> {
        assert!(member_def("uint8 aaa -1").is_err());
        assert!(member_def("uint8 aaa 256").is_err());
        Ok(())
    }
}
