use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::{anyhow, ensure, Result};
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

struct IdlLine {
    pub ns: Ns,
    pub file_name: String,
}

struct IdlPackage {
    pub pkg_name: String,
    pub lines: Vec<IdlLine>,
}

impl IdlLine {
    pub fn name(&self) -> &str {
        self.file_name.rsplit_once('.').unwrap().0
    }
}

pub fn load_package_dir<P>(package_dir: P) -> Result<Package>
where
    P: AsRef<Path>,
{
    let package_dir = package_dir.as_ref().canonicalize()?;
    let invalid_dir_name_err = || anyhow!("Invalid directory name {}", package_dir.display());
    let pkg_name = package_dir
        .file_name()
        .ok_or_else(invalid_dir_name_err)?
        .to_str()
        .ok_or_else(invalid_dir_name_err)?;
    let msg_dir = package_dir.join("msg");
    let srv_dir = package_dir.join("srv");
    let action_dir = package_dir.join("action");

    let mut package = Package::new(pkg_name.to_string());

    if msg_dir.exists() {
        msg_dir
            .read_dir()?
            .map(|entry| -> Result<_> {
                let entry = entry?;
                let path = entry.path();

                match (path.file_stem(), path.extension()) {
                    (Some(_), Some(ext)) if ext == "msg" => {}
                    _ => return Ok(None),
                }

                Ok(Some(path))
            })
            .flatten_ok()
            .map(|path| -> Result<_> {
                let msg = parse_message_file(pkg_name, &path?)?;
                Ok(msg)
            })
            .try_for_each(|msg| -> Result<_> {
                package.messages.push(msg?);
                Ok(())
            })?;
    }

    if srv_dir.exists() {
        srv_dir
            .read_dir()?
            .map(|entry| -> Result<_> {
                let entry = entry?;
                let path = entry.path();

                match (path.file_stem(), path.extension()) {
                    (Some(_), Some(ext)) if ext == "srv" => {}
                    _ => return Ok(None),
                }

                Ok(Some(path))
            })
            .flatten_ok()
            .map(|path| -> Result<_> {
                let srv = parse_service_file(pkg_name, &path?)?;
                Ok(srv)
            })
            .try_for_each(|srv| -> Result<_> {
                package.services.push(srv?);
                Ok(())
            })?;
    }

    if action_dir.exists() {
        action_dir
            .read_dir()?
            .map(|entry| -> Result<_> {
                let entry = entry?;
                let path = entry.path();

                match (path.file_stem(), path.extension()) {
                    (Some(_), Some(ext)) if ext == "action" => {}
                    _ => return Ok(None),
                }

                Ok(Some(path))
            })
            .flatten_ok()
            .map(|path| -> Result<_> {
                let action = parse_action_file(pkg_name, &path?)?;
                Ok(action)
            })
            .try_for_each(|action| -> Result<_> {
                package.actions.push(action?);
                Ok(())
            })?;
    }

    ensure!(
        package.is_empty(),
        "None of msg, srv, action directory found in '{}'",
        package_dir.display()
    );

    Ok(package)
}

pub fn load_ros2_dir<P>(root_dir: P) -> Result<Vec<Package>>
where
    P: AsRef<Path>,
{
    let root_dir = root_dir.as_ref();
    let share_dir = root_dir.join("share");
    let idl_dir = root_dir.join(ROSIDL_INTERFACES);

    let packages: Vec<_> = load_rosidl_interfaces(&idl_dir)?
        .into_iter()
        .map(|pkg| -> Result<_> {
            let IdlPackage { pkg_name, lines } = pkg;

            let package = lines.into_iter().try_fold(
                Package::new(pkg_name.clone()),
                |mut package, idl_line| -> Result<_> {
                    let IdlLine { ns, file_name } = idl_line;

                    match ns {
                        Ns::Msg => {
                            let msg = parse_message_file(
                                &pkg_name,
                                path!(share_dir / &pkg_name / "msg" / &*file_name),
                            )?;
                            package.messages.push(msg);
                        }
                        Ns::Srv => {
                            let srv = parse_service_file(
                                &pkg_name,
                                path!(share_dir / &pkg_name / "srv" / &*file_name),
                            )?;
                            package.services.push(srv);
                        }
                        Ns::Action => {
                            let action = parse_action_file(
                                &pkg_name,
                                path!(share_dir / &pkg_name / "action" / &*file_name),
                            )?;
                            package.actions.push(action);
                        }
                    }

                    Ok(package)
                },
            )?;

            Ok(package)
        })
        .try_collect()?;

    Ok(packages)
}

fn load_rosidl_interfaces<P>(dir: P) -> Result<Vec<IdlPackage>>
where
    P: AsRef<Path>,
{
    let packages: Vec<_> = fs::read_dir(&dir)?
        .map(|entry| -> Result<_> {
            let path = entry?.path();
            let pkg_name: Option<&str> = (|| path.file_name()?.to_str())();

            // Hack
            let pkg_name = match pkg_name {
                Some(name) => name,
                None => return Ok(None),
            };

            let reader = BufReader::new(File::open(&path)?);
            let lines: Vec<_> = reader
                .lines()
                .map(|line| -> Result<_> {
                    let line = line?;
                    let idl_line = parse_line(&line)?;
                    Ok(idl_line)
                })
                .flatten_ok()
                .try_collect()?;

            let package = IdlPackage {
                pkg_name: pkg_name.to_string(),
                lines,
            };
            Ok(Some(package))
        })
        .flatten_ok()
        .try_collect()?;

    Ok(packages)
}

fn parse_line(line: &str) -> Result<Option<IdlLine>> {
    if !line.ends_with(".idl") {
        return Ok(None);
    }

    let err = || anyhow!("Unknown type: {:?}", line);

    let (ns_name, idl_file_name) = line.split_once('/').ok_or_else(err)?;
    let idl_file_name = Path::new(idl_file_name);

    let (ns, file_name) = match ns_name {
        "msg" => (Ns::Msg, idl_file_name.with_extension("msg")),
        "srv" => (Ns::Srv, idl_file_name.with_extension("srv")),
        "action" => (Ns::Action, idl_file_name.with_extension("action")),
        _ => return Err(err()),
    };

    Ok(Some(IdlLine {
        ns,
        file_name: file_name.into_os_string().into_string().unwrap(),
    }))
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
        assert_line("msg/TestHoge.idl", Ns::Msg, "TestHoge.msg");
        assert_line("srv/TestHoge.idl", Ns::Srv, "TestHoge.srv");
        assert_line("action/TestHoge.idl", Ns::Action, "TestHoge.action");

        assert!(matches!(parse_line("test/Test.msg"), Ok(None)));
        assert!(matches!(parse_line("test/Test.srv"), Ok(None)));
        assert!(matches!(parse_line("test/Test.action"), Ok(None)));

        assert!(matches!(parse_line("msg/Test.test"), Ok(None)));
        assert!(matches!(parse_line("srv/Test.test"), Ok(None)));
        assert!(matches!(parse_line("action/Test.test"), Ok(None)));
    }
}
