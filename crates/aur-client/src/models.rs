use serde::Deserialize;

/// A package record as returned by the RPC. Info-only fields are empty for
/// search results.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Package {
    /// Package name.
    pub name: String,
    /// Package base name.
    pub package_base: String,
    /// Version string (`epoch:pkgver-pkgrel`).
    pub version: String,
    /// Short description.
    #[serde(default)]
    pub description: Option<String>,
    /// Upstream URL.
    #[serde(default, rename = "URL")]
    pub url: Option<String>,
    /// Number of votes.
    #[serde(default)]
    pub num_votes: u32,
    /// Popularity score.
    #[serde(default)]
    pub popularity: f64,
    /// Epoch seconds when flagged out of date, if flagged.
    #[serde(default)]
    pub out_of_date: Option<i64>,
    /// Maintainer; `None` means the package is an orphan.
    #[serde(default)]
    pub maintainer: Option<String>,
    /// Original submitter.
    #[serde(default)]
    pub submitter: Option<String>,
    /// Epoch seconds of first submission.
    #[serde(default)]
    pub first_submitted: i64,
    /// Epoch seconds of last modification.
    #[serde(default)]
    pub last_modified: i64,
    /// Snapshot tarball path, relative to the AUR host.
    #[serde(default, rename = "URLPath")]
    pub url_path: Option<String>,
    /// Runtime dependencies (info only).
    #[serde(default)]
    pub depends: Vec<String>,
    /// Build dependencies (info only).
    #[serde(default)]
    pub make_depends: Vec<String>,
    /// Optional dependencies (info only).
    #[serde(default)]
    pub opt_depends: Vec<String>,
    /// Check dependencies (info only).
    #[serde(default)]
    pub check_depends: Vec<String>,
    /// Conflicts (info only).
    #[serde(default)]
    pub conflicts: Vec<String>,
    /// Provides (info only).
    #[serde(default)]
    pub provides: Vec<String>,
    /// Replaces (info only).
    #[serde(default)]
    pub replaces: Vec<String>,
    /// Groups (info only).
    #[serde(default)]
    pub groups: Vec<String>,
    /// Licenses (info only).
    #[serde(default)]
    pub license: Vec<String>,
    /// Keywords (info only).
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Co-maintainers (info only).
    #[serde(default)]
    pub co_maintainers: Vec<String>,
}

impl Package {
    /// True when the package has no maintainer.
    pub fn is_orphan(&self) -> bool {
        self.maintainer.is_none()
    }

    /// True when the package is flagged out of date.
    pub fn is_out_of_date(&self) -> bool {
        self.out_of_date.is_some()
    }
}

/// The RPC response envelope.
#[derive(Debug, Deserialize)]
pub(crate) struct Envelope {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub results: Vec<Package>,
    #[serde(default)]
    pub error: Option<String>,
}
