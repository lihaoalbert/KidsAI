// W10/W11 共享底座 — TrustedStorage
//
// 原子写 + 权限收紧: skills/ 和 secrets/ 复用同一组原语, 防半写崩溃 / 越权读取.
//
// 设计:
// - write_atomic: tmp → fsync → chmod 600 → rename (POSIX 原子)
// - read_bytes:   静默返回 None (文件不存在), 显式区分「不存在」vs「读失败」
// - sha256_file:  读全文 + sha256, 给 skills_verifier 复用
// - ensure_dir:   mkdir -p, 不存在才创建, 幂等
//
// Linux/macOS 都走 POSIX rename (atomic on same filesystem), 跨平台一致性.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub struct TrustedStorage {
    root: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("path escapes root: {0}")]
    PathEscape(String),
    #[error("create dir: {0}")]
    CreateDir(String),
}

impl TrustedStorage {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 解析「相对 root 的子路径」, 防止 `../` 越权写到 root 之外.
    fn resolve(&self, relative: &Path) -> Result<PathBuf, StorageError> {
        let rel = relative.to_path_buf();
        if rel
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(StorageError::PathEscape(rel.display().to_string()));
        }
        Ok(self.root.join(rel))
    }

    pub fn ensure_dir(&self, relative: &Path) -> Result<(), StorageError> {
        let p = self.resolve(relative)?;
        fs::create_dir_all(&p).map_err(|e| StorageError::CreateDir(e.to_string()))?;
        Ok(())
    }

    /// 原子写: 写 tmp → fsync → chmod 600 (Unix) → rename.
    /// 任一步失败, tmp 留待下次覆盖, 不污染正式文件.
    pub fn write_atomic(&self, relative: &Path, bytes: &[u8]) -> Result<(), StorageError> {
        let target = self.resolve(relative)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| StorageError::CreateDir(e.to_string()))?;
        }
        let tmp = target.with_extension(format!(
            "{}.tmp",
            target.extension().and_then(|s| s.to_str()).unwrap_or("bin")
        ));
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &target)?;
        Ok(())
    }

    pub fn read_bytes(&self, relative: &Path) -> Result<Option<Vec<u8>>, StorageError> {
        let p = self.resolve(relative)?;
        match fs::read(&p) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StorageError::Io(e)),
        }
    }

    pub fn read_string(&self, relative: &Path) -> Result<Option<String>, StorageError> {
        match self.read_bytes(relative)? {
            Some(b) => Ok(Some(
                String::from_utf8(b).map_err(|e| StorageError::Io(std::io::Error::other(e)))?,
            )),
            None => Ok(None),
        }
    }

    pub fn sha256_file(&self, relative: &Path) -> Result<Option<String>, StorageError> {
        match self.read_bytes(relative)? {
            Some(b) => Ok(Some(hex_sha256(&b))),
            None => Ok(None),
        }
    }

    pub fn exists(&self, relative: &Path) -> bool {
        self.resolve(relative)
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    pub fn remove(&self, relative: &Path) -> Result<(), StorageError> {
        let p = self.resolve(relative)?;
        match fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StorageError::Io(e)),
        }
    }

    /// 递归删除子目录 (用于 skill 卸载); 不允许删 root 自身.
    pub fn remove_dir_all(&self, relative: &Path) -> Result<(), StorageError> {
        let p = self.resolve(relative)?;
        if p == self.root {
            return Err(StorageError::PathEscape("refuse to remove root".into()));
        }
        match fs::remove_dir_all(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StorageError::Io(e)),
        }
    }
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_atomic_then_read_roundtrip() {
        let dir = tempdir().unwrap();
        let s = TrustedStorage::new(dir.path());
        s.ensure_dir(Path::new("skills/eng")).unwrap();
        s.write_atomic(Path::new("skills/eng/manifest.json"), b"{\"id\":\"eng\"}")
            .unwrap();
        let got = s.read_bytes(Path::new("skills/eng/manifest.json")).unwrap();
        assert_eq!(got, Some(b"{\"id\":\"eng\"}".to_vec()));
    }

    #[test]
    fn read_missing_returns_none_not_err() {
        let dir = tempdir().unwrap();
        let s = TrustedStorage::new(dir.path());
        let got = s.read_bytes(Path::new("nope.json")).unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn overwrite_does_not_leave_tmp() {
        let dir = tempdir().unwrap();
        let s = TrustedStorage::new(dir.path());
        s.write_atomic(Path::new("a.bin"), b"v1").unwrap();
        s.write_atomic(Path::new("a.bin"), b"v2-longer").unwrap();
        let got = s.read_bytes(Path::new("a.bin")).unwrap().unwrap();
        assert_eq!(got, b"v2-longer");
        // 没残留 tmp
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(!entries.iter().any(|n| n.ends_with(".tmp")), "tmp leak: {entries:?}");
    }

    #[test]
    fn refuses_parent_dir_escape() {
        let dir = tempdir().unwrap();
        let s = TrustedStorage::new(dir.path());
        let bad = Path::new("../escape.txt");
        assert!(s.write_atomic(bad, b"x").is_err());
        assert!(s.read_bytes(bad).is_err());
    }

    #[test]
    fn sha256_file_matches_hex_sha256() {
        let dir = tempdir().unwrap();
        let s = TrustedStorage::new(dir.path());
        s.write_atomic(Path::new("f.bin"), b"hello").unwrap();
        let h1 = s.sha256_file(Path::new("f.bin")).unwrap().unwrap();
        let h2 = hex_sha256(b"hello");
        assert_eq!(h1, h2);
        assert_eq!(
            h1,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn remove_dir_all_blocks_root() {
        let dir = tempdir().unwrap();
        let s = TrustedStorage::new(dir.path());
        assert!(s.remove_dir_all(Path::new(".")).is_err());
    }

    #[test]
    fn ensure_dir_is_idempotent() {
        let dir = tempdir().unwrap();
        let s = TrustedStorage::new(dir.path());
        s.ensure_dir(Path::new("a/b/c")).unwrap();
        s.ensure_dir(Path::new("a/b/c")).unwrap();
        assert!(s.exists(Path::new("a/b/c")));
    }
}