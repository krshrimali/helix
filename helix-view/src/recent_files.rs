use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

const MAX_ENTRIES: usize = 100;
const RECENT_FILES_FILENAME: &str = "recent_files";

/// Tracks recently opened files, persisted across sessions.
#[derive(Debug)]
pub struct RecentFiles {
    files: Vec<PathBuf>,
}

impl Default for RecentFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl RecentFiles {
    /// Create an empty RecentFiles instance.
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Load recent files from the cache directory.
    pub fn load() -> Self {
        let path = Self::file_path();
        let files = match fs::File::open(&path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                reader
                    .lines()
                    .map_while(Result::ok)
                    .map(PathBuf::from)
                    .collect()
            }
            Err(_) => Vec::new(),
        };
        Self { files }
    }

    /// Save recent files to the cache directory.
    pub fn save(&self) {
        let path = Self::file_path();

        // Ensure cache directory exists
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                log::error!("Failed to create cache directory: {}", e);
                return;
            }
        }

        match fs::File::create(&path) {
            Ok(mut file) => {
                for p in &self.files {
                    if let Err(e) = writeln!(file, "{}", p.display()) {
                        log::error!("Failed to write recent file entry: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to create recent files cache: {}", e);
            }
        }
    }

    /// Add a file to the recent files list.
    /// If the file already exists, it is moved to the front.
    /// The list is trimmed to MAX_ENTRIES.
    pub fn push(&mut self, path: PathBuf) {
        // Remove if already present
        self.files.retain(|p| p != &path);
        // Insert at front
        self.files.insert(0, path);
        // Trim to max entries
        self.files.truncate(MAX_ENTRIES);
    }

    /// Iterate over recent files (most recent first).
    pub fn iter(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.iter()
    }

    /// Get the path to the recent files cache file.
    fn file_path() -> PathBuf {
        helix_loader::cache_dir().join(RECENT_FILES_FILENAME)
    }
}
