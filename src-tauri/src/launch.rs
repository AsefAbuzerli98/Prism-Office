//! Files handed to us by the OS: command-line arguments (Windows/Linux file
//! association, second instance) and `RunEvent::Opened` (macOS).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use prism_openxml::DocumentKind;

/// Extract openable document paths from argv (skipping the program name and
/// flags), resolving relative paths against `cwd`.
pub fn document_args<I, S>(args: I, cwd: &Path) -> Vec<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .skip(1)
        .filter_map(|a| {
            let a = a.as_ref();
            if a.starts_with('-') {
                return None;
            }
            let p = Path::new(a);
            let ext = p.extension()?.to_str()?;
            DocumentKind::from_extension(ext)?;
            Some(if p.is_absolute() {
                p.to_path_buf()
            } else {
                cwd.join(p)
            })
        })
        .collect()
}

/// Queue of paths received before the UI was ready to take them.
#[derive(Default)]
pub struct PendingOpens(Mutex<Vec<String>>);

impl PendingOpens {
    pub fn push(&self, paths: impl IntoIterator<Item = PathBuf>) {
        self.0
            .lock()
            .unwrap()
            .extend(paths.into_iter().map(|p| p.to_string_lossy().into_owned()));
    }

    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_and_resolves() {
        let cwd = Path::new("/home/u");
        let got = document_args(
            [
                "prism",
                "--flag",
                "a.docx",
                "/abs/b.XLSX",
                "notes.txt",
                "noext",
            ],
            cwd,
        );
        assert_eq!(
            got,
            vec![
                PathBuf::from("/home/u/a.docx"),
                PathBuf::from("/abs/b.XLSX")
            ]
        );
        let q = PendingOpens::default();
        q.push(got);
        assert_eq!(q.take().len(), 2);
        assert!(q.take().is_empty());
    }
}
