use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use itertools::{chain, Itertools as _};
use quote::quote;
use rclrust_msg_parse::{parser::package::AmentPrefix, types::Library};

use crate::{config::CompileConfig, generator::Generator};

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
            .map(|pkg| Generator::new(&self.config, pkg).token_stream(false));

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

    pub fn static_link(&mut self) -> Result<()> {
        let commands: Vec<_> = self
            .aments
            .iter()
            .flat_map(|ament| ament.packages.iter().map(move |pkg| (ament, pkg)))
            .map(|(ament, pkg)| -> Result<_> {
                let include_dir = &ament.include_dir;

                let compile_lib = |lib: &Library| -> Result<_> {
                    let source_files = lib
                        .source_suffixes
                        .iter()
                        .map(|suffix| include_dir.join(suffix));
                    let out_dir = self.config.output_dir.join(&lib.library_name);
                    let commands = [
                        format!("cargo:rustc-link-search={}", out_dir.display()),
                        format!("cargo:rustc-link-lib={}", lib.library_name),
                    ];

                    cc::Build::new()
                        .cargo_metadata(false)
                        .include(include_dir)
                        .files(source_files)
                        .out_dir(out_dir)
                        .try_compile(&lib.library_name)
                        .with_context(|| {
                            format!("unable to compile static library '{}'", lib.library_name)
                        })?;

                    Ok(commands)
                };

                // HACK: Disable the build script in cc but print the build script manually.
                // It avoids `-Wl,--whole-archive` option when using `cargo:rustc-link-lib=static=NAME`.
                // It prints `cargo:rustc-link-lib=NAME` instead.
                // https://github.com/rust-lang/rust/blob/stable/RELEASES.md#compatibility-notes
                let commands1 = compile_lib(&pkg.rosidl_generator_c_lib)?;
                let commands2 = compile_lib(&pkg.rosidl_typesupport_c_lib)?;
                let commands = chain!(commands1, commands2);

                Ok(commands)
            })
            .flatten_ok()
            .try_collect()?;

        self.extend_build_script(commands);

        Ok(())
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
