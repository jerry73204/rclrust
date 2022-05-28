use std::{
    borrow::{Borrow, Cow},
    env,
    fs::OpenOptions,
    io::{prelude::*, BufWriter},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, ensure, Context as _, Result};
use itertools::{chain, Itertools as _};
use quote::{format_ident, quote};

use crate::{
    parser::package::{load_package_dir, load_ros2_dir},
    types::Package,
};

#[derive(Debug, Clone)]
pub struct CompileConfig {
    include_default_rosidl: bool,
    input_dirs: Vec<PathBuf>,
    ament_prefix_paths: Vec<PathBuf>,
    output_path: PathBuf,
    single_file: bool,
}

impl Default for CompileConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl CompileConfig {
    pub fn new() -> Self {
        Self {
            include_default_rosidl: false,
            input_dirs: vec![],
            ament_prefix_paths: vec![],
            output_path: env::var_os("OUT_DIR").unwrap().into(),
            single_file: false,
        }
    }

    pub const fn include_default_rosidl(mut self, yes: bool) -> Self {
        self.include_default_rosidl = yes;
        self
    }

    pub fn input_dir<'a, P>(mut self, dir: P) -> Self
    where
        P: Into<Cow<'a, Path>>,
    {
        self.input_dirs.push(dir.into().into_owned());
        self
    }

    pub fn input_dirs<'a, P, I>(mut self, dirs: I) -> Self
    where
        P: Into<Cow<'a, Path>>,
        I: IntoIterator<Item = P>,
    {
        self.input_dirs
            .extend(dirs.into_iter().map(|dir| dir.into().into_owned()));
        self
    }

    pub fn ament_prefix_path<'a, P>(mut self, dir: P) -> Self
    where
        P: Into<Cow<'a, Path>>,
    {
        self.ament_prefix_paths.push(dir.into().into_owned());
        self
    }

    pub fn ament_prefix_paths<'a, P, I>(mut self, dirs: I) -> Self
    where
        P: Into<Cow<'a, Path>>,
        I: IntoIterator<Item = P>,
    {
        self.ament_prefix_paths
            .extend(dirs.into_iter().map(|dir| dir.into().into_owned()));
        self
    }

    pub fn out_dir<'a, P>(mut self, dir: P) -> Self
    where
        P: Into<Cow<'a, Path>>,
    {
        self.output_path = dir.into().into_owned();
        self.single_file = false;
        self
    }

    pub fn out_file<'a, P>(mut self, file: P) -> Self
    where
        P: Into<Cow<'a, Path>>,
    {
        self.output_path = file.into().into_owned();
        self.single_file = true;
        self
    }

    pub fn compile(self) -> Result<Vec<String>> {
        let input_packages = self.compile_input_dirs()?;
        let ros2_packages = self.compile_ros2_dirs()?;

        let mut packages: Vec<_> = chain!(input_packages, ros2_packages).collect();
        packages.sort_by_cached_key(|pkg| pkg.name.clone());

        let packages: Vec<_> = packages
            .into_iter()
            .dedup_by_with_count(|prev, next| prev.name == next.name)
            .map(|(count, pkg)| -> Result<_> {
                ensure!(count == 1, "multiple '{}' packages found", pkg.name);
                Ok(pkg)
            })
            .try_collect()?;

        if self.single_file {
            self.generate_file(&packages)?;
        } else {
            self.generate_dir(&packages)?;
        }

        let pkg_names: Vec<_> = packages.into_iter().map(|pkg| pkg.name).collect();
        Ok(pkg_names)
    }

    fn compile_input_dirs(&self) -> Result<Vec<Package>> {
        let packages: Vec<_> = self.input_dirs.iter().map(load_package_dir).try_collect()?;
        Ok(packages)
    }

    fn compile_ros2_dirs(&self) -> Result<Vec<Package>> {
        let default_dirs = if self.include_default_rosidl {
            let ament_prefix_paths = env::var("AMENT_PREFIX_PATH")
                .with_context(|| anyhow!("$AMENT_PREFIX_PATH is supposed to be set."))?;
            let dirs: Vec<_> = ament_prefix_paths.split(':').map(PathBuf::from).collect();
            dirs
        } else {
            vec![]
        };

        let dirs = chain!(&default_dirs, &self.ament_prefix_paths);

        let packages: Vec<_> = dirs
            .map(load_ros2_dir)
            .flatten_ok()
            .filter(|pkg| {
                if let Ok(pkg) = pkg {
                    pkg.name != "libstatistics_collector"
                } else {
                    true
                }
            })
            .try_collect()?;
        Ok(packages)
    }

    fn generate_dir<P>(&self, packages: &[P]) -> Result<()>
    where
        P: Borrow<Package>,
    {
        packages.iter().try_for_each(|pkg| -> Result<_> {
            let pkg = pkg.borrow();
            let output_file = self.output_path.join(format!("{}.rs", pkg.name));
            let token_stream = pkg.token_stream(true);
            let content = (quote! { #token_stream }).to_string();

            let mut writer = BufWriter::new(
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(output_file)?,
            );
            writer.write_all(content.as_bytes())?;
            writer.flush()?;

            Ok(())
        })?;

        Ok(())
    }

    fn generate_file<P>(&self, packages: &[P]) -> Result<()>
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

        let mut writer = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create(true)
                .open(&self.output_path)?,
        );
        writer.write_all(content.as_bytes())?;
        writer.flush()?;

        Ok(())
    }
}
