//! Oil.nvim-like file manager for Helix editor.
//!
//! This module provides a buffer-based file manager that allows editing the filesystem
//! like a buffer. Directory contents are displayed as text lines, and file operations
//! (create, delete, rename) are performed by editing the buffer and saving.

use std::fs;
use std::path::{Path, PathBuf};

/// Prefix for oil buffer names.
pub const OIL_BUFFER_NAME_PREFIX: &str = "[oil] ";

/// File type for oil entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OilFileType {
    File,
    Directory,
    Symlink,
}

/// A single entry in the oil buffer representing a file/directory.
#[derive(Debug, Clone)]
pub struct OilEntry {
    /// The name of the file/directory.
    pub name: String,
    /// The type of the entry.
    pub file_type: OilFileType,
    /// The original full path of the entry.
    pub original_path: PathBuf,
    /// For symlinks, the target path.
    pub symlink_target: Option<PathBuf>,
}

impl OilEntry {
    /// Create a new oil entry.
    pub fn new(name: String, file_type: OilFileType, original_path: PathBuf) -> Self {
        Self {
            name,
            file_type,
            original_path,
            symlink_target: None,
        }
    }

    /// Create a new symlink entry.
    pub fn symlink(name: String, original_path: PathBuf, target: PathBuf) -> Self {
        Self {
            name,
            file_type: OilFileType::Symlink,
            original_path,
            symlink_target: Some(target),
        }
    }
}

/// Operations that can be performed on the filesystem.
#[derive(Debug, Clone)]
pub enum OilOperation {
    /// Create a new file or directory.
    Create { path: PathBuf, is_dir: bool },
    /// Delete a file or directory.
    Delete { path: PathBuf, is_dir: bool },
    /// Rename a file or directory.
    Rename { from: PathBuf, to: PathBuf },
}

impl OilOperation {
    /// Get a human-readable description of the operation.
    pub fn description(&self) -> String {
        match self {
            OilOperation::Create { path, is_dir } => {
                let kind = if *is_dir { "directory" } else { "file" };
                format!("Create {} '{}'", kind, path.display())
            }
            OilOperation::Delete { path, is_dir } => {
                let kind = if *is_dir { "directory" } else { "file" };
                format!("Delete {} '{}'", kind, path.display())
            }
            OilOperation::Rename { from, to } => {
                format!("Rename '{}' to '{}'", from.display(), to.display())
            }
        }
    }
}

/// Sort order for oil entries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OilSortOrder {
    #[default]
    Name,
    Size,
    Modified,
}

/// State for an oil buffer.
#[derive(Debug, Clone)]
pub struct OilState {
    /// The directory being viewed.
    pub directory: PathBuf,
    /// The original entries when the buffer was created/refreshed.
    pub original_entries: Vec<OilEntry>,
    /// Whether to show hidden files.
    pub show_hidden: bool,
    /// Sort order for entries.
    pub sort_order: OilSortOrder,
}

impl OilState {
    /// Create a new oil state for a directory.
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            original_entries: Vec::new(),
            show_hidden: false,
            sort_order: OilSortOrder::default(),
        }
    }

    /// Get the buffer name for this oil state.
    pub fn buffer_name(&self) -> String {
        format!("{}{}", OIL_BUFFER_NAME_PREFIX, self.directory.display())
    }
}

/// A parsed entry from an oil buffer line.
#[derive(Debug, Clone)]
pub struct ParsedEntry {
    /// The name of the entry.
    pub name: String,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Whether this is a symlink.
    pub is_symlink: bool,
}

