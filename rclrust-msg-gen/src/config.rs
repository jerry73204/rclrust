use std::{
    collections::HashSet,
    env,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, ensure, Context as _, Result};
use itertools::{chain, Itertools as _};
use rclrust_msg_parse::parser::package::{load_ament_prefix, AmentPrefix};

use crate::compiler::Compiler;

const DEFAULT_EXCLUDED_PACKAGES: &[&str] = &["libstatistics_collector"];

#[derive(Debug, Clone)]
pub struct CompileConfig {
    pub(crate) search_env: bool,
    pub(crate) link_rpath: bool,
    pub(crate) emit_build_script: bool,
    pub(crate) ament_prefix_paths: Vec<PathBuf>,
    pub(crate) exclude_packages: HashSet<String>,
    pub(crate) output_dir: PathBuf,
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
            exclude_packages: DEFAULT_EXCLUDED_PACKAGES
                .iter()
                .map(|pkg| pkg.to_string())
                .collect(),
            link_rpath: true,
            emit_build_script: true,
        }
    }

    pub const fn emit_build_script(mut self, yes: bool) -> Self {
        self.emit_build_script = yes;
        self
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

    pub fn build(self) -> Result<Compiler> {
        let mut build_script = vec![];

        // list packages
        let aments = self.load_ament_prefixes(&mut build_script)?;

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
        {
            let commands = aments.iter().flat_map(|ament| {
                [
                    format!("cargo:rerun-if-changed={}", ament.resource_dir.display()),
                    format!("cargo:rerun-if-changed={}", ament.include_dir.display()),
                ]
            });
            build_script.extend(commands);
        };

        Ok(Compiler {
            config: self,
            build_script,
            package_names,
            aments,
        })
    }

    pub fn run(self) -> Result<()> {
        let mut compiler = self.build()?;
        compiler.codegen()?;
        compiler.dynamic_link();
        Ok(())
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
}

#[derive(Debug)]
pub struct CompileOutput {
    pub package_names: Vec<String>,
    pub build_commands: Vec<String>,
    pub generated_rust_files: Vec<PathBuf>,
}
