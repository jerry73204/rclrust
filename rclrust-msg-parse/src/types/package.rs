use std::path::PathBuf;

use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::Ident;

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

    fn messages_block(&self) -> impl ToTokens {
        if self.msgs.is_empty() {
            quote! {
                // empty msg
            }
        } else {
            let items = self.msgs.iter().map(|v| v.token_stream_with_mod("msg"));
            quote! {
                pub mod msg {
                    #(#items)*
                }  // msg
            }
        }
    }

    fn services_block(&self) -> impl ToTokens {
        if self.srvs.is_empty() {
            quote! {
                // empty srv
            }
        } else {
            let items = self.srvs.iter().map(|v| v.token_stream_with_mod("srv"));
            quote! {
                pub mod srv {
                    #(#items)*
                }  // srv
            }
        }
    }

    fn actions_block(&self) -> impl ToTokens {
        if self.actions.is_empty() {
            quote! {
                // empty srv
            }
        } else {
            let items = self
                .actions
                .iter()
                .map(|v| v.token_stream_with_mod("action"));
            quote! {
                pub mod action {
                    #(#items)*
                }  // action
            }
        }
    }

    pub fn token_stream(&self, body_only: bool) -> impl ToTokens {
        let name = Ident::new(&self.name, Span::call_site());
        let messages_block = self.messages_block();
        let services_block = self.services_block();
        let actions_block = self.actions_block();

        let body = quote! {
            #messages_block
            #services_block
            #actions_block
        };

        if body_only {
            body
        } else {
            quote! {
                pub mod #name {
                    #body
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Library {
    pub library_name: String,
    pub include_suffixes: Vec<PathBuf>,
    pub source_suffixes: Vec<PathBuf>,
}