/// Read directory entries from the filesystem.
pub fn read_directory_entries(
    path: &Path,
    show_hidden: bool,
    sort_order: OilSortOrder,
) -> std::io::Result<Vec<OilEntry>> {
    let mut entries = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();

        // Skip hidden files if not showing them
        if !show_hidden && name.starts_with('.') {
            continue;
        }

        let path = entry.path();
        let metadata = entry.metadata()?;

        let (file_type, symlink_target) = if metadata.is_symlink() {
            let target = fs::read_link(&path).ok();
            (OilFileType::Symlink, target)
        } else if metadata.is_dir() {
            (OilFileType::Directory, None)
        } else {
            (OilFileType::File, None)
        };

        let mut oil_entry = OilEntry::new(name, file_type.clone(), path);
        if file_type == OilFileType::Symlink {
            oil_entry.symlink_target = symlink_target;
        }
        entries.push(oil_entry);
    }

    // Sort entries
    match sort_order {
        OilSortOrder::Name => {
            entries.sort_by(|a, b| {
                // Directories first, then files
                match (&a.file_type, &b.file_type) {
                    (OilFileType::Directory, OilFileType::Directory) => {
                        a.name.to_lowercase().cmp(&b.name.to_lowercase())
                    }
                    (OilFileType::Directory, _) => std::cmp::Ordering::Less,
                    (_, OilFileType::Directory) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }
            });
        }
        OilSortOrder::Size => {
            // For size sorting, we'd need to get file sizes
            // For now, fall back to name sorting
            entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        OilSortOrder::Modified => {
            // For modified time sorting, we'd need to get modification times
            // For now, fall back to name sorting
            entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
    }

    Ok(entries)
}

/// Generate the content for an oil buffer.
pub fn generate_oil_buffer_content(directory: &Path, entries: &[OilEntry]) -> String {
    let mut content = String::new();

    // Header comments
    content.push_str(&format!("# {}\n", directory.display()));
    content.push_str("# d=directory -=file l=symlink | Enter:open -:parent :w:apply\n");
    content.push('\n');

    // Entries
    for entry in entries {
        let line = format_oil_entry(entry);
        content.push_str(&line);
        content.push('\n');
    }

    content
}

/// Format a single oil entry as a line.
fn format_oil_entry(entry: &OilEntry) -> String {
    match &entry.file_type {
        OilFileType::Directory => format!("d {}/", entry.name),
        OilFileType::File => format!("- {}", entry.name),
        OilFileType::Symlink => {
            let target = entry
                .symlink_target
                .as_ref()
                .map(|t| t.to_string_lossy().into_owned())
                .unwrap_or_else(|| "?".to_string());
            format!("l {} -> {}", entry.name, target)
        }
    }
}

/// Parse a single line from an oil buffer.
/// Returns None if the line is a comment or empty.
pub fn parse_oil_line(line: &str) -> Option<ParsedEntry> {
    let line = line.trim();

    // Skip empty lines and comments
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // Parse the line based on prefix
    if let Some(rest) = line.strip_prefix("d ") {
        // Directory
        let name = rest.trim_end_matches('/').to_string();
        if name.is_empty() {
            return None;
        }
        Some(ParsedEntry {
            name,
            is_dir: true,
            is_symlink: false,
        })
    } else if let Some(rest) = line.strip_prefix("- ") {
        // File
        let name = rest.to_string();
        if name.is_empty() {
            return None;
        }
        Some(ParsedEntry {
            name,
            is_dir: false,
            is_symlink: false,
        })
    } else if let Some(rest) = line.strip_prefix("l ") {
        // Symlink - format is "name -> target"
        // We only care about the name part for operations
        let name = if let Some(arrow_pos) = rest.find(" -> ") {
            rest[..arrow_pos].to_string()
        } else {
            rest.to_string()
        };
        if name.is_empty() {
            return None;
        }
        Some(ParsedEntry {
            name,
            is_dir: false,
            is_symlink: true,
        })
    } else {
        None
    }
}

/// Compute the operations needed to transform from original to current state.
pub fn compute_oil_operations(
    original_entries: &[OilEntry],
    current_content: &str,
    directory: &Path,
) -> Vec<OilOperation> {
    use std::collections::HashMap;

    let mut operations = Vec::new();

    // Parse current content into entries
    let current_entries: Vec<ParsedEntry> = current_content
        .lines()
        .filter_map(parse_oil_line)
        .collect();

    // Build a map of original entries by name
    let original_map: HashMap<&str, &OilEntry> = original_entries
        .iter()
        .map(|e| (e.name.as_str(), e))
        .collect();

    // Build a set of current entry names
    let current_names: std::collections::HashSet<&str> =
        current_entries.iter().map(|e| e.name.as_str()).collect();

    // Find deletions: entries in original but not in current
    for original in original_entries {
        if !current_names.contains(original.name.as_str()) {
            let is_dir = original.file_type == OilFileType::Directory;
            operations.push(OilOperation::Delete {
                path: original.original_path.clone(),
                is_dir,
            });
        }
    }

    // Find creations and renames
    for current in &current_entries {
        if let Some(original) = original_map.get(current.name.as_str()) {
            // Entry exists with same name - check if type changed
            // For now, we don't handle type changes
            let _original = original; // suppress unused warning
        } else {
            // New entry - could be a creation or a rename
            // For simplicity, we treat it as a creation
            // A more sophisticated implementation could detect renames
            // by looking for entries with the same content but different names
            let path = directory.join(&current.name);
            operations.push(OilOperation::Create {
                path,
                is_dir: current.is_dir,
            });
        }
    }

    // Simple rename detection: if we have exactly one deletion and one creation
    // with the same type, treat it as a rename
    let deletions: Vec<_> = operations
        .iter()
        .filter(|op| matches!(op, OilOperation::Delete { .. }))
        .collect();
    let creations: Vec<_> = operations
        .iter()
        .filter(|op| matches!(op, OilOperation::Create { .. }))
        .collect();

    if deletions.len() == 1 && creations.len() == 1 {
        if let (
            OilOperation::Delete {
                path: from,
                is_dir: del_is_dir,
            },
            OilOperation::Create {
                path: to,
                is_dir: create_is_dir,
            },
        ) = (deletions[0], creations[0])
        {
            if del_is_dir == create_is_dir {
                // Replace with a rename operation
                return vec![OilOperation::Rename {
                    from: from.clone(),
                    to: to.clone(),
                }];
            }
        }
    }

    operations
}

/// Execute a single oil operation.
pub fn execute_oil_operation(operation: &OilOperation) -> std::io::Result<()> {
    match operation {
        OilOperation::Create { path, is_dir } => {
            if *is_dir {
                fs::create_dir_all(path)?;
            } else {
                // Create parent directories if needed
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::File::create(path)?;
            }
        }
        OilOperation::Delete { path, is_dir } => {
            if *is_dir {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }
        OilOperation::Rename { from, to } => {
            fs::rename(from, to)?;
        }
    }
    Ok(())
}

/// Execute all oil operations.
/// Returns Ok(count) with the number of successful operations, or Err on first failure.
pub fn execute_oil_operations(operations: &[OilOperation]) -> Result<usize, (usize, std::io::Error)> {
    for (idx, op) in operations.iter().enumerate() {
        if let Err(e) = execute_oil_operation(op) {
            return Err((idx, e));
        }
    }
    Ok(operations.len())
}

/// Check if a buffer name indicates an oil buffer.
pub fn is_oil_buffer_name(name: &str) -> bool {
    name.starts_with(OIL_BUFFER_NAME_PREFIX)
}

/// Extract the directory path from an oil buffer name.
pub fn directory_from_buffer_name(name: &str) -> Option<PathBuf> {
    name.strip_prefix(OIL_BUFFER_NAME_PREFIX)
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_oil_line_file() {
        let parsed = parse_oil_line("- README.md").unwrap();
        assert_eq!(parsed.name, "README.md");
        assert!(!parsed.is_dir);
        assert!(!parsed.is_symlink);
    }

    #[test]
    fn test_parse_oil_line_directory() {
        let parsed = parse_oil_line("d src/").unwrap();
        assert_eq!(parsed.name, "src");
        assert!(parsed.is_dir);
        assert!(!parsed.is_symlink);
    }

    #[test]
    fn test_parse_oil_line_symlink() {
        let parsed = parse_oil_line("l config -> /etc/config").unwrap();
        assert_eq!(parsed.name, "config");
        assert!(!parsed.is_dir);
        assert!(parsed.is_symlink);
    }

    #[test]
    fn test_parse_oil_line_comment() {
        assert!(parse_oil_line("# This is a comment").is_none());
        assert!(parse_oil_line("").is_none());
        assert!(parse_oil_line("   ").is_none());
    }

    #[test]
    fn test_format_oil_entry_file() {
        let entry = OilEntry::new(
            "test.txt".to_string(),
            OilFileType::File,
            PathBuf::from("/tmp/test.txt"),
        );
        assert_eq!(format_oil_entry(&entry), "- test.txt");
    }

    #[test]
    fn test_format_oil_entry_directory() {
        let entry = OilEntry::new(
            "src".to_string(),
            OilFileType::Directory,
            PathBuf::from("/tmp/src"),
        );
        assert_eq!(format_oil_entry(&entry), "d src/");
    }

    #[test]
    fn test_compute_operations_create() {
        let original = vec![];
        let current = "- newfile.txt\n";
        let ops = compute_oil_operations(&original, current, Path::new("/tmp"));
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], OilOperation::Create { is_dir: false, .. }));
    }

    #[test]
    fn test_compute_operations_delete() {
        let original = vec![OilEntry::new(
            "oldfile.txt".to_string(),
            OilFileType::File,
            PathBuf::from("/tmp/oldfile.txt"),
        )];
        let current = "";
        let ops = compute_oil_operations(&original, current, Path::new("/tmp"));
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], OilOperation::Delete { is_dir: false, .. }));
    }

    #[test]
    fn test_compute_operations_rename() {
        let original = vec![OilEntry::new(
            "old.txt".to_string(),
            OilFileType::File,
            PathBuf::from("/tmp/old.txt"),
        )];
        let current = "- new.txt\n";
        let ops = compute_oil_operations(&original, current, Path::new("/tmp"));
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], OilOperation::Rename { .. }));
    }

    #[test]
    fn test_is_oil_buffer_name() {
        assert!(is_oil_buffer_name("[oil] /home/user"));
        assert!(!is_oil_buffer_name("[quickfix]"));
        assert!(!is_oil_buffer_name("test.txt"));
    }

    #[test]
    fn test_directory_from_buffer_name() {
        let dir = directory_from_buffer_name("[oil] /home/user").unwrap();
        assert_eq!(dir, PathBuf::from("/home/user"));
    }
}
