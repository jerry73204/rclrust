use std::{
    borrow::Borrow,
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, ensure, Context as _, Result};
use itertools::{chain, Itertools as _};
use quote::quote;

use crate::{
    parser::package::{load_ament_prefix, AmentPrefix},
    types::Package,
};

const DEFAULT_EXCLUDED_PACKAGES: &[&str] = &["libstatistics_collector"];

#[derive(Debug, Clone)]
pub struct CompileConfig {
    search_env: bool,
    codegen_single_file: bool,
    link_rpath: bool,
    ament_prefix_paths: Vec<PathBuf>,
    exclude_packages: HashSet<String>,
    output_dir: PathBuf,
}

impl Default for CompileConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl CompileConfig {
    pub fn new() -> Self {
        Self {
            search_env: true,
            ament_prefix_paths: vec![],
            output_dir: env::var_os("OUT_DIR").unwrap().into(),
            codegen_single_file: true,
            exclude_packages: DEFAULT_EXCLUDED_PACKAGES
                .iter()
                .map(|pkg| pkg.to_string())
                .collect(),
            link_rpath: true,
        }
    }

    pub const fn search_env(mut self, yes: bool) -> Self {
        self.search_env = yes;
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

    pub const fn link_rpath(mut self, yes: bool) -> Self {
        self.link_rpath = yes;
        self
    }

    pub fn out_dir<P>(mut self, dir: P) -> Self
    where
        P: AsRef<Path>,
    {
        self.output_dir = dir.as_ref().to_owned();
        self
    }

    pub fn clear_exclude_packages(mut self) -> Self {
        self.exclude_packages.clear();
        self
    }

    pub fn exclude_package<S>(mut self, package: S) -> Self
    where
        S: ToString,
    {
        self.exclude_packages.insert(package.to_string());
        self
    }

    pub fn exclude_packages<S, I>(mut self, packages: I) -> Self
    where
        S: ToString,
        I: IntoIterator<Item = S>,
    {
        self.exclude_packages
            .extend(packages.into_iter().map(|pkg| pkg.to_string()));
        self
    }

    pub fn run(self) -> Result<CompileOutput> {
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
                // format!("cargo:rerun-if-changed={}", ament.lib_dir.display()),
                format!("cargo:rerun-if-changed={}", ament.include_dir.display()),
            ])
        });

        // codegen
        let generated_rust_files = if self.codegen_single_file {
            let rust_file = self.run_codegen_file(&packages)?;
            vec![rust_file]
        } else {
            self.run_codegen_dir(&packages)?
        };

        // compile
        self.link(&aments, &mut build_commands);

        Ok(CompileOutput {
            package_names,
            build_commands,
            generated_rust_files,
        })
    }

    fn load_ament_prefixes<B>(&self, build_commands: &mut B) -> Result<Vec<AmentPrefix>>
    where
        B: Extend<String>,
    {
        let default_dirs = if self.search_env {
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
                fs::write(&rust_file, &content)?;
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
        fs::write(&rust_file, &content)?;
        Ok(rust_file)
    }

    fn link<A, B>(&self, aments: &[A], build_commands: &mut B)
    where
        A: Borrow<AmentPrefix>,
        B: Extend<String>,
    {
        // Add library search dirs
        aments.iter().for_each(|ament| {
            let lib_dir = &ament.borrow().lib_dir;

            build_commands.extend(chain!(
                [format!(
                    "cargo:rustc-link-search=native={}",
                    lib_dir.display()
                )],
                self.link_rpath
                    .then(|| [
                        format!("cargo:rustc-link-arg=-Wl,-rpath={}", lib_dir.display()),
                        format!("cargo:rustc-link-arg=-Wl,--disable-new-dtags")
                    ])
                    .into_iter()
                    .flatten()
            ));
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
    }

    fn rust_src_dir(&self) -> PathBuf {
        self.output_dir.join("rust_src")
    }
}

#[derive(Debug)]
pub struct CompileOutput {
    pub package_names: Vec<String>,
    pub build_commands: Vec<String>,
    pub generated_rust_files: Vec<PathBuf>,
}
