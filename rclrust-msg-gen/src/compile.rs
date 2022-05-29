use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    env,
    fs::{self, OpenOptions},
    io::{prelude::*, BufWriter},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, ensure, Context as _, Result};
use itertools::{chain, Itertools as _};
use quote::quote;

use crate::{
    parser::package::{load_ament_prefix, AmentPrefix},
    types::Package,
};

const LIBSTATISTICS_COLLECTOR_NAME: &str = "libstatistics_collector";

#[derive(Debug, Clone)]
pub struct CompileConfig {
    search_ament_prefix_path: bool,
    ament_prefix_paths: Vec<PathBuf>,
    exclude_packages: HashSet<Cow<'static, str>>,
    output_dir: PathBuf,
    codegen_single_file: bool,
    codegen: bool,
    cc_build: Option<cc::Build>,
}

impl Default for CompileConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl CompileConfig {
    pub fn new() -> Self {
        Self {
            search_ament_prefix_path: true,
            ament_prefix_paths: vec![],
            output_dir: env::var_os("OUT_DIR").unwrap().into(),
            codegen: true,
            codegen_single_file: true,
            cc_build: None,
            exclude_packages: [Cow::Borrowed(LIBSTATISTICS_COLLECTOR_NAME)]
                .into_iter()
                .collect(),
        }
    }

    pub const fn search_ament_prefix_path(mut self, yes: bool) -> Self {
        self.search_ament_prefix_path = yes;
        self
    }

    pub fn ament_prefix_path<P>(mut self, dir: P) -> Self
    where
        P: AsRef<Path>,
    {
        self.ament_prefix_paths.push(dir.as_ref().to_owned());
        self
    }

    pub fn ament_prefix_paths<P, I>(mut self, dirs: I) -> Self
    where
        P: AsRef<Path>,
        I: IntoIterator<Item = P>,
    {
        self.ament_prefix_paths
            .extend(dirs.into_iter().map(|dir| dir.as_ref().to_owned()));
        self
    }

    pub const fn codegen_single_file(mut self, yes: bool) -> Self {
        self.codegen_single_file = yes;
        self
    }

    pub fn out_dir<P>(mut self, dir: P) -> Self
    where
        P: AsRef<Path>,
    {
        self.output_dir = dir.as_ref().to_owned();
        self
    }

    pub const fn codegen(mut self, yes: bool) -> Self {
        self.codegen = yes;
        self
    }

    pub fn clear_exclude_packages(mut self) -> Self {
        self.exclude_packages.clear();
        self
    }

    pub fn exclude_package<S>(mut self, package: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        self.exclude_packages.insert(package.into());
        self
    }

    pub fn exclude_packages<S, I>(mut self, packages: I) -> Self
    where
        S: Into<Cow<'static, str>>,
        I: IntoIterator<Item = S>,
    {
        self.exclude_packages
            .extend(packages.into_iter().map(|pkg| pkg.into()));
        self
    }

    pub fn compile_ffi<B>(mut self, build: B) -> Self
    where
        B: Into<Option<cc::Build>>,
    {
        self.cc_build = build.into();
        self
    }

    pub fn run(mut self) -> Result<CompileOutput> {
        // init
        let mut build_commands = vec![];

        // list packages
        let aments = self.load_ament_prefixes(&mut build_commands)?;

        // reject duplicated package names
        let mut packages: Vec<_> = aments.iter().flat_map(|ament| &ament.packages).collect();
        packages.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

        let package_names: Vec<_> = packages
            .iter()
            .dedup_by_with_count(|lhs, rhs| lhs.name == rhs.name)
            .map(|(count, pkg)| -> Result<_> {
                ensure!(count == 1, "multiple '{}' packages found", pkg.name);
                Ok(pkg.name.clone())
            })
            .try_collect()?;

        // register rerun-ifs
        aments.iter().for_each(|ament| {
            build_commands.extend([
                format!("cargo:rerun-if-changed={}", ament.resource_dir.display()),
                format!("cargo:rerun-if-changed={}", ament.lib_dir.display()),
                format!("cargo:rerun-if-changed={}", ament.include_dir.display()),
            ])
        });

        // codegen
        let generated_rust_files = if self.codegen {
            if self.codegen_single_file {
                let rust_file = self.run_codegen_file(&packages)?;
                vec![rust_file]
            } else {
                self.run_codegen_dir(&packages)?
            }
        } else {
            vec![]
        };

        // compile
        let (cc_build, generated_c_files) = if let Some(mut cc_build) = self.cc_build.take() {
            let c_file = self.configure_cc_build(&aments, &mut build_commands, &mut cc_build)?;
            (Some(cc_build), vec![c_file])
        } else {
            (None, vec![])
        };

        Ok(CompileOutput {
            package_names,
            cc_build,
            build_commands,
            generated_rust_files,
            generated_c_files,
        })
    }

