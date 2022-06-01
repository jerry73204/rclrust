use std::{fs, path::Path};

use anyhow::{Context, Result};

use super::{error::RclMsgError, message::parse_message_string, utils::fix_newlines};
use crate::types::Action;

const ACTION_GOAL_SUFFIX: &str = "_Goal";
const ACTION_RESULT_SUFFIX: &str = "_Result";
const ACTION_FEEDBACK_SUFFIX: &str = "_Feedback";

pub fn parse_action_file<P>(pkg_name: &str, interface_file: P) -> Result<Action>
where
    P: AsRef<Path>,
{
    let interface_file = interface_file.as_ref();
    parse_action_string(
        pkg_name,
        interface_file.file_stem().unwrap().to_str().unwrap(),
        fs::read_to_string(interface_file)?.as_str(),
    )
    .with_context(|| format!("Parse file error: {}", interface_file.display()))
}

fn parse_action_string(pkg_name: &str, action_name: &str, action_string: &str) -> Result<Action> {
    let err = || {
        RclMsgError::InvalidActionSpecification(
            "Number of '---' separators nonconformant with action definition".into(),
        )
    };

    let action_string = fix_newlines(action_string);
    let (block1, tail) = action_string.split_once("---\n").ok_or_else(err)?;
    let (block2, block3) = tail.split_once("---\n").ok_or_else(err)?;

    Ok(Action {
        package: pkg_name.into(),
        name: action_name.into(),
        goal: parse_message_string(
            pkg_name,
            &format!("{}{}", action_name, ACTION_GOAL_SUFFIX),
            block1,
        )?,
        result: parse_message_string(
            pkg_name,
            &format!("{}{}", action_name, ACTION_RESULT_SUFFIX),
            block2,
        )?,
        feedback: parse_message_string(
            pkg_name,
            &format!("{}{}", action_name, ACTION_FEEDBACK_SUFFIX),
            block3,
        )?,
    })
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;
    use crate::types::{primitives::*, sequences::*, MemberType};

    fn parse_action_def(srv_name: &str) -> Result<Action> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("test_msgs/action/{}.action", srv_name));
        parse_action_file("test_msgs", path)
    }

    #[test]
    fn parse_fibonacci() -> Result<()> {
        let action = parse_action_def("Fibonacci")?;
        assert_eq!(action.package, "test_msgs".to_string());
        assert_eq!(action.name, "Fibonacci".to_string());

        assert_eq!(action.goal.name, "Fibonacci_Goal".to_string());
        assert_eq!(action.goal.members.len(), 1);
        assert_eq!(action.goal.members[0].name, "order".to_string());
        assert_eq!(action.goal.members[0].r#type, BasicType::I32.into());
        assert_eq!(action.goal.constants.len(), 0);

        assert_eq!(action.result.name, "Fibonacci_Result".to_string());
        assert_eq!(action.result.members.len(), 1);
        assert_eq!(action.result.members[0].name, "sequence".to_string());
        assert_eq!(
            action.result.members[0].r#type,
            MemberType::Sequence(Sequence {
                value_type: NestableType::BasicType(BasicType::I32)
            })
        );
        assert_eq!(action.result.constants.len(), 0);

        assert_eq!(action.feedback.name, "Fibonacci_Feedback".to_string());
        assert_eq!(action.feedback.members.len(), 1);
        assert_eq!(action.feedback.members[0].name, "sequence".to_string());
        assert_eq!(
            action.feedback.members[0].r#type,
            MemberType::Sequence(Sequence {
                value_type: NestableType::BasicType(BasicType::I32)
            })
        );
        assert_eq!(action.feedback.constants.len(), 0);

        Ok(())
    }
}
