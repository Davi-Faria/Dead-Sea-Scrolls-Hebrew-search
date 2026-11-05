//! The page and the data, built into the program.
//!
//! Everything the page needs is compiled into the executable, so one file is all there is to
//! download. For people who rebuild the data (or change the page) the built-in copies can be
//! replaced, one file at a time, by files in a folder: `--data-dir` for `dss.json`, `bible.json`
//! and `scroll-names.json`, `--web-dir` for `index.html`, `style.css`, `app.js` and `search.js`.

use dss_search::corpus::Corpus;
use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Web,
    Data,
}

pub struct Asset {
    /// the address the page asks for
    pub route: &'static str,
    /// the file name inside the web or data folder
    pub file: &'static str,
    pub mime: &'static str,
    pub kind: Kind,
    pub bytes: &'static [u8],
}

pub static ASSETS: [Asset; 7] = [
    Asset {
        route: "/index.html",
        file: "index.html",
        mime: "text/html; charset=utf-8",
        kind: Kind::Web,
        bytes: include_bytes!("../web/index.html"),
    },
    Asset {
        route: "/style.css",
        file: "style.css",
        mime: "text/css; charset=utf-8",
        kind: Kind::Web,
        bytes: include_bytes!("../web/style.css"),
    },
    Asset {
        route: "/app.js",
        file: "app.js",
        mime: "text/javascript; charset=utf-8",
        kind: Kind::Web,
        bytes: include_bytes!("../web/app.js"),
    },
    Asset {
        route: "/search.js",
        file: "search.js",
        mime: "text/javascript; charset=utf-8",
        kind: Kind::Web,
        bytes: include_bytes!("../web/search.js"),
    },
    Asset {
        route: "/data/dss.json",
        file: "dss.json",
        mime: "application/json; charset=utf-8",
        kind: Kind::Data,
        bytes: include_bytes!("../data/dss.json"),
    },
    Asset {
        route: "/data/bible.json",
        file: "bible.json",
        mime: "application/json; charset=utf-8",
        kind: Kind::Data,
        bytes: include_bytes!("../data/bible.json"),
    },
    Asset {
        route: "/data/scroll-names.json",
        file: "scroll-names.json",
        mime: "application/json; charset=utf-8",
        kind: Kind::Data,
        bytes: include_bytes!("../data/scroll-names.json"),
    },
];

/// Where the files come from: the built-in copies unless a folder has a file of the same name.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    pub web_dir: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
}

impl Sources {
    pub fn find(route: &str) -> Option<&'static Asset> {
        ASSETS.iter().find(|a| a.route == route)
    }

    /// The file on disk that replaces this asset, if there is one.
    pub fn override_path(&self, asset: &Asset) -> Option<PathBuf> {
        let dir = match asset.kind {
            Kind::Web => self.web_dir.as_ref(),
            Kind::Data => self.data_dir.as_ref(),
        }?;
        let path = dir.join(asset.file);
        path.is_file().then_some(path)
    }

    pub fn read(&self, asset: &'static Asset) -> Result<Cow<'static, [u8]>, String> {
        match self.override_path(asset) {
            Some(path) => std::fs::read(&path).map(Cow::Owned).map_err(|e| format!("Could not read {}: {e}", path.display())),
            None => Ok(Cow::Borrowed(asset.bytes)),
        }
    }

    fn named(&self, file: &str) -> &'static Asset {
        ASSETS.iter().find(|a| a.file == file).expect("a built-in asset")
    }

    /// Load the corpus for the statistics.
    pub fn load_corpus(&self) -> Result<Corpus, String> {
        let dss = self.read(self.named("dss.json"))?;
        let names = self.read(self.named("scroll-names.json"))?;
        Corpus::from_json(&dss, Some(&names))
    }

    /// Say which files are being used from disk instead of the built-in ones.
    pub fn describe_overrides(&self) -> Vec<String> {
        ASSETS.iter().filter_map(|a| self.override_path(a).map(|p| p.display().to_string())).collect()
    }
}

/// A short fingerprint of some bytes (FNV-1a), for the browser to know when a file has not changed.
pub fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
