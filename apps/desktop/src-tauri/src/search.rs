//! Search DTOs and the pure merge of repo, local and AUR results.
//!
//! Serialization matches `apps/desktop/src/api/types.ts` exactly.

use serde::Serialize;
use yaygui_alpm_reader::RepoPackage;
use yaygui_aur_client::Package as AurPackage;

/// Maximum results returned per source, to keep the UI fast.
pub const MAX_RESULTS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PackageSource {
    Repo { repo: String },
    Aur,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: PackageSource,
    pub installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub votes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub popularity: Option<f64>,
    /// Outer `None` omits the field (repo packages); `Some(None)` is JSON null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out_of_date: Option<Option<i64>>,
    /// Outer `None` omits the field (repo packages); `Some(None)` is JSON null (orphan).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintainer: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub repo: Vec<PackageSummary>,
    pub aur: Vec<PackageSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aur_error: Option<String>,
}

/// Merge repo results, AUR results (or an error message) and local install
/// state into the response. `installed_version` returns the locally installed
/// version of a package name, if any.
pub fn merge_results(
    repo: Vec<RepoPackage>,
    aur: Result<Vec<AurPackage>, String>,
    installed_version: impl Fn(&str) -> Option<String>,
) -> SearchResponse {
    let repo = repo
        .into_iter()
        .take(MAX_RESULTS)
        .map(|p| {
            let installed_version = installed_version(&p.name);
            PackageSummary {
                installed: installed_version.is_some(),
                installed_version,
                name: p.name,
                version: p.version,
                description: p.description,
                source: PackageSource::Repo { repo: p.repo },
                votes: None,
                popularity: None,
                out_of_date: None,
                maintainer: None,
                last_modified: None,
            }
        })
        .collect();

    let (aur, aur_error) = match aur {
        Ok(pkgs) => {
            let list = pkgs
                .into_iter()
                .take(MAX_RESULTS)
                .map(|p| {
                    let installed_version = installed_version(&p.name);
                    PackageSummary {
                        installed: installed_version.is_some(),
                        installed_version,
                        name: p.name,
                        version: p.version,
                        description: p.description.unwrap_or_default(),
                        source: PackageSource::Aur,
                        votes: Some(p.num_votes),
                        popularity: Some(p.popularity),
                        out_of_date: Some(p.out_of_date),
                        maintainer: Some(p.maintainer),
                        last_modified: Some(p.last_modified),
                    }
                })
                .collect();
            (list, None)
        }
        Err(e) => (Vec::new(), Some(e)),
    };

    SearchResponse {
        repo,
        aur,
        aur_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn repo_pkg(name: &str) -> RepoPackage {
        RepoPackage {
            name: name.into(),
            version: "1.0-1".into(),
            description: "d".into(),
            repo: "extra".into(),
            ..Default::default()
        }
    }

    fn aur_pkg(name: &str, maintainer: Option<&str>, ood: Option<i64>) -> AurPackage {
        let v: Value = json!({
            "ID": 1, "Name": name, "PackageBaseID": 1, "PackageBase": name,
            "Version": "2.0-1", "Description": null, "NumVotes": 7,
            "Popularity": 1.5, "OutOfDate": ood, "Maintainer": maintainer,
            "FirstSubmitted": 1, "LastModified": 1700000000
        });
        serde_json::from_value(v).expect("valid AUR package json")
    }

    fn to_json(r: &SearchResponse) -> Value {
        serde_json::to_value(r).expect("serializes")
    }

    #[test]
    fn repo_shape_matches_ts_contract() {
        let r = merge_results(vec![repo_pkg("foo")], Ok(vec![]), |n| {
            (n == "foo").then(|| "0.9-1".to_owned())
        });
        assert_eq!(
            to_json(&r),
            json!({
                "repo": [{
                    "name": "foo", "version": "1.0-1", "description": "d",
                    "source": {"kind": "repo", "repo": "extra"},
                    "installed": true, "installedVersion": "0.9-1"
                }],
                "aur": []
            })
        );
    }

    #[test]
    fn aur_shape_has_nulls_for_orphan_and_not_flagged() {
        let r = merge_results(vec![], Ok(vec![aur_pkg("bar", None, None)]), |_| None);
        assert_eq!(
            to_json(&r),
            json!({
                "repo": [],
                "aur": [{
                    "name": "bar", "version": "2.0-1", "description": "",
                    "source": {"kind": "aur"}, "installed": false,
                    "votes": 7, "popularity": 1.5,
                    "outOfDate": null, "maintainer": null,
                    "lastModified": 1700000000
                }]
            })
        );
    }

    #[test]
    fn aur_values_pass_through() {
        let r = merge_results(
            vec![],
            Ok(vec![aur_pkg("bar", Some("alice"), Some(42))]),
            |_| Some("2.0-1".to_owned()),
        );
        let j = to_json(&r);
        assert_eq!(j["aur"][0]["outOfDate"], 42);
        assert_eq!(j["aur"][0]["maintainer"], "alice");
        assert_eq!(j["aur"][0]["installed"], true);
    }

    #[test]
    fn aur_error_keeps_repo_results() {
        let r = merge_results(vec![repo_pkg("foo")], Err("boom".into()), |_| None);
        let j = to_json(&r);
        assert_eq!(j["aurError"], "boom");
        assert_eq!(j["repo"].as_array().map(Vec::len), Some(1));
        assert_eq!(j["aur"], json!([]));
    }

    #[test]
    fn results_are_capped() {
        let repo = (0..MAX_RESULTS + 10)
            .map(|i| repo_pkg(&format!("p{i}")))
            .collect();
        let aur = (0..MAX_RESULTS + 10)
            .map(|i| aur_pkg(&format!("a{i}"), Some("m"), None))
            .collect();
        let r = merge_results(repo, Ok(aur), |_| None);
        assert_eq!(r.repo.len(), MAX_RESULTS);
        assert_eq!(r.aur.len(), MAX_RESULTS);
    }
}
