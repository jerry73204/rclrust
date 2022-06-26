use std::path::PathBuf;

use crate::types::{Action, Message, Service};

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub msgs: Vec<Message>,
    pub srvs: Vec<Service>,
    pub actions: Vec<Action>,
    pub share_suffixes: Vec<PathBuf>,
    pub rosidl_generator_c_lib: Library,
    pub rosidl_typesupport_c_lib: Library,
    // pub rosidl_typesupport_instrospection_c_lib: Library,
}

impl Package {
    pub fn library_names(&self) -> [&str; 2] {
        [
            &self.rosidl_generator_c_lib.library_name,
            &self.rosidl_typesupport_c_lib.library_name,
        ]
    }

    pub fn is_empty(&self) -> bool {
        self.msgs.is_empty() && self.srvs.is_empty() && self.actions.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Library {
    pub library_name: String,
    pub include_suffixes: Vec<PathBuf>,
    pub source_suffixes: Vec<PathBuf>,
}
