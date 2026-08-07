//! 파일시스템 포트 — 코어 로직이 실제 디스크에 의존하지 않게 한다.

use std::io;
use std::path::Path;

pub trait FileSystem: Send + Sync {
    fn exists(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn len(&self, path: &Path) -> Option<u64>;
    /// 파일 앞부분 최대 `n` 바이트를 읽는다 (헤더 판별용).
    fn read_prefix(&self, path: &Path, n: usize) -> io::Result<Vec<u8>>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
}

/// 실제 디스크.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFs;

impl FileSystem for RealFs {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn len(&self, path: &Path) -> Option<u64> {
        std::fs::metadata(path).ok().map(|m| m.len())
    }

    fn read_prefix(&self, path: &Path, n: usize) -> io::Result<Vec<u8>> {
        use std::io::Read;

        let mut file = std::fs::File::open(path)?;
        let mut buffer = vec![0u8; n];
        let read = file.read(&mut buffer)?;
        buffer.truncate(read);

        Ok(buffer)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }
}

/// 테스트용 인메모리 파일시스템.
#[cfg(test)]
pub mod fake {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct FakeFs {
        files: Mutex<BTreeMap<PathBuf, Vec<u8>>>,
        dirs: Mutex<BTreeSet<PathBuf>>,
    }

    impl FakeFs {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_file(self, path: impl Into<PathBuf>, contents: impl Into<Vec<u8>>) -> Self {
            self.files
                .lock()
                .unwrap()
                .insert(path.into(), contents.into());
            self
        }

        pub fn with_dir(self, path: impl Into<PathBuf>) -> Self {
            self.dirs.lock().unwrap().insert(path.into());
            self
        }
    }

    impl FileSystem for FakeFs {
        fn exists(&self, path: &Path) -> bool {
            self.is_file(path) || self.is_dir(path)
        }

        fn is_file(&self, path: &Path) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }

        fn is_dir(&self, path: &Path) -> bool {
            self.dirs.lock().unwrap().contains(path)
        }

        fn len(&self, path: &Path) -> Option<u64> {
            self.files.lock().unwrap().get(path).map(|b| b.len() as u64)
        }

        fn read_prefix(&self, path: &Path, n: usize) -> io::Result<Vec<u8>> {
            let files = self.files.lock().unwrap();
            let bytes = files
                .get(path)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such fake file"))?;

            Ok(bytes.iter().take(n).copied().collect())
        }

        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            self.dirs.lock().unwrap().insert(path.to_path_buf());
            Ok(())
        }

        fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
            self.dirs.lock().unwrap().remove(path);
            self.files
                .lock()
                .unwrap()
                .retain(|p, _| !p.starts_with(path));
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> io::Result<()> {
            self.files.lock().unwrap().remove(path);
            Ok(())
        }

        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            let mut files = self.files.lock().unwrap();
            let bytes = files
                .remove(from)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such fake file"))?;
            files.insert(to.to_path_buf(), bytes);
            Ok(())
        }
    }
}