    fn load_ament_prefixes<B>(&self, build_commands: &mut B) -> Result<Vec<AmentPrefix>>
    where
        B: Extend<String>,
    {
        let default_dirs = if self.search_ament_prefix_path {
            build_commands.extend(["cargo:rerun-if-env-changed=AMENT_PREFIX_PATH".to_string()]);

            let ament_prefix_paths = env::var("AMENT_PREFIX_PATH")
                .with_context(|| anyhow!("$AMENT_PREFIX_PATH is supposed to be set."))?;
            let dirs: Vec<_> = ament_prefix_paths.split(':').map(PathBuf::from).collect();
            dirs
        } else {
            vec![]
        };

        let dirs = chain!(&default_dirs, &self.ament_prefix_paths);

        let aments: Vec<_> = dirs
            .map(|path| load_ament_prefix(path, &self.exclude_packages))
            .try_collect()?;
        Ok(aments)
    }

    fn run_codegen_dir<P>(&self, packages: &[P]) -> Result<Vec<PathBuf>>
    where
        P: Borrow<Package>,
    {
        let rust_src_dir = self.rust_src_dir();
        fs::create_dir(&rust_src_dir)?;

        let rust_files: Vec<_> = packages
            .iter()
            .map(|pkg| -> Result<_> {
                let pkg = pkg.borrow();
                let rust_file = rust_src_dir.join(format!("{}.rs", pkg.name));
                let token_stream = pkg.token_stream(true);
                let content = (quote! { #token_stream }).to_string();

                let mut writer = BufWriter::new(
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(&rust_file)?,
                );
                writer.write_all(content.as_bytes())?;
                writer.flush()?;

                Ok(rust_file)
            })
            .try_collect()?;

        Ok(rust_files)
    }

    fn run_codegen_file<P>(&self, packages: &[P]) -> Result<PathBuf>
    where
        P: Borrow<Package>,
    {
        let mods: Vec<_> = packages
            .iter()
            .map(|pkg| {
                let pkg = pkg.borrow();
                pkg.token_stream(false)
            })
            .collect();

        let content = quote! {
            #(#mods)*
        }
        .to_string();

        let rust_src_dir = self.rust_src_dir();
        fs::create_dir_all(&rust_src_dir)?;
        let rust_file = rust_src_dir.join("ffi.rs");

        let mut writer = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create(true)
                .open(&rust_file)?,
        );
        writer.write_all(content.as_bytes())?;
        writer.flush()?;

        Ok(rust_file)
    }

    fn configure_cc_build<A, B>(
        &self,
        aments: &[A],
        build_commands: &mut B,
        build: &mut cc::Build,
    ) -> Result<PathBuf>
    where
        A: Borrow<AmentPrefix>,
        B: Extend<String>,
    {
        let c_src_dir = self.c_src_dir();

        let include_dirs = aments.iter().map(|ament| &ament.borrow().include_dir);
        build.includes(include_dirs);

        // Add library search dirs
        aments.iter().for_each(|ament| {
            build_commands.extend([format!(
                "cargo:rustc-link-search=native={}",
                ament.borrow().lib_dir.display()
            )]);
        });

        // Add linked libraries
        aments
            .iter()
            .flat_map(|ament| {
                ament
                    .borrow()
                    .packages
                    .iter()
                    .flat_map(|pkg| &pkg.libraries)
            })
            .map(|library_name| format!("cargo:rustc-link-lib=dylib={}", library_name))
            .for_each(|command| {
                build_commands.extend([command]);
            });

        // Generate C code
        let c_code = aments
            .iter()
            .flat_map(|ament| {
                ament.borrow().packages.iter().flat_map(|pkg| {
                    pkg.include_suffixes
                        .iter()
                        .map(|suffix| format!("#include <{}>", suffix.display()))
                })
            })
            .join("\n");

        let c_file = c_src_dir.join("ffi.c");
        fs::create_dir_all(&c_src_dir)?;
        fs::write(&c_file, &c_code)?;

        build.file(&c_file);

        Ok(c_file)
    }

    fn rust_src_dir(&self) -> PathBuf {
        self.output_dir.join("rust_src")
    }

    fn c_src_dir(&self) -> PathBuf {
        self.output_dir.join("c_src")
    }
}

#[derive(Debug)]
pub struct CompileOutput {
    pub package_names: Vec<String>,
    pub cc_build: Option<cc::Build>,
    pub build_commands: Vec<String>,
    pub generated_rust_files: Vec<PathBuf>,
    pub generated_c_files: Vec<PathBuf>,
}
