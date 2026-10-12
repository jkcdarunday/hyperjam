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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortColumn {
    #[default]
    Title,
    Artist,
    Id,
    Bpm,
    Easy,
    Normal,
    Hard,
}
impl SortColumn {
    pub fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Artist => "artist",
            Self::Id => "id",
            Self::Bpm => "bpm",
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "title" => Ok(Self::Title),
            "artist" => Ok(Self::Artist),
            "id" => Ok(Self::Id),
            "bpm" => Ok(Self::Bpm),
            "easy" => Ok(Self::Easy),
            "normal" => Ok(Self::Normal),
            "hard" => Ok(Self::Hard),
            _ => Err("expected title, artist, id, bpm, easy, normal, or hard".into()),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LibrarySort {
    pub column: SortColumn,
    pub descending: bool,
}
impl LibrarySort {
    pub fn choose(self, column: SortColumn) -> Self {
        Self {
            column,
            descending: self.column == column && !self.descending,
        }
    }
}

pub fn scan(root: &Path) -> Result<Library, String> {
    let mut library = Library::default();
    if !root.exists() {
        return Ok(library);
    }
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let files = match std::fs::read_dir(&folder) {
            Ok(files) => files,
            Err(error) if folder == root => return Err(format!("{}: {error}", folder.display())),
            Err(error) => {
                library
                    .warnings
                    .push(format!("{}: {error}", folder.display()));
                continue;
            }
        };
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
                    let search = format!(
                        "{} {} {} {}",
                        header.title,
                        header.artist,
                        header.id,
                        path.display()
                    )
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
    library.sort(LibrarySort::default());
    Ok(library)
}

impl Library {
    pub fn sort(&mut self, sort: LibrarySort) {
        self.entries.sort_by_cached_key(|entry| {
            let h = &entry.header;
            (h.title.to_lowercase(), h.id, entry.path.clone())
        });
        // Stable primary sorting preserves deterministic title/ID/path ties.
        match sort.column {
            SortColumn::Title => self
                .entries
                .sort_by_cached_key(|e| e.header.title.to_lowercase()),
            SortColumn::Artist => self
                .entries
                .sort_by_cached_key(|e| e.header.artist.to_lowercase()),
            SortColumn::Id => self.entries.sort_by_key(|e| e.header.id),
            SortColumn::Bpm => self
                .entries
                .sort_by(|a, b| a.header.bpm.total_cmp(&b.header.bpm)),
            SortColumn::Easy => self.entries.sort_by_key(|e| e.header.levels[0]),
            SortColumn::Normal => self.entries.sort_by_key(|e| e.header.levels[1]),
            SortColumn::Hard => self.entries.sort_by_key(|e| e.header.levels[2]),
        }
        if sort.descending {
            self.entries.reverse();
        }
    }
    pub fn filter(&self, query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.search.contains(&query).then_some(i))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(id: u32, name: &str, artist: &str, bpm: f64, levels: [u16; 3]) -> Entry {
        let path = PathBuf::from(format!("{id}.ojn"));
        let header = SongHeader {
            id,
            title: name.into(),
            artist: artist.into(),
            noter: String::new(),
            sample_file: String::new(),
            bpm,
            levels,
        };
        let search = format!("{name} {artist} {id} {}", path.display()).to_lowercase();
        Entry {
            path,
            header,
            search,
        }
    }
    fn library() -> Library {
        Library {
            entries: vec![
                entry(10, "Zulu", "Bravo", 100.0, [1, 6, 10]),
                entry(2, "alpha", "charlie", 180.0, [3, 5, 2]),
                entry(100, "Beta", "Alpha", 120.0, [2, 4, 8]),
            ],
            warnings: vec![],
        }
    }
    #[test]
    fn all_columns_sort_correctly_in_both_directions() {
        for (column, expected) in [
            (SortColumn::Title, [2, 100, 10]),
            (SortColumn::Artist, [100, 10, 2]),
            (SortColumn::Id, [2, 10, 100]),
            (SortColumn::Bpm, [10, 100, 2]),
            (SortColumn::Easy, [10, 100, 2]),
            (SortColumn::Normal, [100, 2, 10]),
            (SortColumn::Hard, [2, 100, 10]),
        ] {
            let mut library = library();
            library.sort(LibrarySort {
                column,
                descending: false,
            });
            assert_eq!(
                library
                    .entries
                    .iter()
                    .map(|e| e.header.id)
                    .collect::<Vec<_>>(),
                expected,
                "{column:?}"
            );
            library.sort(LibrarySort {
                column,
                descending: true,
            });
            assert_eq!(
                library
                    .entries
                    .iter()
                    .map(|e| e.header.id)
                    .collect::<Vec<_>>(),
                expected.into_iter().rev().collect::<Vec<_>>(),
                "{column:?}"
            );
        }
    }
    #[test]
    fn name_filter_keeps_the_current_sort_order_and_handles_no_matches() {
        let mut library = library();
        library.sort(LibrarySort {
            column: SortColumn::Id,
            descending: true,
        });
        assert_eq!(
            library
                .filter("ALPHA")
                .iter()
                .map(|&i| library.entries[i].header.id)
                .collect::<Vec<_>>(),
            [100, 2]
        );
        assert_eq!(library.filter("zUlU"), [1]);
        assert!(library.filter("nonexistent song").is_empty());
        assert_eq!(library.filter("100"), [0]);
    }
    #[test]
    fn repeated_headers_toggle_direction_new_columns_start_ascending() {
        let sort = LibrarySort::default().choose(SortColumn::Hard);
        assert!(!sort.descending);
        let sort = sort.choose(SortColumn::Hard);
        assert!(sort.descending);
        assert!(!sort.choose(SortColumn::Id).descending);
        for column in [
            SortColumn::Title,
            SortColumn::Artist,
            SortColumn::Id,
            SortColumn::Bpm,
            SortColumn::Easy,
            SortColumn::Normal,
            SortColumn::Hard,
        ] {
            assert_eq!(SortColumn::parse(column.name()).unwrap(), column);
        }
    }
}
