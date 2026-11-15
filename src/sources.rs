//! Port of `sources.py`: where program text comes from.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::syntax::OrderedMap;

/// Port of `sources.py::SourceProvider`.
pub trait SourceProvider {
    /// Port of `sources.py::SourceProvider.read`.
    fn read(&self, path: &Path) -> Option<Vec<u8>>;

    /// Port of `sources.py::SourceProvider.list_dir`.
    fn list_dir(&self, directory: &Path) -> Option<Vec<String>>;

    fn list_subdirs(&self, _directory: &Path) -> Option<Vec<String>> {
        None
    }
}

/// Port of `sources.py::DiskSource`.
#[derive(Debug, Default, Clone, Copy)]
pub struct DiskSource;

impl SourceProvider for DiskSource {
    /// Port of `sources.py::DiskSource.read`.
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        fs::read(path).ok()
    }

    /// Port of `sources.py::DiskSource.list_dir`.
    fn list_dir(&self, directory: &Path) -> Option<Vec<String>> {
        if !directory.is_dir() {
            return None;
        }
        let entries = fs::read_dir(directory).ok()?;
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                out.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        Some(out)
    }

    /// Port of `sources.py::DiskSource.list_subdirs`.
    fn list_subdirs(&self, directory: &Path) -> Option<Vec<String>> {
        if !directory.is_dir() {
            return None;
        }
        let entries = fs::read_dir(directory).ok()?;
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        Some(out)
    }
}

/// Port of `sources.py::OverlaySource`.
pub struct OverlaySource {
    pub base: Box<dyn SourceProvider>,
    pub overlays: OrderedMap<PathBuf, Vec<u8>>,
}

impl OverlaySource {
    /// Port of `sources.py::OverlaySource.__init__`.
    pub fn new(
        base: Box<dyn SourceProvider>,
        overlays: Option<OrderedMap<PathBuf, Vec<u8>>>,
    ) -> Self {
        let mut map = OrderedMap::new();
        if let Some(items) = overlays {
            for (p, b) in items.iter() {
                let abs = std::path::absolute(p).unwrap_or_else(|_| p.clone());
                map.insert(abs, b.clone());
            }
        }
        Self {
            base,
            overlays: map,
        }
    }
}

impl SourceProvider for OverlaySource {
    /// Port of `sources.py::OverlaySource.read`.
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        if let Some(data) = self.overlays.get(&abs) {
            return Some(data.clone());
        }
        self.base.read(path)
    }

    /// Port of `sources.py::OverlaySource.list_dir`.
    fn list_dir(&self, directory: &Path) -> Option<Vec<String>> {
        let abs_dir = std::path::absolute(directory).unwrap_or_else(|_| directory.to_path_buf());
        let names = self.base.list_dir(&abs_dir);
        let mut extra: Vec<String> = self
            .overlays
            .keys()
            .filter(|p| p.parent() == Some(abs_dir.as_path()))
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        extra.sort();
        match names {
            None => {
                if extra.is_empty() {
                    None
                } else {
                    Some(extra)
                }
            }
            Some(mut existing) => {
                let have: HashSet<String> = existing.iter().cloned().collect();
                for n in extra {
                    if !have.contains(&n) {
                        existing.push(n);
                    }
                }
                Some(existing)
            }
        }
    }

    /// Port of `sources.py::OverlaySource.list_subdirs`.
    fn list_subdirs(&self, directory: &Path) -> Option<Vec<String>> {
        self.base.list_subdirs(directory)
    }
}

/// Port of `sources.py::LimitedSource`.
pub struct LimitedSource {
    pub inner: OverlaySource,
    pub limit: usize,
    pub oversized: RefCell<OrderedMap<PathBuf, usize>>,
}

impl LimitedSource {
    /// Port of `sources.py::LimitedSource.__init__`.
    pub fn new(inner: OverlaySource, limit: usize) -> Self {
        Self {
            inner,
            limit,
            oversized: RefCell::new(OrderedMap::new()),
        }
    }
}

impl SourceProvider for LimitedSource {
    /// Port of `sources.py::LimitedSource.read`.
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let mut data = self.inner.overlays.get(&abs).cloned();
        if data.is_none() {
            if let Ok(info) = fs::metadata(path)
                && info.is_file()
                && info.len() > self.limit as u64
            {
                let size = usize::try_from(info.len()).unwrap_or(usize::MAX);
                self.oversized.borrow_mut().insert(path.to_path_buf(), size);
                return None;
            }
            data = self.inner.read(path);
        }
        if let Some(ref d) = data
            && d.len() > self.limit
        {
            self.oversized
                .borrow_mut()
                .insert(path.to_path_buf(), d.len());
            return None;
        }
        data
    }

    /// Port of `sources.py::LimitedSource.list_dir`.
    fn list_dir(&self, directory: &Path) -> Option<Vec<String>> {
        self.inner.list_dir(directory)
    }

    /// Port of `sources.py::LimitedSource.list_subdirs`.
    fn list_subdirs(&self, directory: &Path) -> Option<Vec<String>> {
        self.inner.list_subdirs(directory)
    }
}

/// Port of `sources.py::RecordingSource`.
pub struct RecordingSource {
    pub inner: Box<dyn SourceProvider>,
    pub reads: RefCell<OrderedMap<PathBuf, Option<String>>>,
    pub listings: RefCell<OrderedMap<PathBuf, Option<Vec<String>>>>,
}

impl RecordingSource {
    /// Port of `sources.py::RecordingSource.__init__`.
    pub fn new(inner: Box<dyn SourceProvider>) -> Self {
        Self {
            inner,
            reads: RefCell::new(OrderedMap::new()),
            listings: RefCell::new(OrderedMap::new()),
        }
    }
}

impl SourceProvider for RecordingSource {
    /// Port of `sources.py::RecordingSource.read`.
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        let data = self.inner.read(path);
        let hash = data.as_deref().map(sha256_hex);
        self.reads.borrow_mut().insert(path.to_path_buf(), hash);
        data
    }

    /// Port of `sources.py::RecordingSource.list_dir`.
    fn list_dir(&self, directory: &Path) -> Option<Vec<String>> {
        let names = self.inner.list_dir(directory);
        let sorted = names.as_ref().map(|v| {
            let mut s = v.clone();
            s.sort();
            s
        });
        self.listings
            .borrow_mut()
            .insert(directory.to_path_buf(), sorted);
        names
    }

    /// Port of `sources.py::RecordingSource.list_subdirs`.
    fn list_subdirs(&self, directory: &Path) -> Option<Vec<String>> {
        self.inner.list_subdirs(directory)
    }
}

fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, b) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, delta) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(delta);
        }
    }
    let mut out = String::with_capacity(64);
    for word in h {
        let _ = write!(out, "{word:08x}");
    }
    out
}
