use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::{anyhow, Result};
use itertools::Itertools as _;
use path_macro::path;

use super::{action::parse_action_file, message::parse_message_file, service::parse_service_file};
use crate::types::Package;

const ROSIDL_INTERFACES: &str = "share/ament_index/resource_index/rosidl_interfaces";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Ns {
    Msg,
    Srv,
    Action,
}

struct IdlLine<'a> {
    pub ns: Ns,
    pub file_name: &'a str,
}

fn parse_line(line: &str) -> Result<Option<IdlLine>> {
    if !line.ends_with(".idl") {
        return Ok(None);
    }

    let err = || anyhow!("Unknown type: {:?}", line);

    let (ns_name, file_name) = line.split_once('/').ok_or_else(err)?;
    let ns = match ns_name {
        "msg" => Ns::Msg,
        "srv" => Ns::Srv,
        "action" => Ns::Action,
        _ => return Err(err()),
    };

    Ok(Some(IdlLine { ns, file_name }))
}

fn get_ros_msgs_each_package<P>(root_dir: P) -> Result<Vec<Package>>
where
    P: AsRef<Path>,
{
    let root_dir = root_dir.as_ref();
    let share_dir = root_dir.join("share");
    let idl_dir = root_dir.join(ROSIDL_INTERFACES);

    let packages: Vec<_> = fs::read_dir(&idl_dir)?
        .map(|entry| -> Result<Option<_>> {
            let path = entry?.path();
            let pkg_name: Option<&str> = (|| path.file_name()?.to_str())();

            // Hack
            let pkg_name = match pkg_name {
                Some(name) if name == "libstatistics_collector" => return Ok(None),
                Some(name) => name,
                None => return Ok(None),
            };

            let mut lines = BufReader::new(File::open(&path)?).lines();

            let package = lines.try_fold(
                Package::new(pkg_name.to_string()),
                |mut package, line| -> Result<_> {
                    let line = line?;
                    let IdlLine { ns, file_name } = match parse_line(&line)? {
                        Some(tuple) => tuple,
                        None => return Ok(package),
                    };

                    match ns {
                        Ns::Msg => {
                            let msg = parse_message_file(
                                pkg_name,
                                path!(share_dir / &pkg_name / "msg" / file_name),
                            )?;
                            package.messages.push(msg);
                        }
                        Ns::Srv => {
                            let srv = parse_service_file(
                                pkg_name,
                                path!(share_dir / &pkg_name / "srv" / file_name),
                            )?;
                            package.services.push(srv);
                        }
                        Ns::Action => {
                            let action = parse_action_file(
                                pkg_name,
                                path!(share_dir / &pkg_name / "action" / file_name),
                            )?;
                            package.actions.push(action);
                        }
                    }

                    Ok(package)
                },
            )?;

            Ok(Some(package))
        })
        .flatten_ok()
        .try_collect()?;

    Ok(packages)
}

pub fn get_packages<P>(paths: &[P]) -> Result<Vec<Package>>
where
    P: AsRef<Path>,
{
    let mut packages: Vec<_> = paths
        .iter()
        .map(|path| get_ros_msgs_each_package(path.as_ref()))
        .flatten_ok()
        .filter(|p| if let Ok(p) = p { !p.is_empty() } else { true })
        .try_collect()?;

    packages.sort_by(|lp, rp| lp.name.cmp(&rp.name));
    packages.dedup_by(|lp, rp| lp.name == rp.name);

    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_line(path: &str, expect_ns: Ns, expect_file_name: &str) {
        assert!(matches!(
                parse_line(path),
                Ok(Some(IdlLine{ns, file_name, ..}))
                    if ns == expect_ns && file_name == expect_file_name));
    }

    #[test]
    fn parse_line_test() {
        assert_line("msg/TestHoge.idl", Ns::Msg, "TestHoge.idl");
        assert_line("srv/TestHoge.idl", Ns::Srv, "TestHoge.idl");
        assert_line("action/TestHoge.idl", Ns::Action, "TestHoge.idl");

        assert!(matches!(parse_line("test/Test.msg"), Ok(None)));
        assert!(matches!(parse_line("test/Test.srv"), Ok(None)));
        assert!(matches!(parse_line("test/Test.action"), Ok(None)));

        assert!(matches!(parse_line("msg/Test.test"), Ok(None)));
        assert!(matches!(parse_line("srv/Test.test"), Ok(None)));
        assert!(matches!(parse_line("action/Test.test"), Ok(None)));
    }
}
