use std::path::{Path, PathBuf};

pub struct FolderBrowser {
    pub directory: PathBuf,
    pub folders: Vec<PathBuf>,
    pub input: String,
    pub selected: usize,
    pub replace_input: bool,
    pub error: Option<String>,
}

pub fn readable_folder(path: &Path) -> Result<PathBuf, String> {
    let path = std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !path.is_dir() {
        return Err("Choose a folder, not a file.".into());
    }
    std::fs::read_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

impl FolderBrowser {
    pub fn new(start: &Path) -> Self {
        let mut browser = Self {
            directory: PathBuf::new(),
            folders: vec![],
            input: String::new(),
            selected: 0,
            replace_input: false,
            error: None,
        };
        let start = if start.is_dir() {
            start.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        if let Err(error) = browser.navigate(&start) {
            browser.input = start.to_string_lossy().into_owned();
            browser.error = Some(error);
        }
        browser
    }
    pub fn navigate(&mut self, path: &Path) -> Result<(), String> {
        let directory = readable_folder(path)?;
        let mut folders: Vec<_> = std::fs::read_dir(&directory)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        folders.sort_by_cached_key(|path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        });
        self.input = directory.to_string_lossy().into_owned();
        self.directory = directory;
        self.folders = folders;
        self.selected = 0;
        self.replace_input = false;
        self.error = None;
        Ok(())
    }
    pub fn open_input(&mut self) {
        let path = PathBuf::from(self.input.trim().trim_matches('"'));
        if let Err(error) = self.navigate(&path) {
            self.error = Some(error);
        }
    }
    pub fn parent(&mut self) {
        if let Some(parent) = self.directory.parent().map(Path::to_path_buf)
            && let Err(error) = self.navigate(&parent)
        {
            self.error = Some(error);
        }
    }
    pub fn home(&mut self) {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
            && let Err(error) = self.navigate(Path::new(&home))
        {
            self.error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_lists_folders_and_keeps_selection_on_errors() {
        let root = std::env::temp_dir().join(format!("hyperjam-folders-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Zeta")).unwrap();
        std::fs::create_dir_all(root.join("alpha music")).unwrap();
        std::fs::write(root.join("chart.ojn"), b"test").unwrap();
        let mut browser = FolderBrowser::new(&root);
        assert_eq!(browser.folders.len(), 2);
        assert_eq!(browser.folders[0].file_name().unwrap(), "alpha music");
        let original = browser.directory.clone();
        assert!(browser.navigate(&root.join("missing")).is_err());
        assert_eq!(browser.directory, original);
        assert!(readable_folder(&root.join("chart.ojn")).is_err());
        browser.navigate(&root.join("alpha music")).unwrap();
        assert!(browser.folders.is_empty());
        browser.parent();
        assert_eq!(browser.directory, original);
        std::fs::remove_dir_all(root).unwrap();
    }
}
