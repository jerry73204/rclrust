use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Library {
    pub library_name: String,
    pub include: Vec<PathBuf>,
    pub source: Vec<PathBuf>,
}
