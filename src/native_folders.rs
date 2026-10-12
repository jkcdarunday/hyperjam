use std::path::{Path, PathBuf};

pub enum FolderResult {
    Selected(PathBuf),
    Cancelled,
    Unavailable(String),
}

pub struct NativeFolderPicker {
    #[cfg(target_os = "linux")]
    child: std::process::Child,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    result: std::sync::mpsc::Receiver<Option<PathBuf>>,
}

impl NativeFolderPicker {
    pub fn open(start: &Path) -> Result<Self, String> {
        let directory = std::fs::canonicalize(start)
            .ok()
            .filter(|path| path.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        #[cfg(target_os = "linux")]
        {
            use std::process::{Command, Stdio};
            let kde = std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_default()
                .to_uppercase()
                .contains("KDE");
            let programs = if kde {
                ["kdialog", "zenity"]
            } else {
                ["zenity", "kdialog"]
            };
            for program in programs {
                let mut command = Command::new(program);
                if program == "kdialog" {
                    command
                        .arg("--title")
                        .arg("Choose music folder")
                        .arg("--getexistingdirectory")
                        .arg(&directory);
                } else {
                    command
                        .arg("--file-selection")
                        .arg("--directory")
                        .arg("--title=Choose music folder")
                        .arg("--filename")
                        .arg(directory.join(""));
                }
                match command.stdout(Stdio::piped()).stderr(Stdio::null()).spawn() {
                    Ok(child) => return Ok(Self { child }),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(format!("Could not open {program}: {error}")),
                }
            }
            Err("Install kdialog or zenity to use the system folder chooser.".into())
        }
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let (sender, result) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let folder = rfd::FileDialog::new()
                    .set_title("Choose music folder")
                    .set_directory(directory)
                    .pick_folder();
                let _ = sender.send(folder);
            });
            Ok(Self { result })
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            let _ = directory;
            Err("No system folder chooser is available on this platform.".into())
        }
    }

    pub fn poll(&mut self) -> Option<FolderResult> {
        #[cfg(target_os = "linux")]
        {
            use std::io::Read;
            match self.child.try_wait() {
                Ok(None) => None,
                Ok(Some(status)) => {
                    let mut bytes = vec![];
                    if status.success()
                        && let Some(stdout) = self.child.stdout.take()
                        && let Err(error) = stdout.take(65_537).read_to_end(&mut bytes)
                    {
                        return Some(FolderResult::Unavailable(format!(
                            "Could not read chosen folder: {error}"
                        )));
                    }
                    Some(dialog_result(status.success(), status.code(), bytes))
                }
                Err(error) => Some(FolderResult::Unavailable(format!(
                    "System chooser failed: {error}"
                ))),
            }
        }
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            match self.result.try_recv() {
                Ok(Some(folder)) => Some(FolderResult::Selected(folder)),
                Ok(None) => Some(FolderResult::Cancelled),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(_) => Some(FolderResult::Unavailable(
                    "System chooser stopped unexpectedly.".into(),
                )),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Some(FolderResult::Unavailable(
                "No system chooser available.".into(),
            ))
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for NativeFolderPicker {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(target_os = "linux")]
fn dialog_result(success: bool, code: Option<i32>, mut bytes: Vec<u8>) -> FolderResult {
    use std::os::unix::ffi::OsStringExt;
    if !success {
        return if code == Some(1) {
            FolderResult::Cancelled
        } else {
            FolderResult::Unavailable(format!("System chooser exited with status {code:?}."))
        };
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    if bytes.is_empty() || bytes.len() > 65_536 || bytes.contains(&0) {
        return FolderResult::Unavailable("System chooser returned an invalid path.".into());
    }
    FolderResult::Selected(std::ffi::OsString::from_vec(bytes).into())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn cancelled_dialog_is_not_an_unavailable_dialog() {
        assert!(matches!(
            dialog_result(false, Some(1), vec![]),
            FolderResult::Cancelled
        ));
        assert!(matches!(
            dialog_result(false, Some(2), vec![]),
            FolderResult::Unavailable(_)
        ));
        assert!(matches!(
            dialog_result(true, Some(0), vec![]),
            FolderResult::Unavailable(_)
        ));
    }
    #[test]
    fn chosen_paths_preserve_spaces_and_shell_characters() {
        let bytes = b"/tmp/music collection/$test; $(example) \n".to_vec();
        match dialog_result(true, Some(0), bytes) {
            FolderResult::Selected(path) => assert_eq!(
                path,
                PathBuf::from("/tmp/music collection/$test; $(example) ")
            ),
            _ => panic!("expected a selected folder"),
        }
    }
}
