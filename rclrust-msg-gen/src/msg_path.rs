pub use ament_tree::*;
pub mod ament_tree {
    use std::collections::HashMap;

    use itertools::chain;

    use super::MsgPath;

    pub enum MsgKind {
        Msg,
        Srv,
        Action,
    }

    pub struct AmentTree<T> {
        wildcard: Option<T>,
        packages: HashMap<String, PackageTree<T>>,
    }

    impl<T> Default for AmentTree<T> {
        fn default() -> Self {
            Self {
                wildcard: None,
                packages: HashMap::new(),
            }
        }
    }

    impl<T> AmentTree<T> {
        pub fn set(&mut self, path: &MsgPath, context: T) {
            match path {
                MsgPath::Wild => self.wildcard = Some(context),
                MsgPath::PkgWild { pkg } => {
                    self.packages
                        .entry(pkg.clone())
                        .or_insert_with(PackageTree::default)
                        .wildcard = Some(context);
                }
                MsgPath::PkgMsgWild { pkg } => {
                    self.packages
                        .entry(pkg.clone())
                        .or_insert_with(PackageTree::default)
                        .msg
                        .wildcard = Some(context);
                }
                MsgPath::PkgMsgMsg { pkg, msg } => {
                    self.packages
                        .entry(pkg.clone())
                        .or_insert_with(PackageTree::default)
                        .msg
                        .messages
                        .insert(msg.clone(), context);
                }
                MsgPath::PkgSrvWild { pkg } => {
                    self.packages
                        .entry(pkg.clone())
                        .or_insert_with(PackageTree::default)
                        .srv
                        .wildcard = Some(context);
                }
                MsgPath::PkgSrvMsg { pkg, srv } => {
                    self.packages
                        .entry(pkg.clone())
                        .or_insert_with(PackageTree::default)
                        .srv
                        .messages
                        .insert(srv.clone(), context);
                }
                MsgPath::PkgActionWild { pkg } => {
                    self.packages
                        .entry(pkg.clone())
                        .or_insert_with(PackageTree::default)
                        .action
                        .wildcard = Some(context);
                }
                MsgPath::PkgActionMsg { pkg, action } => {
                    self.packages
                        .entry(pkg.clone())
                        .or_insert_with(PackageTree::default)
                        .action
                        .messages
                        .insert(action.clone(), context);
                }
            }
        }

        pub fn get(&mut self, pkg: &str, kind: MsgKind, msg: &str) -> Vec<&T> {
            let opt1 = self.wildcard.as_ref();

            let opts = self
                .packages
                .get(pkg)
                .map(|pkg_tree| {
                    let opt2 = pkg_tree.wildcard.as_ref();

                    let msg_tree = match kind {
                        MsgKind::Msg => &pkg_tree.msg,
                        MsgKind::Srv => &pkg_tree.srv,
                        MsgKind::Action => &pkg_tree.action,
                    };

                    let opt3 = msg_tree.wildcard.as_ref();
                    let opt4 = msg_tree.messages.get(msg);

                    chain!(opt2, opt3, opt4)
                })
                .into_iter()
                .flatten();

            let contexts: Vec<&T> = chain!(opt1, opts).collect();
            contexts
        }
    }

    struct PackageTree<T> {
        pub wildcard: Option<T>,
        pub msg: MsgTree<T>,
        pub srv: MsgTree<T>,
        pub action: MsgTree<T>,
    }

    impl<T> Default for PackageTree<T> {
        fn default() -> Self {
            Self {
                wildcard: None,
                msg: Default::default(),
                srv: Default::default(),
                action: Default::default(),
            }
        }
    }

    struct MsgTree<T> {
        pub wildcard: Option<T>,
        pub messages: HashMap<String, T>,
    }

    impl<T> Default for MsgTree<T> {
        fn default() -> Self {
            Self {
                wildcard: None,
                messages: HashMap::new(),
            }
        }
    }
}

pub use msg_path::*;
pub mod msg_path {
    use std::str::FromStr;

    use anyhow::{anyhow, Error};

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum MsgPath {
        Wild,
        PkgWild { pkg: String },
        PkgMsgWild { pkg: String },
        PkgMsgMsg { pkg: String, msg: String },
        PkgSrvWild { pkg: String },
        PkgSrvMsg { pkg: String, srv: String },
        PkgActionWild { pkg: String },
        PkgActionMsg { pkg: String, action: String },
    }

    impl FromStr for MsgPath {
        type Err = Error;

        fn from_str(text: &str) -> Result<Self, Self::Err> {
            enum Pkg<'a> {
                Wildcard,
                Name(&'a str),
            }

            enum Msg<'a> {
                Wildcard,
                Name(&'a str),
            }

            enum Kind {
                Wildcard,
                Msg,
                Srv,
                Action,
            }

            let err = || Err(anyhow!("invalid message path '{}'", text));

            let tokens: Vec<_> = text.split('/').collect();
            let [pkg, kind, msg] = match *tokens {
                [pkg, kind, msg] => [pkg, kind, msg],
                _ => return err(),
            };

            let pkg = match pkg {
                "*" => Pkg::Wildcard,
                pkg if pkg.chars().all(|c: char| char::is_ascii_alphanumeric(&c)) => Pkg::Name(pkg),
                _ => return err(),
            };
            let kind = match kind {
                "*" => Kind::Wildcard,
                pkg if pkg == "msg" => Kind::Msg,
                pkg if pkg == "srv" => Kind::Srv,
                pkg if pkg == "action" => Kind::Action,
                _ => return err(),
            };
            let msg = match msg {
                "*" => Msg::Wildcard,
                pkg if pkg
                    .chars()
                    .all(|c: char| char::is_ascii_alphanumeric(&c) || c == '_') =>
                {
                    Msg::Name(pkg)
                }
                _ => return err(),
            };

            let path = match (pkg, kind, msg) {
                (Pkg::Wildcard, Kind::Wildcard, Msg::Wildcard) => Self::Wild,
                (Pkg::Name(pkg), Kind::Wildcard, Msg::Wildcard) => Self::PkgWild {
                    pkg: pkg.to_string(),
                },
                (Pkg::Name(pkg), Kind::Msg, Msg::Wildcard) => Self::PkgMsgWild {
                    pkg: pkg.to_string(),
                },
                (Pkg::Name(pkg), Kind::Msg, Msg::Name(msg)) => Self::PkgMsgMsg {
                    pkg: pkg.to_string(),
                    msg: msg.to_string(),
                },
                (Pkg::Name(pkg), Kind::Srv, Msg::Wildcard) => Self::PkgSrvWild {
                    pkg: pkg.to_string(),
                },
                (Pkg::Name(pkg), Kind::Srv, Msg::Name(msg)) => Self::PkgSrvMsg {
                    pkg: pkg.to_string(),
                    srv: msg.to_string(),
                },
                (Pkg::Name(pkg), Kind::Action, Msg::Wildcard) => Self::PkgActionWild {
                    pkg: pkg.to_string(),
                },
                (Pkg::Name(pkg), Kind::Action, Msg::Name(msg)) => Self::PkgActionMsg {
                    pkg: pkg.to_string(),
                    action: msg.to_string(),
                },
                _ => {
                    return err();
                }
            };

            Ok(path)
        }
    }
}
