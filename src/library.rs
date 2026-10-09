use crate::chart::{SongHeader, parse_header};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub header: SongHeader,
    pub search: String,
}
#[derive(Default)]
pub struct Library {
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}

pub fn scan(root: &Path) -> Result<Library, String> {
    let mut library = Library::default();
    if !root.exists() {
        return Ok(library);
    }
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let files = std::fs::read_dir(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        for file in files {
            let file = match file {
                Ok(file) => file,
                Err(e) => {
                    library.warnings.push(e.to_string());
                    continue;
                }
            };
            let kind = file.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() {
                folders.push(file.path());
                continue;
            }
            let path = file.path();
            if !kind.is_file()
                || !path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("ojn"))
            {
                continue;
            }
            let result = (|| {
                let mut header = [0u8; 300];
                File::open(&path)
                    .map_err(|e| e.to_string())?
                    .read_exact(&mut header)
                    .map_err(|e| e.to_string())?;
                parse_header(&header)
            })();
            match result {
                Ok(mut header) => {
                    if header.title.is_empty() {
                        header.title = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                    }
                    let search = format!("{} {} {}", header.title, header.artist, path.display())
                        .to_lowercase();
                    library.entries.push(Entry {
                        path,
                        header,
                        search,
                    });
                }
                Err(e) => library.warnings.push(format!("{}: {e}", path.display())),
            }
        }
    }
    library.entries.sort_by(|a, b| {
        a.header
            .title
            .to_lowercase()
            .cmp(&b.header.title.to_lowercase())
            .then(a.path.cmp(&b.path))
    });
    Ok(library)
}

impl Library {
    pub fn filter(&self, query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.search.contains(&query).then_some(i))
            .collect()
    }
}
