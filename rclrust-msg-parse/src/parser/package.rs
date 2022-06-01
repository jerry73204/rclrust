use std::{
    borrow::Borrow,
    collections::HashSet,
    fs::{self, File},
    hash::Hash,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context as _, Result};
use convert_case::{Boundary, Case, Casing as _};
use itertools::Itertools as _;
use path_macro::path;

use super::{action::parse_action_file, message::parse_message_file, service::parse_service_file};
use crate::types::Package;

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

#[derive(Debug, Clone)]
pub struct AmentPrefix {
    pub packages: Vec<Package>,
    pub resource_dir: PathBuf,
    pub lib_dir: PathBuf,
    pub include_dir: PathBuf,
}

pub fn load_ament_prefix<P, S>(root_dir: P, exclude_packages: &HashSet<S>) -> Result<AmentPrefix>
where
    P: AsRef<Path>,
    S: Borrow<str> + Hash + Eq,
{
    let root_dir = root_dir.as_ref();
    let resource_dir =
        path!(root_dir / "share" / "ament_index" / "resource_index" / "rosidl_interfaces");
    let lib_dir = root_dir.join("lib");
    let include_dir = root_dir.join("include");
    let share_dir = root_dir.join("share");

    let packages: Vec<_> = load_rosidl_interfaces(&resource_dir)?
        .into_iter()
        .map(|pkg| -> Result<_> {
            let IdlPackage { pkg_name, lines } = pkg;

            if exclude_packages.contains(&pkg_name) {
                return Ok(None);
            }

            let libraries = vec![
                format!("{}__rosidl_generator_c", pkg_name),
                format!("{}__rosidl_typesupport_c", pkg_name),
                format!("{}__rosidl_typesupport_introspection_c", pkg_name),
            ];
            let mut msgs = vec![];
            let mut srvs = vec![];
            let mut actions = vec![];
            let mut share_suffixes = vec![];
            let mut include_suffixes = vec![
                path!(&pkg_name / "msg" / "rosidl_generator_c__visibility_control.h"),
                path!(
                    &pkg_name / "msg" / "rosidl_typesupport_introspection_c__visibility_control.h"
                ),
            ];

            lines.into_iter().try_for_each(|idl_line| -> Result<_> {
                let camel_name = idl_line.name();
                let snake_name = camel2snake(camel_name);
                let IdlLine { ns, file_name } = &idl_line;

                match ns {
                    Ns::Msg => {
                        let include_suffix = path!(&pkg_name / "msg" / format!("{}.h", snake_name));
                        include_suffixes.push(include_suffix);

                        let share_suffix = path!(&pkg_name / "msg" / &*file_name);
                        let idl_path = path!(share_dir / share_suffix);
                        share_suffixes.push(share_suffix);

                        // panic!("{}", msg_path.display());
                        let msg = parse_message_file(&pkg_name, &idl_path).with_context(|| {
                            anyhow!("unable to parse file '{}'", idl_path.display())
                        })?;
                        msgs.push(msg);
                    }
                    Ns::Srv => {
                        let include_suffix = path!(&pkg_name / "srv" / format!("{}.h", snake_name));
                        include_suffixes.push(include_suffix);

                        let share_suffix = path!(&pkg_name / "srv" / &*file_name);
                        let idl_path = path!(share_dir / share_suffix);
                        share_suffixes.push(share_suffix);

                        // panic!("{}", srv_path.display());
                        let srv = parse_service_file(&pkg_name, &idl_path).with_context(|| {
                            anyhow!("unable to parse file '{}'", idl_path.display())
                        })?;
                        srvs.push(srv);
                    }
                    Ns::Action => {
                        let include_suffix =
                            path!(&pkg_name / "action" / format!("{}.h", snake_name));
                        include_suffixes.push(include_suffix);

                        let share_suffix = path!(&pkg_name / "action" / &*file_name);
                        let idl_path = path!(share_dir / share_suffix);
                        share_suffixes.push(share_suffix);

                        // panic!("{}", action_path.display());
                        let action =
                            parse_action_file(&pkg_name, &idl_path).with_context(|| {
                                anyhow!("unable to parse file '{}'", idl_path.display())
                            })?;
                        actions.push(action);
                    }
                }

                Ok(())
            })?;

            let package = Package {
                name: pkg_name,
                msgs,
                srvs,
                actions,
                include_suffixes,
                share_suffixes,
                libraries,
            };

            Ok(Some(package))
        })
        .flatten_ok()
        .try_collect()?;

    Ok(AmentPrefix {
        packages,
        resource_dir,
        lib_dir,
        include_dir,
    })
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

fn camel2snake(input: &str) -> String {
    use Boundary as B;

    input
        .from_case(Case::Camel)
        .with_boundaries(&[B::LowerUpper, B::DigitUpper, B::Acronym])
        .to_case(Case::Snake)
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
