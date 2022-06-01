use std::{fs, path::PathBuf};

use anyhow::Result;
use itertools::chain;
use quote::quote;
use rclrust_msg_parse::parser::package::AmentPrefix;

use crate::config::CompileConfig;

#[derive(Debug)]
pub struct Compiler {
    pub(crate) aments: Vec<AmentPrefix>,
    pub(crate) config: CompileConfig,
    pub(crate) build_script: Vec<String>,
    pub(crate) package_names: Vec<String>,
}

impl Compiler {
    pub fn codegen(&mut self) -> Result<()> {
        // register rerun-ifs
        let commands: Vec<_> = self
            .aments
            .iter()
            .flat_map(|ament| {
                [
                    format!("cargo:rerun-if-changed={}", ament.resource_dir.display()),
                    format!("cargo:rerun-if-changed={}", ament.include_dir.display()),
                ]
            })
            .collect();
        self.extend_build_script(commands);

        let output_dir = &self.config.output_dir;

        let mods = self
            .aments
            .iter()
            .flat_map(|ament| &ament.packages)
            .map(|pkg| pkg.token_stream(false));

        let content = quote! {
            #(#mods)*
        }
        .to_string();

        let output_file = output_dir.join("bindings.rs");
        fs::write(&output_file, &content)?;

        Ok(())
    }

    pub fn dynamic_link(&mut self) {
        let link_rpath = self.config.link_rpath;

        // Add library search dirs
        let link_search_cmds = self.aments.iter().flat_map(|ament| {
            let lib_dir = &ament.lib_dir;
            let link_search_cmd = format!("cargo:rustc-link-search=native={}", lib_dir.display());
            let link_arg_cmds = link_rpath
                .then(|| {
                    [
                        format!("cargo:rustc-link-arg=-Wl,-rpath={}", lib_dir.display()),
                        "cargo:rustc-link-arg=-Wl,--disable-new-dtags".to_string(),
                    ]
                })
                .into_iter()
                .flatten();
            chain!([link_search_cmd], link_arg_cmds)
        });

        // Add linked libraries
        let link_lib_cmds = self
            .aments
            .iter()
            .flat_map(|ament| ament.packages.iter().flat_map(|pkg| pkg.library_names()))
            .map(|library_name| format!("cargo:rustc-link-lib=dylib={}", library_name));

        let commands: Vec<_> = chain!(link_search_cmds, link_lib_cmds).collect();
        self.extend_build_script(commands);
    }

    /// Get a reference to the compiler's build script.
    pub fn build_script(&self) -> &[String] {
        self.build_script.as_ref()
    }

    fn extend_build_script<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = String>,
    {
        let commands = commands.into_iter().map(|cmd| {
            if self.config.emit_build_script {
                println!("{}", cmd);
            }
            cmd
        });
        self.build_script.extend(commands);
    }
}

#[derive(Debug)]
pub struct CompileOutput {
    pub package_names: Vec<String>,
    pub build_commands: Vec<String>,
    pub generated_rust_files: Vec<PathBuf>,
}
