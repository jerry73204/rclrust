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

impl FromStr for MsgPath {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
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
