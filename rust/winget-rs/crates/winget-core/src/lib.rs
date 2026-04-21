use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use reqwest::blocking::{Client, Response};
use rusqlite::{
    Connection, OpenFlags, Row as SqlRow, params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml::{Mapping as YamlMapping, Value as YamlValue};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;
#[cfg(windows)]
use winreg::{RegKey, enums::*};
use zip::ZipArchive;

const DEFAULT_MARKET: &str = "US";
const DEFAULT_MAX_RESULTS: usize = 50;
const LIST_LOOKUP_MAX_RESULTS: usize = 500;
const PREINDEXED_CANDIDATES: &[&str] = &["source2.msix", "source.msix"];
const REST_SUPPORTED_CONTRACTS: &[&str] = &[
    "1.12.0", "1.10.0", "1.9.0", "1.7.0", "1.6.0", "1.5.0", "1.4.0", "1.1.0", "1.0.0",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    PreIndexed,
    Rest,
}

impl Display for SourceKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceKind::PreIndexed => f.write_str("preindexed"),
            SourceKind::Rest => f.write_str("rest"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRecord {
    pub name: String,
    pub kind: SourceKind,
    pub arg: String,
    pub identifier: String,
    #[serde(default)]
    pub last_update: Option<DateTime<Utc>>,
    #[serde(default)]
    pub source_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceStore {
    sources: Vec<SourceRecord>,
}

impl Default for SourceStore {
    fn default() -> Self {
        Self {
            sources: vec![
                SourceRecord {
                    name: "winget".to_string(),
                    kind: SourceKind::PreIndexed,
                    arg: "https://cdn.winget.microsoft.com/cache".to_string(),
                    identifier: "Microsoft.Winget.Source_8wekyb3d8bbwe".to_string(),
                    last_update: None,
                    source_version: None,
                },
                SourceRecord {
                    name: "msstore".to_string(),
                    kind: SourceKind::Rest,
                    arg: "https://storeedgefd.dsx.mp.microsoft.com/v9.0".to_string(),
                    identifier: "StoreEdgeFD".to_string(),
                    last_update: None,
                    source_version: None,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackageQuery {
    pub query: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub moniker: Option<String>,
    pub tag: Option<String>,
    pub command: Option<String>,
    pub source: Option<String>,
    pub count: Option<usize>,
    pub exact: bool,
    pub version: Option<String>,
    pub channel: Option<String>,
    pub locale: Option<String>,
    pub installer_type: Option<String>,
    pub installer_architecture: Option<String>,
    pub install_scope: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ListQuery {
    pub query: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub moniker: Option<String>,
    pub tag: Option<String>,
    pub command: Option<String>,
    pub source: Option<String>,
    pub count: Option<usize>,
    pub exact: bool,
    pub install_scope: Option<String>,
    pub upgrade_only: bool,
    pub include_unknown: bool,
    pub include_pinned: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchMatch {
    pub source_name: String,
    pub source_kind: SourceKind,
    pub id: String,
    pub name: String,
    pub moniker: Option<String>,
    pub version: Option<String>,
    pub channel: Option<String>,
    pub match_criteria: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResponse {
    pub matches: Vec<SearchMatch>,
    pub warnings: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ListMatch {
    pub name: String,
    pub id: String,
    pub local_id: String,
    pub installed_version: String,
    pub available_version: Option<String>,
    pub source_name: Option<String>,
    pub publisher: Option<String>,
    pub scope: Option<String>,
    pub installer_category: Option<String>,
    pub install_location: Option<String>,
    pub package_family_names: Vec<String>,
    pub product_codes: Vec<String>,
    pub upgrade_codes: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ListResponse {
    pub matches: Vec<ListMatch>,
    pub warnings: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VersionKey {
    pub version: String,
    pub channel: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub channel: String,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub moniker: Option<String>,
    pub package_url: Option<String>,
    pub publisher_url: Option<String>,
    pub publisher_support_url: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub privacy_url: Option<String>,
    pub author: Option<String>,
    pub copyright: Option<String>,
    pub copyright_url: Option<String>,
    pub release_notes: Option<String>,
    pub release_notes_url: Option<String>,
    pub tags: Vec<String>,
    pub package_dependencies: Vec<String>,
    pub documentation: Vec<Documentation>,
    pub installers: Vec<Installer>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Documentation {
    pub label: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Installer {
    pub architecture: Option<String>,
    pub installer_type: Option<String>,
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub product_code: Option<String>,
    pub locale: Option<String>,
    pub scope: Option<String>,
    pub release_date: Option<String>,
    pub package_family_name: Option<String>,
    pub upgrade_code: Option<String>,
    pub commands: Vec<String>,
    pub package_dependencies: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ShowResult {
    pub package: SearchMatch,
    pub manifest: Manifest,
    pub selected_installer: Option<Installer>,
    pub cached_files: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VersionsResult {
    pub package: SearchMatch,
    pub versions: Vec<VersionKey>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheWarmResult {
    pub package: SearchMatch,
    pub cached_files: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceUpdateResult {
    pub name: String,
    pub kind: SourceKind,
    pub detail: String,
}

#[derive(Debug, Clone)]
struct InstalledPackage {
    name: String,
    local_id: String,
    installed_version: String,
    publisher: Option<String>,
    scope: Option<String>,
    installer_category: Option<String>,
    install_location: Option<String>,
    package_family_names: Vec<String>,
    product_codes: Vec<String>,
    upgrade_codes: Vec<String>,
    correlated: Option<SearchMatch>,
}

#[derive(Debug, Clone)]
struct LocatedMatch {
    display: SearchMatch,
    source_index: usize,
    locator: MatchLocator,
}

#[derive(Debug)]
struct SearchSourceMatches {
    matches: Vec<LocatedMatch>,
    truncated: bool,
}

#[derive(Debug, Clone)]
enum MatchLocator {
    PreIndexedV1 {
        package_rowid: i64,
    },
    PreIndexedV2 {
        package_rowid: i64,
        package_hash: String,
    },
    Rest {
        package_id: String,
        versions: Vec<VersionKey>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchSemantics {
    Many,
    Single,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RestInfoCache {
    expires_at: DateTime<Utc>,
    value: RestInformation,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RestInformation {
    #[serde(rename = "SourceIdentifier")]
    source_identifier: String,
    #[serde(rename = "ServerSupportedVersions", default)]
    server_supported_versions: Vec<String>,
    #[serde(rename = "RequiredPackageMatchFields", default)]
    required_package_match_fields: Vec<String>,
    #[serde(rename = "UnsupportedPackageMatchFields", default)]
    unsupported_package_match_fields: Vec<String>,
    #[serde(rename = "RequiredQueryParameters", default)]
    required_query_parameters: Vec<String>,
    #[serde(rename = "UnsupportedQueryParameters", default)]
    unsupported_query_parameters: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PackageVersionDataDocument {
    #[serde(rename = "vD", default)]
    versions: Vec<PackageVersionDataEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct PackageVersionDataEntry {
    #[serde(rename = "v")]
    version: String,
    #[serde(rename = "rP")]
    manifest_relative_path: String,
    #[serde(rename = "s256H")]
    manifest_hash: String,
}

pub struct Repository {
    client: Client,
    store: SourceStore,
}

impl Repository {
    pub fn open() -> Result<Self> {
        ensure_app_dirs()?;
        let store = load_store()?;
        let client = Client::builder()
            .user_agent("winget-rs/0.1")
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self { client, store })
    }

    pub fn list_sources(&self) -> Vec<SourceRecord> {
        self.store.sources.clone()
    }

    pub fn update_sources(&mut self, source_name: Option<&str>) -> Result<Vec<SourceUpdateResult>> {
        let indexes = self.resolve_source_indexes(source_name)?;
        let mut results = Vec::new();

        for index in indexes {
            let detail = match self.store.sources[index].kind {
                SourceKind::PreIndexed => self.update_preindexed(index)?,
                SourceKind::Rest => self.update_rest(index)?,
            };

            results.push(SourceUpdateResult {
                name: self.store.sources[index].name.clone(),
                kind: self.store.sources[index].kind,
                detail,
            });
        }

        save_store(&self.store)?;
        Ok(results)
    }

    pub fn search(&mut self, query: &PackageQuery) -> Result<SearchResponse> {
        let (matches, warnings, truncated) = self.search_located(query, SearchSemantics::Many)?;
        Ok(SearchResponse {
            matches: matches.into_iter().map(|item| item.display).collect(),
            warnings,
            truncated,
        })
    }

    pub fn list(&mut self, query: &ListQuery) -> Result<ListResponse> {
        if (query.include_unknown || query.include_pinned) && !query.upgrade_only {
            bail!("--include-unknown and --include-pinned require --upgrade-available");
        }
        if query.source.is_some()
            && query.query.is_none()
            && query.id.is_none()
            && query.name.is_none()
            && query.moniker.is_none()
            && query.tag.is_none()
            && query.command.is_none()
        {
            bail!("list --source currently requires a query or explicit filter");
        }

        let has_filter = list_query_needs_available_lookup(query);
        let needs_available = has_filter || query.upgrade_only;

        let mut warnings = Vec::new();
        let mut installed = collect_installed_packages(query.install_scope.as_deref())?;

        if needs_available && has_filter {
            // Filtered lookup: search sources with the user's query
            let available_query = package_query_from_list_query(query);
            let (matches, source_warnings, _) =
                self.search_located(&available_query, SearchSemantics::Many)?;
            warnings.extend(source_warnings);
            let candidates: Vec<SearchMatch> = matches.into_iter().map(|c| c.display).collect();
            for package in &mut installed {
                package.correlated = correlate_installed_package(
                    package,
                    &candidates,
                    allow_loose_list_correlation(query),
                );
            }
        } else if needs_available {
            // Unfiltered upgrade: look up each installed package by its correlation names
            warnings.extend(self.correlate_all_installed(&mut installed)?);
        }

        let mut matches = installed
            .into_iter()
            .filter(|package| {
                list_package_matches(package, query)
                    && (!query.upgrade_only
                        || installed_package_matches_upgrade_filter(package, query))
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            list_sort_weight(left)
                .cmp(&list_sort_weight(right))
                .then_with(|| {
                    left.name
                        .to_ascii_lowercase()
                        .cmp(&right.name.to_ascii_lowercase())
                })
                .then_with(|| left.local_id.cmp(&right.local_id))
        });

        let truncated = if let Some(limit) = query.count {
            let was_truncated = matches.len() > limit;
            matches.truncate(limit);
            was_truncated
        } else {
            false
        };

        Ok(ListResponse {
            matches: matches.into_iter().map(list_match_from_installed).collect(),
            warnings,
            truncated,
        })
    }

    /// For unfiltered upgrade/list, search the entire available index and correlate
    /// against all installed packages.
    fn correlate_all_installed(
        &mut self,
        installed: &mut [InstalledPackage],
    ) -> Result<Vec<String>> {
        let all_query = PackageQuery {
            query: None,
            id: None,
            name: None,
            moniker: None,
            tag: None,
            command: None,
            source: None,
            count: Some(100_000), // fetch the entire index
            exact: false,
            version: None,
            channel: None,
            locale: None,
            installer_type: None,
            installer_architecture: None,
            install_scope: None,
        };
        let (matches, warnings, _) = self.search_located(&all_query, SearchSemantics::Many)?;
        let candidates: Vec<SearchMatch> = matches.into_iter().map(|c| c.display).collect();

        for package in installed.iter_mut() {
            package.correlated = correlate_installed_package(package, &candidates, true);
        }

        Ok(warnings)
    }

    pub fn search_versions(&mut self, query: &PackageQuery) -> Result<VersionsResult> {
        let (located, warnings) =
            self.find_single_match_with_semantics(query, SearchSemantics::Many)?;
        let versions = self.versions_for_match(&located, query)?;
        Ok(VersionsResult {
            package: located.display,
            versions,
            warnings,
        })
    }

    pub fn show_versions(&mut self, query: &PackageQuery) -> Result<VersionsResult> {
        let (located, warnings) = self.find_single_match(query)?;
        let versions = self.versions_for_match(&located, query)?;
        Ok(VersionsResult {
            package: located.display,
            versions,
            warnings,
        })
    }

    pub fn show(&mut self, query: &PackageQuery) -> Result<ShowResult> {
        let (located, warnings) = self.find_single_match(query)?;
        let (manifest, cached_files) = self.manifest_for_match(&located, query)?;
        let selected_installer = select_installer(&manifest.installers, query);

        Ok(ShowResult {
            package: located.display,
            manifest,
            selected_installer,
            cached_files,
            warnings,
        })
    }

    pub fn warm_cache(&mut self, query: &PackageQuery) -> Result<CacheWarmResult> {
        let (located, warnings) = self.find_single_match(query)?;
        let (_, cached_files) = self.manifest_for_match(&located, query)?;

        Ok(CacheWarmResult {
            package: located.display,
            cached_files,
            warnings,
        })
    }

    fn find_single_match(&mut self, query: &PackageQuery) -> Result<(LocatedMatch, Vec<String>)> {
        self.find_single_match_with_semantics(query, SearchSemantics::Single)
    }

    fn find_single_match_with_semantics(
        &mut self,
        query: &PackageQuery,
        semantics: SearchSemantics,
    ) -> Result<(LocatedMatch, Vec<String>)> {
        let (matches, warnings, _) = self.search_located(query, semantics)?;

        if matches.is_empty() {
            bail!("no package matched the supplied query");
        }

        if matches.len() > 1 {
            let choices = matches
                .iter()
                .take(10)
                .map(|item| {
                    format!(
                        "{} [{}] ({})",
                        item.display.name, item.display.id, item.display.source_name
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            bail!("multiple packages matched: {choices}");
        }

        Ok((matches.into_iter().next().expect("one match"), warnings))
    }

    fn search_located(
        &mut self,
        query: &PackageQuery,
        semantics: SearchSemantics,
    ) -> Result<(Vec<LocatedMatch>, Vec<String>, bool)> {
        let indexes = self.resolve_source_indexes(query.source.as_deref())?;
        let mut matches = Vec::new();
        let mut warnings = Vec::new();
        let mut truncated = false;

        for index in indexes {
            match self.search_source(index, query, semantics) {
                Ok(mut source_matches) => {
                    truncated |= source_matches.truncated;
                    matches.append(&mut source_matches.matches);
                }
                Err(_error) => warnings.push(format!(
                    "Failed when searching source; results will not be included: {}",
                    self.store.sources[index].name
                )),
            }
        }

        if semantics == SearchSemantics::Many {
            matches.sort_by(|left, right| {
                search_match_sort_score(&right.display, query)
                    .cmp(&search_match_sort_score(&left.display, query))
            });
            let limit = max_results(query);
            if matches.len() > limit {
                truncated = true;
                matches.truncate(limit);
            }
        }

        Ok((matches, warnings, truncated))
    }

    fn search_source(
        &mut self,
        source_index: usize,
        query: &PackageQuery,
        semantics: SearchSemantics,
    ) -> Result<SearchSourceMatches> {
        match self.store.sources[source_index].kind {
            SourceKind::PreIndexed => self.search_preindexed(source_index, query, semantics),
            SourceKind::Rest => self.search_rest(source_index, query, semantics),
        }
    }

    fn versions_for_match(
        &mut self,
        located: &LocatedMatch,
        _query: &PackageQuery,
    ) -> Result<Vec<VersionKey>> {
        match &located.locator {
            MatchLocator::PreIndexedV1 { package_rowid } => {
                let source = self.source_clone(located.source_index);
                let connection = self.open_preindexed_connection(located.source_index)?;
                let versions = query_v1_versions(&connection, *package_rowid)?;
                let mut keys = versions
                    .into_iter()
                    .map(|row| VersionKey {
                        version: row.version,
                        channel: row.channel,
                    })
                    .collect::<Vec<_>>();
                sort_versions_desc(&mut keys);
                if keys.is_empty() {
                    bail!(
                        "no versions found for {} in {}",
                        located.display.id,
                        source.name
                    );
                }
                Ok(keys)
            }
            MatchLocator::PreIndexedV2 {
                package_rowid,
                package_hash,
            } => {
                let source = self.source_clone(located.source_index);
                let (entries, _) =
                    self.load_v2_version_data(&source, *package_rowid, package_hash.as_str())?;
                let mut keys = entries
                    .into_iter()
                    .map(|entry| VersionKey {
                        version: entry.version,
                        channel: String::new(),
                    })
                    .collect::<Vec<_>>();
                sort_versions_desc(&mut keys);
                Ok(keys)
            }
            MatchLocator::Rest { versions, .. } => {
                let mut keys = versions.clone();
                sort_versions_desc(&mut keys);
                Ok(keys)
            }
        }
    }

    fn manifest_for_match(
        &mut self,
        located: &LocatedMatch,
        query: &PackageQuery,
    ) -> Result<(Manifest, Vec<PathBuf>)> {
        match &located.locator {
            MatchLocator::PreIndexedV1 { package_rowid } => {
                let source = self.source_clone(located.source_index);
                let connection = self.open_preindexed_connection(located.source_index)?;
                let versions = query_v1_versions(&connection, *package_rowid)?;
                let selected = select_v1_version(
                    &versions,
                    query.version.as_deref(),
                    query.channel.as_deref(),
                )?;
                let relative_path = resolve_v1_relative_path(&connection, selected.pathpart_id)?;
                let bytes = self.get_cached_source_file(
                    "V1_M",
                    &source,
                    &relative_path,
                    selected.manifest_hash.as_deref(),
                )?;
                let mut manifest = parse_yaml_manifest(&bytes.bytes)?;
                manifest.version = selected.version.clone();
                manifest.channel = selected.channel.clone();
                Ok((manifest, vec![bytes.path]))
            }
            MatchLocator::PreIndexedV2 {
                package_rowid,
                package_hash,
            } => {
                let source = self.source_clone(located.source_index);
                let (entries, version_data_file) =
                    self.load_v2_version_data(&source, *package_rowid, package_hash.as_str())?;
                let selected = select_v2_version(&entries, query.version.as_deref())?;
                let manifest_bytes = self.get_cached_source_file(
                    "V2_M",
                    &source,
                    &selected.manifest_relative_path,
                    Some(selected.manifest_hash.as_str()),
                )?;
                let mut manifest = parse_yaml_manifest(&manifest_bytes.bytes)?;
                manifest.version = selected.version.clone();
                Ok((manifest, vec![version_data_file, manifest_bytes.path]))
            }
            MatchLocator::Rest {
                package_id,
                versions,
            } => {
                let source = self.source_clone(located.source_index);
                let selected = select_rest_version(
                    versions,
                    query.version.as_deref(),
                    query.channel.as_deref(),
                )?;
                let (bytes, cache_path) = self.get_or_fetch_rest_manifest(
                    &source,
                    package_id,
                    &selected.version,
                    &selected.channel,
                )?;
                let manifest =
                    parse_rest_manifest(&bytes, package_id, &selected.version, &selected.channel)?;
                Ok((manifest, vec![cache_path]))
            }
        }
    }

    fn search_preindexed(
        &mut self,
        source_index: usize,
        query: &PackageQuery,
        semantics: SearchSemantics,
    ) -> Result<SearchSourceMatches> {
        let connection = self.open_preindexed_connection(source_index)?;
        let source = self.source_clone(source_index);

        match query_v2_matches(&connection, query, semantics) {
            Ok((rows, truncated)) => Ok(SearchSourceMatches {
                truncated,
                matches: rows
                    .into_iter()
                    .map(|row| LocatedMatch {
                        display: SearchMatch {
                            source_name: source.name.clone(),
                            source_kind: source.kind,
                            id: row.id.clone(),
                            name: row.name.clone(),
                            moniker: row.moniker.clone(),
                            version: Some(row.version.clone()),
                            channel: None,
                            match_criteria: row.match_criteria.clone(),
                        },
                        source_index,
                        locator: MatchLocator::PreIndexedV2 {
                            package_rowid: row.package_rowid,
                            package_hash: row.package_hash,
                        },
                    })
                    .collect(),
            }),
            Err(error) if can_fallback_to_v1(&error) => {
                let (rows, truncated) = query_v1_matches(&connection, query, semantics)?;
                let grouped = group_v1_rows(rows);

                Ok(SearchSourceMatches {
                    truncated,
                    matches: grouped
                        .into_iter()
                        .map(|row| LocatedMatch {
                            display: SearchMatch {
                                source_name: source.name.clone(),
                                source_kind: source.kind,
                                id: row.id.clone(),
                                name: row.name.clone(),
                                moniker: row.moniker.clone(),
                                version: Some(row.version.clone()),
                                channel: Some(row.channel.clone()),
                                match_criteria: row.match_criteria.clone(),
                            },
                            source_index,
                            locator: MatchLocator::PreIndexedV1 {
                                package_rowid: row.package_rowid,
                            },
                        })
                        .collect(),
                })
            }
            Err(error) => Err(error),
        }
    }

    fn search_rest(
        &mut self,
        source_index: usize,
        query: &PackageQuery,
        semantics: SearchSemantics,
    ) -> Result<SearchSourceMatches> {
        let source = self.source_clone(source_index);
        let info = self.load_rest_information(source_index)?;
        let contract = choose_contract(&info.server_supported_versions)
            .ok_or_else(|| anyhow!("no compatible REST contract for {}", source.name))?;

        let url = format!("{}/manifestSearch", source.arg.trim_end_matches('/'));
        let body = build_rest_search_body(query, &info, semantics)?;
        let response = self
            .client
            .post(url)
            .header("Version", contract)
            .json(&body)
            .send()
            .context("REST search request failed")?
            .error_for_status()
            .context("REST search request returned an error")?;
        let json = response
            .json::<JsonValue>()
            .context("failed to parse REST search response")?;
        let data = json
            .get("Data")
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        let max_results = source_fetch_results(query, semantics);

        let mut results = Vec::new();
        for item in data {
            let package_id = json_string(&item, "PackageIdentifier")
                .ok_or_else(|| anyhow!("REST search result missing PackageIdentifier"))?;
            let package_name = json_string(&item, "PackageName")
                .ok_or_else(|| anyhow!("REST search result missing PackageName"))?;
            let mut versions = parse_rest_versions(&item)?;
            sort_versions_desc(&mut versions);
            let latest = versions
                .first()
                .cloned()
                .ok_or_else(|| anyhow!("REST search result had no versions"))?;

            results.push(LocatedMatch {
                display: SearchMatch {
                    source_name: source.name.clone(),
                    source_kind: source.kind,
                    id: package_id.clone(),
                    name: package_name,
                    moniker: json_string(&item, "Moniker"),
                    version: Some(latest.version.clone()),
                    channel: if latest.channel.is_empty() {
                        None
                    } else {
                        Some(latest.channel.clone())
                    },
                    match_criteria: rest_match_criteria(&item, query, semantics),
                },
                source_index,
                locator: MatchLocator::Rest {
                    package_id,
                    versions,
                },
            });
        }

        Ok(SearchSourceMatches {
            truncated: results.len() >= max_results,
            matches: results,
        })
    }

    fn open_preindexed_connection(&mut self, source_index: usize) -> Result<Connection> {
        let source = self.source_clone(source_index);
        let index_path = preindexed_index_path(&source);
        if !index_path.exists() {
            let _ = self.update_preindexed(source_index)?;
            save_store(&self.store)?;
        }

        self.open_sqlite_connection(index_path)
            .context("failed to open preindexed index")
    }

    fn open_sqlite_connection(&self, path: PathBuf) -> Result<Connection> {
        let path_text = path.to_string_lossy().into_owned();
        Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .with_context(|| format!("failed to open SQLite database at {path_text}"))
    }

    fn update_preindexed(&mut self, source_index: usize) -> Result<String> {
        let source = &mut self.store.sources[source_index];
        let state_dir = source_state_dir(source);
        fs::create_dir_all(&state_dir).context("failed to create source state directory")?;

        let mut last_error = None;
        for candidate in PREINDEXED_CANDIDATES {
            let url = format!("{}/{}", source.arg.trim_end_matches('/'), candidate);
            match self.client.get(&url).send() {
                Ok(response) if response.status().is_success() => {
                    let header_version = response
                        .headers()
                        .get("x-ms-meta-sourceversion")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let bytes = response
                        .bytes()
                        .context("failed to read preindexed package bytes")?;
                    let payload = bytes.to_vec();
                    let index_bytes = extract_zip_member(&payload, "Public/index.db")
                        .context("preindexed package did not contain Public/index.db")?;

                    fs::write(preindexed_package_path(source), &payload)
                        .context("failed to persist source package")?;
                    fs::write(preindexed_index_path(source), index_bytes)
                        .context("failed to persist source index")?;
                    source.last_update = Some(Utc::now());
                    source.source_version = header_version;
                    return Ok(format!("downloaded {}", candidate));
                }
                Ok(response) => {
                    last_error = Some(anyhow!(
                        "candidate {} returned HTTP {}",
                        candidate,
                        response.status()
                    ));
                }
                Err(error) => {
                    last_error = Some(anyhow!("candidate {} failed: {error}", candidate));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("no preindexed source candidate succeeded")))
    }

    fn update_rest(&mut self, source_index: usize) -> Result<String> {
        let info = self.fetch_rest_information(source_index, true)?;
        let source = &mut self.store.sources[source_index];
        if !info.source_identifier.is_empty() {
            source.identifier = info.source_identifier.clone();
        }
        source.last_update = Some(Utc::now());
        source.source_version =
            choose_contract(&info.server_supported_versions).map(str::to_string);
        Ok("refreshed information cache".to_string())
    }

    fn load_rest_information(&mut self, source_index: usize) -> Result<RestInformation> {
        let source = self.source_clone(source_index);
        let cache_path = rest_information_cache_path(&source);

        if cache_path.exists() {
            let cache = serde_json::from_slice::<RestInfoCache>(
                &fs::read(&cache_path).context("failed to read REST information cache")?,
            )
            .context("failed to parse REST information cache")?;
            if cache.expires_at > Utc::now() {
                return Ok(cache.value);
            }
        }

        self.fetch_rest_information(source_index, true)
    }

    fn fetch_rest_information(
        &mut self,
        source_index: usize,
        persist: bool,
    ) -> Result<RestInformation> {
        let source = self.source_clone(source_index);
        let url = format!("{}/information", source.arg.trim_end_matches('/'));
        let response = self
            .client
            .get(url)
            .send()
            .context("REST information request failed")?
            .error_for_status()
            .context("REST information request returned an error")?;

        let max_age = cache_control_max_age(&response);
        let json = response
            .json::<JsonValue>()
            .context("failed to parse REST information response")?;
        let data = json
            .get("Data")
            .cloned()
            .ok_or_else(|| anyhow!("REST information response missing Data"))?;
        let info = serde_json::from_value::<RestInformation>(data)
            .context("failed to deserialize REST information payload")?;

        if persist {
            let cache = RestInfoCache {
                expires_at: Utc::now() + Duration::seconds(max_age as i64),
                value: info.clone(),
            };
            write_json(rest_information_cache_path(&source), &cache)?;
        }

        Ok(info)
    }

    fn load_v2_version_data(
        &self,
        source: &SourceRecord,
        package_rowid: i64,
        package_hash: &str,
    ) -> Result<(Vec<PackageVersionDataEntry>, PathBuf)> {
        let connection = self
            .open_sqlite_connection(preindexed_index_path(source))
            .context("failed to reopen preindexed index for V2 version data")?;
        let package_hash = package_hash.to_ascii_lowercase();
        let package_id = query_optional_value(
            &connection,
            "SELECT id FROM packages WHERE rowid = ?1",
            vec![SqlValue::Integer(package_rowid)],
            |row| row_string(row, 0),
        )?
        .ok_or_else(|| anyhow!("failed to resolve package id for V2 version data"))?;
        let relative_path = package_version_data_relative_path(&package_id, &package_hash);
        let file =
            self.get_cached_source_file("V2_PVD", source, &relative_path, Some(&package_hash))?;
        let yaml = decompress_mszyml(&file.bytes)?;
        let document = serde_yaml::from_str::<PackageVersionDataDocument>(&yaml)
            .context("failed to parse versionData.mszyml")?;
        Ok((document.versions, file.path))
    }

    fn get_cached_source_file(
        &self,
        bucket: &str,
        source: &SourceRecord,
        relative_path: &str,
        expected_hash: Option<&str>,
    ) -> Result<CachedBytes> {
        let normalized_relative = relative_path.replace('\\', "/");
        let cache_path = temp_cache_path(bucket, &source.identifier)
            .join(normalized_relative.replace('/', "\\"));

        if cache_path.exists() {
            let cached = fs::read(&cache_path).context("failed to read cached source file")?;
            if hash_matches(expected_hash, &cached) {
                return Ok(CachedBytes {
                    path: cache_path,
                    bytes: cached,
                });
            }
        }

        let url = format!(
            "{}/{}",
            source.arg.trim_end_matches('/'),
            normalized_relative.trim_start_matches('/')
        );
        let response = self
            .client
            .get(url)
            .send()
            .context("failed to fetch source file")?
            .error_for_status()
            .context("source file request returned an error")?;
        let bytes = response
            .bytes()
            .context("failed to read source file body")?
            .to_vec();

        if let Some(hash) = expected_hash {
            verify_hash(hash, &bytes)?;
        }

        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).context("failed to create cache directory")?;
        }
        fs::write(&cache_path, &bytes).context("failed to write cache file")?;

        Ok(CachedBytes {
            path: cache_path,
            bytes,
        })
    }

    fn get_or_fetch_rest_manifest(
        &mut self,
        source: &SourceRecord,
        package_id: &str,
        version: &str,
        channel: &str,
    ) -> Result<(Vec<u8>, PathBuf)> {
        let cache_path = rest_manifest_cache_path(source, package_id, version, channel);
        if cache_path.exists() {
            return Ok((
                fs::read(&cache_path).context("failed to read cached REST manifest")?,
                cache_path,
            ));
        }

        let info = self.load_rest_information_by_name(&source.name)?;
        let contract = choose_contract(&info.server_supported_versions)
            .ok_or_else(|| anyhow!("no compatible REST contract for {}", source.name))?;
        let url = format!(
            "{}/packageManifests/{}",
            source.arg.trim_end_matches('/'),
            package_id
        );

        let mut params = vec![("Version", version.to_string())];
        if !channel.is_empty() {
            params.push(("Channel", channel.to_string()));
        }
        if info
            .required_query_parameters
            .iter()
            .any(|value| value.eq_ignore_ascii_case("Market"))
        {
            params.push(("Market", default_market()));
        }

        let response = self
            .client
            .get(url)
            .header("Version", contract)
            .query(&params)
            .send()
            .context("REST manifest request failed")?
            .error_for_status()
            .context("REST manifest request returned an error")?;
        let bytes = response
            .bytes()
            .context("failed to read REST manifest response")?
            .to_vec();

        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).context("failed to create REST manifest cache directory")?;
        }
        fs::write(&cache_path, &bytes).context("failed to write REST manifest cache")?;

        Ok((bytes, cache_path))
    }

    fn load_rest_information_by_name(&mut self, name: &str) -> Result<RestInformation> {
        let index = self
            .store
            .sources
            .iter()
            .position(|source| source.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow!("source '{name}' was not configured"))?;
        self.load_rest_information(index)
    }

    fn resolve_source_indexes(&self, source_name: Option<&str>) -> Result<Vec<usize>> {
        if let Some(name) = source_name {
            let index = self
                .store
                .sources
                .iter()
                .position(|source| source.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| anyhow!("source '{name}' was not configured"))?;
            return Ok(vec![index]);
        }

        Ok((0..self.store.sources.len()).collect())
    }

    fn source_clone(&self, index: usize) -> SourceRecord {
        self.store.sources[index].clone()
    }
}

#[derive(Debug)]
struct CachedBytes {
    path: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct V1SearchRow {
    manifest_rowid: i64,
    package_rowid: i64,
    version: String,
    channel: String,
    id: String,
    name: String,
    moniker: Option<String>,
    match_criteria: Option<String>,
}

#[derive(Debug, Clone)]
struct V1VersionRow {
    version: String,
    channel: String,
    pathpart_id: i64,
    manifest_hash: Option<String>,
}

#[derive(Debug, Clone)]
struct V2SearchRow {
    package_rowid: i64,
    package_hash: String,
    id: String,
    name: String,
    moniker: Option<String>,
    version: String,
    match_criteria: Option<String>,
}

fn list_query_needs_available_lookup(query: &ListQuery) -> bool {
    query.query.is_some()
        || query.id.is_some()
        || query.name.is_some()
        || query.moniker.is_some()
        || query.tag.is_some()
        || query.command.is_some()
        || query.source.is_some()
}

fn package_query_from_list_query(query: &ListQuery) -> PackageQuery {
    PackageQuery {
        query: query.query.clone(),
        id: query.id.clone(),
        name: query.name.clone(),
        moniker: query.moniker.clone(),
        tag: query.tag.clone(),
        command: query.command.clone(),
        source: query.source.clone(),
        count: Some(LIST_LOOKUP_MAX_RESULTS),
        exact: query.exact,
        version: None,
        channel: None,
        locale: None,
        installer_type: None,
        installer_architecture: None,
        install_scope: query.install_scope.clone(),
    }
}

fn allow_loose_list_correlation(query: &ListQuery) -> bool {
    query.query.is_some() || query.id.is_some() || query.name.is_some()
}

fn installed_package_has_upgrade(package: &InstalledPackage) -> bool {
    package
        .correlated
        .as_ref()
        .and_then(|candidate| candidate.version.as_deref())
        .is_some_and(|version| {
            compare_version(version, &package.installed_version) == Ordering::Greater
        })
}

fn installed_package_has_unknown_version(package: &InstalledPackage) -> bool {
    package.installed_version.eq_ignore_ascii_case("Unknown")
}

fn installed_package_matches_upgrade_filter(package: &InstalledPackage, query: &ListQuery) -> bool {
    installed_package_has_upgrade(package)
        || (query.include_unknown
            && installed_package_has_unknown_version(package)
            && package.correlated.is_some())
}

fn list_match_from_installed(package: InstalledPackage) -> ListMatch {
    let available_version = package.correlated.as_ref().and_then(|candidate| {
        candidate.version.as_ref().and_then(|candidate_version| {
            if installed_package_has_unknown_version(&package)
                || compare_version(candidate_version, &package.installed_version)
                    == Ordering::Greater
            {
                Some(candidate_version.clone())
            } else {
                None
            }
        })
    });
    let source_name = package
        .correlated
        .as_ref()
        .map(|candidate| candidate.source_name.clone())
        .filter(|value| !value.is_empty());
    let id = package
        .correlated
        .as_ref()
        .map(|candidate| candidate.id.clone())
        .unwrap_or_else(|| package.local_id.clone());

    ListMatch {
        name: package.name,
        id,
        local_id: package.local_id,
        installed_version: package.installed_version,
        available_version,
        source_name,
        publisher: package.publisher,
        scope: package.scope,
        installer_category: package.installer_category,
        install_location: package.install_location,
        package_family_names: package.package_family_names,
        product_codes: package.product_codes,
        upgrade_codes: package.upgrade_codes,
    }
}

fn list_sort_weight(package: &InstalledPackage) -> usize {
    if package.local_id.starts_with("ARP\\") {
        0
    } else if package.name.contains(".SparseApp") || package.local_id.contains(".SparseApp_") {
        1
    } else {
        2
    }
}

fn list_package_matches(package: &InstalledPackage, query: &ListQuery) -> bool {
    let correlated = package.correlated.as_ref();

    if let Some(value) = &query.id {
        let local_match = matches_text(&package.local_id, value, query.exact);
        let correlated_match = correlated
            .map(|candidate| matches_text(&candidate.id, value, query.exact))
            .unwrap_or(false);
        if !local_match && !correlated_match {
            return false;
        }
    }

    if let Some(value) = &query.name
        && !matches_text(&package.name, value, query.exact)
    {
        return false;
    }

    if let Some(value) = &query.query {
        let local_match = matches_text(&package.name, value, query.exact)
            || matches_text(&package.local_id, value, query.exact);
        let correlated_match = correlated
            .map(|candidate| {
                matches_text(&candidate.id, value, query.exact)
                    || matches_text(&candidate.name, value, query.exact)
            })
            .unwrap_or(false);
        if !local_match && !correlated_match {
            return false;
        }
    }

    if let Some(source) = &query.source
        && correlated
            .map(|candidate| !candidate.source_name.eq_ignore_ascii_case(source))
            .unwrap_or(true)
    {
        return false;
    }

    if (query.moniker.is_some() || query.tag.is_some() || query.command.is_some())
        && correlated.is_none()
    {
        return false;
    }

    true
}

fn correlate_installed_package(
    package: &InstalledPackage,
    candidates: &[SearchMatch],
    allow_loose_name_match: bool,
) -> Option<SearchMatch> {
    if package.local_id.starts_with("MSIX\\") {
        return None;
    }

    let installed_name = normalize_correlation_name(&package.name);
    let candidate_names = correlation_name_candidates(&package.name);

    candidates
        .iter()
        .filter_map(|candidate| {
            let candidate_name = normalize_correlation_name(&candidate.name);
            let score = if candidate.id.eq_ignore_ascii_case(&package.local_id) {
                1000
            } else if candidate_names.iter().any(|name| {
                let normalized = normalize_correlation_name(name);
                normalized == candidate_name
            }) {
                900
            } else if allow_loose_name_match
                && candidate_name.len() >= 6
                && installed_name.contains(&candidate_name)
            {
                700
            } else {
                0
            };

            (score > 0).then_some((score, candidate.clone()))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, candidate)| candidate)
}

fn correlation_name_candidates(name: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    candidates.push(name.trim().to_string());

    let mut trimmed = name.trim().to_string();
    if let Some(index) = trimmed.find(" (") {
        trimmed.truncate(index);
    }

    let mut words = Vec::new();
    for token in trimmed.split_whitespace() {
        let lower = token
            .trim_matches(|ch: char| ch == '(' || ch == ')')
            .to_ascii_lowercase();
        if !words.is_empty()
            && (lower == "x64"
                || lower == "x86"
                || lower == "arm64"
                || token.chars().any(|ch| ch.is_ascii_digit()))
        {
            break;
        }
        words.push(token);
    }

    if !words.is_empty() {
        candidates.push(words.join(" "));
    }

    candidates.sort();
    candidates.dedup();
    candidates
}

fn normalize_correlation_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

#[cfg(windows)]
fn collect_installed_packages(scope: Option<&str>) -> Result<Vec<InstalledPackage>> {
    let mut packages = Vec::new();
    let mut seen = BTreeSet::new();

    let machine = !matches!(scope, Some(value) if value.eq_ignore_ascii_case("user"));
    let user = !matches!(scope, Some(value) if value.eq_ignore_ascii_case("machine"));

    if machine {
        collect_uninstall_view(
            &mut packages,
            &mut seen,
            RegKey::predef(HKEY_LOCAL_MACHINE),
            "Machine",
            "X64",
            KEY_READ | KEY_WOW64_64KEY,
        )?;
        collect_uninstall_view(
            &mut packages,
            &mut seen,
            RegKey::predef(HKEY_LOCAL_MACHINE),
            "Machine",
            "X86",
            KEY_READ | KEY_WOW64_32KEY,
        )?;
        collect_appmodel_packages(
            &mut packages,
            &mut seen,
            RegKey::predef(HKEY_LOCAL_MACHINE),
            "Machine",
            KEY_READ | KEY_WOW64_64KEY,
        )?;
    }

    if user {
        collect_uninstall_view(
            &mut packages,
            &mut seen,
            RegKey::predef(HKEY_CURRENT_USER),
            "User",
            "X64",
            KEY_READ,
        )?;
        collect_appmodel_packages(
            &mut packages,
            &mut seen,
            RegKey::predef(HKEY_CURRENT_USER),
            "User",
            KEY_READ,
        )?;
    }

    Ok(packages)
}

#[cfg(not(windows))]
fn collect_installed_packages(_scope: Option<&str>) -> Result<Vec<InstalledPackage>> {
    bail!("installed package discovery is only supported on Windows");
}

#[cfg(windows)]
fn collect_uninstall_view(
    packages: &mut Vec<InstalledPackage>,
    seen: &mut BTreeSet<String>,
    root: RegKey,
    scope: &str,
    arch: &str,
    flags: u32,
) -> Result<()> {
    const UNINSTALL_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

    let uninstall = match root.open_subkey_with_flags(UNINSTALL_PATH, flags) {
        Ok(key) => key,
        Err(_) => return Ok(()),
    };

    for key_name in uninstall.enum_keys().flatten() {
        let subkey = match uninstall.open_subkey_with_flags(&key_name, flags) {
            Ok(key) => key,
            Err(_) => continue,
        };
        if read_reg_dword(&subkey, "SystemComponent") == Some(1)
            || read_reg_string(&subkey, "ParentKeyName").is_some()
        {
            continue;
        }

        let Some(name) = read_reg_string(&subkey, "DisplayName").filter(|value| !value.is_empty())
        else {
            continue;
        };

        let local_id = format!(r"ARP\{scope}\{arch}\{key_name}");
        let installed_version =
            read_reg_string(&subkey, "DisplayVersion").unwrap_or_else(|| "Unknown".to_string());
        let publisher = read_reg_string(&subkey, "Publisher");
        let install_location = read_reg_string(&subkey, "InstallLocation");
        let package_family_names = read_reg_string(&subkey, "PackageFamilyName")
            .into_iter()
            .collect::<Vec<_>>();
        let mut product_codes = read_reg_string(&subkey, "ProductCode")
            .into_iter()
            .collect::<Vec<_>>();
        if product_codes.is_empty() && looks_like_product_code(&key_name) {
            product_codes.push(key_name.to_ascii_lowercase());
        }
        let upgrade_codes = read_reg_string(&subkey, "UpgradeCode")
            .into_iter()
            .collect::<Vec<_>>();
        let installer_category = if local_id.starts_with("ARP\\")
            && read_reg_dword(&subkey, "WindowsInstaller") == Some(1)
        {
            Some("msi".to_string())
        } else if key_name.starts_with("MSIX\\") {
            Some("msix".to_string())
        } else {
            Some("exe".to_string())
        };

        let dedupe_key = format!(
            "{}|{}|{}|{}",
            local_id,
            name.to_ascii_lowercase(),
            installed_version.to_ascii_lowercase(),
            publisher.clone().unwrap_or_default().to_ascii_lowercase()
        );
        if !seen.insert(dedupe_key) {
            continue;
        }

        packages.push(InstalledPackage {
            name,
            local_id,
            installed_version,
            publisher,
            scope: Some(scope.to_string()),
            installer_category,
            install_location,
            package_family_names,
            product_codes,
            upgrade_codes,
            correlated: None,
        });
    }

    Ok(())
}

#[cfg(windows)]
fn collect_appmodel_packages(
    packages: &mut Vec<InstalledPackage>,
    seen: &mut BTreeSet<String>,
    root: RegKey,
    scope: &str,
    flags: u32,
) -> Result<()> {
    const APPMODEL_PACKAGES_PATH: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";

    let appmodel = match root.open_subkey_with_flags(APPMODEL_PACKAGES_PATH, flags) {
        Ok(key) => key,
        Err(_) => return Ok(()),
    };

    for key_name in appmodel.enum_keys().flatten() {
        let subkey = match appmodel.open_subkey_with_flags(&key_name, flags) {
            Ok(key) => key,
            Err(_) => continue,
        };

        let Some(name) = read_reg_string(&subkey, "DisplayName").filter(|value| !value.is_empty())
        else {
            continue;
        };
        let install_location = read_reg_string(&subkey, "PackageRootFolder");
        if install_location
            .as_deref()
            .is_some_and(is_windows_system_path)
        {
            continue;
        }

        let Some(metadata) = parse_msix_package_full_name(&key_name) else {
            continue;
        };

        let local_id = format!(r"MSIX\{key_name}");
        let dedupe_key = format!(
            "{}|{}|{}",
            local_id,
            name.to_ascii_lowercase(),
            metadata.version.to_ascii_lowercase()
        );
        if !seen.insert(dedupe_key) {
            continue;
        }

        packages.push(InstalledPackage {
            name,
            local_id,
            installed_version: metadata.version,
            publisher: None,
            scope: Some(scope.to_string()),
            installer_category: Some("msix".to_string()),
            install_location,
            package_family_names: vec![metadata.family_name],
            product_codes: Vec::new(),
            upgrade_codes: Vec::new(),
            correlated: None,
        });
    }

    Ok(())
}

#[cfg(windows)]
struct ParsedMsixPackageFullName {
    version: String,
    family_name: String,
}

#[cfg(windows)]
fn parse_msix_package_full_name(value: &str) -> Option<ParsedMsixPackageFullName> {
    let segments = value.split('_').collect::<Vec<_>>();
    if segments.len() < 5 {
        return None;
    }

    let name = segments[..segments.len() - 4].join("_");
    if name.is_empty() {
        return None;
    }

    let version = segments[segments.len() - 4].trim();
    let resource_id = segments[segments.len() - 2].trim();
    let publisher_id = segments[segments.len() - 1].trim();
    if version.is_empty() || publisher_id.is_empty() {
        return None;
    }

    let family_name = if resource_id.is_empty() {
        format!("{name}_{publisher_id}")
    } else {
        format!("{name}_{resource_id}_{publisher_id}")
    };

    Some(ParsedMsixPackageFullName {
        version: version.to_string(),
        family_name,
    })
}

#[cfg(windows)]
fn is_windows_system_path(path: &str) -> bool {
    path.trim().to_ascii_lowercase().starts_with(r"c:\windows\")
}

#[cfg(windows)]
fn read_reg_string(key: &RegKey, value_name: &str) -> Option<String> {
    key.get_value::<String, _>(value_name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(windows)]
fn read_reg_dword(key: &RegKey, value_name: &str) -> Option<u32> {
    key.get_value::<u32, _>(value_name).ok()
}

fn looks_like_product_code(value: &str) -> bool {
    value.starts_with('{') && value.ends_with('}')
}

fn ensure_app_dirs() -> Result<()> {
    fs::create_dir_all(app_root()?.join("sources"))
        .context("failed to create app source directory")?;
    Ok(())
}

fn app_root() -> Result<PathBuf> {
    dirs::data_local_dir()
        .map(|path| path.join("winget-rs"))
        .ok_or_else(|| anyhow!("unable to determine LocalAppData path"))
}

fn store_path() -> Result<PathBuf> {
    Ok(app_root()?.join("sources.json"))
}

fn source_state_dir(source: &SourceRecord) -> PathBuf {
    app_root().expect("app root").join("sources").join(
        source
            .name
            .replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_"),
    )
}

fn preindexed_package_path(source: &SourceRecord) -> PathBuf {
    source_state_dir(source).join("source.msix")
}

fn preindexed_index_path(source: &SourceRecord) -> PathBuf {
    source_state_dir(source).join("index.db")
}

fn rest_information_cache_path(source: &SourceRecord) -> PathBuf {
    source_state_dir(source).join("rest-information.json")
}

fn rest_manifest_cache_path(
    source: &SourceRecord,
    package_id: &str,
    version: &str,
    channel: &str,
) -> PathBuf {
    let key = format!(
        "{}|{}|{}|{}",
        source.identifier, package_id, version, channel
    );
    let digest = sha256_hex(key.as_bytes());
    std::env::temp_dir()
        .join("cache")
        .join("REST_M")
        .join(source.identifier.replace(':', "_"))
        .join(format!("{digest}.json"))
}

fn temp_cache_path(bucket: &str, identifier: &str) -> PathBuf {
    std::env::temp_dir()
        .join("cache")
        .join(bucket)
        .join(identifier.replace(':', "_"))
}

fn load_store() -> Result<SourceStore> {
    let path = store_path()?;
    if !path.exists() {
        let store = SourceStore::default();
        save_store(&store)?;
        return Ok(store);
    }

    let bytes = fs::read(path).context("failed to read source store")?;
    serde_json::from_slice(&bytes).context("failed to parse source store")
}

fn save_store(store: &SourceStore) -> Result<()> {
    write_json(store_path()?, store)
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create JSON parent directory")?;
    }

    let bytes = serde_json::to_vec_pretty(value).context("failed to serialize JSON")?;
    fs::write(path, bytes).context("failed to write JSON file")
}

fn query_rows<T, F>(
    connection: &Connection,
    sql: &str,
    params: Vec<SqlValue>,
    mut map: F,
) -> Result<Vec<T>>
where
    F: FnMut(&SqlRow<'_>) -> Result<T>,
{
    let mut statement = connection
        .prepare(sql)
        .with_context(|| format!("failed to prepare SQL query: {sql}"))?;
    let mut rows = statement
        .query(params_from_iter(params))
        .with_context(|| format!("failed to execute SQL query: {sql}"))?;
    let mut result = Vec::new();

    while let Some(row) = rows.next().context("failed to read SQL row")? {
        result.push(map(row)?);
    }

    Ok(result)
}

fn query_optional_value<T, F>(
    connection: &Connection,
    sql: &str,
    params: Vec<SqlValue>,
    map: F,
) -> Result<Option<T>>
where
    F: FnOnce(&SqlRow<'_>) -> Result<T>,
{
    let mut statement = connection
        .prepare(sql)
        .with_context(|| format!("failed to prepare SQL query: {sql}"))?;
    let mut rows = statement
        .query(params_from_iter(params))
        .with_context(|| format!("failed to execute SQL query: {sql}"))?;

    match rows.next().context("failed to read SQL row")? {
        Some(row) => Ok(Some(map(row)?)),
        None => Ok(None),
    }
}

fn row_ref<'a>(row: &'a SqlRow<'_>, index: usize) -> Result<ValueRef<'a>> {
    row.get_ref(index)
        .with_context(|| format!("failed to read SQL column {index}"))
}

fn row_string(row: &SqlRow<'_>, index: usize) -> Result<String> {
    match row_ref(row, index)? {
        ValueRef::Text(value) => std::str::from_utf8(value)
            .context("SQL text column was not valid UTF-8")
            .map(str::to_owned),
        ValueRef::Integer(value) => Ok(value.to_string()),
        ValueRef::Real(value) => Ok(value.to_string()),
        ValueRef::Blob(value) => {
            String::from_utf8(value.to_vec()).context("SQL blob column was not valid UTF-8")
        }
        ValueRef::Null => bail!("SQL column {index} was unexpectedly NULL"),
    }
}

fn row_opt_string(row: &SqlRow<'_>, index: usize) -> Result<Option<String>> {
    match row_ref(row, index)? {
        ValueRef::Null => Ok(None),
        ValueRef::Text(value) => Ok(Some(
            std::str::from_utf8(value)
                .context("SQL text column was not valid UTF-8")?
                .to_owned(),
        )),
        ValueRef::Integer(value) => Ok(Some(value.to_string())),
        ValueRef::Real(value) => Ok(Some(value.to_string())),
        ValueRef::Blob(value) => Ok(Some(
            String::from_utf8(value.to_vec()).context("SQL blob column was not valid UTF-8")?,
        )),
    }
}

fn row_i64(row: &SqlRow<'_>, index: usize) -> Result<i64> {
    match row_ref(row, index)? {
        ValueRef::Integer(value) => Ok(value),
        ValueRef::Text(value) => std::str::from_utf8(value)
            .context("SQL text column was not valid UTF-8")?
            .parse::<i64>()
            .with_context(|| format!("failed to parse integer from SQL text column {index}")),
        ValueRef::Real(value) => Ok(value as i64),
        ValueRef::Null => bail!("SQL column {index} was unexpectedly NULL"),
        ValueRef::Blob(_) => bail!("SQL column {index} was unexpectedly a blob"),
    }
}

fn row_hex_string(row: &SqlRow<'_>, index: usize) -> Result<String> {
    match row_ref(row, index)? {
        ValueRef::Blob(value) => Ok(bytes_to_hex(value)),
        ValueRef::Text(value) => Ok(std::str::from_utf8(value)
            .context("SQL text column was not valid UTF-8")?
            .to_ascii_lowercase()),
        ValueRef::Null => bail!("SQL column {index} was unexpectedly NULL"),
        ValueRef::Integer(value) => Ok(format!("{value:x}")),
        ValueRef::Real(value) => Ok(format!("{:x}", value as i64)),
    }
}

fn row_opt_hex_string(row: &SqlRow<'_>, index: usize) -> Result<Option<String>> {
    match row_ref(row, index)? {
        ValueRef::Null => Ok(None),
        ValueRef::Blob(value) => Ok(Some(bytes_to_hex(value))),
        ValueRef::Text(value) => Ok(Some(
            std::str::from_utf8(value)
                .context("SQL text column was not valid UTF-8")?
                .to_ascii_lowercase(),
        )),
        ValueRef::Integer(value) => Ok(Some(format!("{value:x}"))),
        ValueRef::Real(value) => Ok(Some(format!("{:x}", value as i64))),
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}

fn query_v2_matches(
    connection: &Connection,
    query: &PackageQuery,
    semantics: SearchSemantics,
) -> Result<(Vec<V2SearchRow>, bool)> {
    let (where_clause, params) = build_preindexed_where_clause(query, true, semantics);
    let limit = source_fetch_results(query, semantics);
    let sql = format!(
        "SELECT rowid, id, name, moniker, latest_version, hash \
         FROM packages WHERE {where_clause} LIMIT {}",
        limit + 1
    );
    let mut rows = query_rows(
        connection,
        &sql,
        params.into_iter().map(SqlValue::Text).collect(),
        |row| {
            Ok(V2SearchRow {
                package_rowid: row_i64(row, 0)?,
                id: row_string(row, 1)?,
                name: row_string(row, 2)?,
                moniker: row_opt_string(row, 3)?,
                version: row_string(row, 4)?,
                package_hash: row_hex_string(row, 5)?,
                match_criteria: None,
            })
        },
    )?;
    for row in &mut rows {
        row.match_criteria = infer_preindexed_match_criteria_v2(connection, row, query, semantics)?;
    }
    let truncated = rows.len() > limit;
    rows.truncate(limit);
    Ok((rows, truncated))
}

fn query_v1_matches(
    connection: &Connection,
    query: &PackageQuery,
    semantics: SearchSemantics,
) -> Result<(Vec<V1SearchRow>, bool)> {
    let (where_clause, params) = build_preindexed_where_clause(query, false, semantics);
    let limit = source_fetch_results(query, semantics);
    let sql = format!(
        "SELECT manifest.rowid, manifest.id, versions.version, channels.channel, ids.id, names.name, monikers.moniker \
         FROM manifest \
         JOIN ids ON manifest.id = ids.rowid \
         JOIN names ON manifest.name = names.rowid \
         LEFT JOIN monikers ON manifest.moniker = monikers.rowid \
         JOIN versions ON manifest.version = versions.rowid \
         JOIN channels ON manifest.channel = channels.rowid \
         WHERE {where_clause} LIMIT {}",
        limit + 1
    );
    let mut rows = query_rows(
        connection,
        &sql,
        params.into_iter().map(SqlValue::Text).collect(),
        |row| {
            Ok(V1SearchRow {
                manifest_rowid: row_i64(row, 0)?,
                package_rowid: row_i64(row, 1)?,
                version: row_string(row, 2)?,
                channel: row_string(row, 3)?,
                id: row_string(row, 4)?,
                name: row_string(row, 5)?,
                moniker: row_opt_string(row, 6)?,
                match_criteria: None,
            })
        },
    )?;
    for row in &mut rows {
        row.match_criteria = infer_preindexed_match_criteria_v1(connection, row, query, semantics)?;
    }
    let truncated = rows.len() > limit;
    rows.truncate(limit);
    Ok((rows, truncated))
}

fn group_v1_rows(rows: Vec<V1SearchRow>) -> Vec<V1SearchRow> {
    let mut grouped = BTreeMap::<i64, V1SearchRow>::new();

    for row in rows {
        match grouped.get(&row.package_rowid) {
            Some(existing)
                if compare_version_and_channel(
                    &existing.version,
                    &existing.channel,
                    &row.version,
                    &row.channel,
                ) != Ordering::Less => {}
            _ => {
                grouped.insert(row.package_rowid, row);
            }
        }
    }

    grouped.into_values().collect()
}

fn query_v1_versions(connection: &Connection, package_rowid: i64) -> Result<Vec<V1VersionRow>> {
    let has_hash = table_has_column(connection, "manifest", "hash")?;
    let hash_sql = if has_hash { "manifest.hash" } else { "NULL" };
    let sql = format!(
        "SELECT versions.version, channels.channel, manifest.pathpart, {hash_sql} \
         FROM manifest \
         JOIN versions ON manifest.version = versions.rowid \
         JOIN channels ON manifest.channel = channels.rowid \
         WHERE manifest.id = ?1"
    );
    query_rows(
        connection,
        &sql,
        vec![SqlValue::Integer(package_rowid)],
        |row| {
            Ok(V1VersionRow {
                version: row_string(row, 0)?,
                channel: row_string(row, 1)?,
                pathpart_id: row_i64(row, 2)?,
                manifest_hash: row_opt_hex_string(row, 3)?,
            })
        },
    )
}

fn resolve_v1_relative_path(connection: &Connection, pathpart_id: i64) -> Result<String> {
    let mut parts = Vec::new();
    let mut current = Some(pathpart_id);

    while let Some(id) = current {
        let (parent, pathpart) = query_optional_value(
            connection,
            "SELECT parent, pathpart FROM pathparts WHERE rowid = ?1",
            vec![SqlValue::Integer(id)],
            |row| {
                Ok((
                    row_opt_string(row, 0)?
                        .and_then(|value| value.parse::<i64>().ok())
                        .filter(|value| *value != 0),
                    row_string(row, 1)?,
                ))
            },
        )?
        .ok_or_else(|| anyhow!("failed to resolve pathpart {id}"))?;
        parts.push(pathpart);
        current = parent;
    }

    parts.reverse();
    Ok(parts.join("/"))
}

fn package_version_data_relative_path(package_id: &str, package_hash: &str) -> String {
    format!(
        "packages/{}/{}/versionData.mszyml",
        package_id,
        package_hash[..package_hash.len().min(8)].to_ascii_lowercase()
    )
}

fn can_fallback_to_v1(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}").to_ascii_lowercase();
    message.contains("no such table") && message.contains("packages")
}

fn table_has_column(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    for value in query_rows(
        connection,
        &format!("PRAGMA table_info({table})"),
        Vec::new(),
        |row| row_string(row, 1),
    )? {
        if value.eq_ignore_ascii_case(column) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn build_preindexed_where_clause(
    query: &PackageQuery,
    v2: bool,
    semantics: SearchSemantics,
) -> (String, Vec<String>) {
    let rowid_column = if v2 {
        "packages.rowid"
    } else {
        "manifest.rowid"
    };
    let id_column = if v2 { "id" } else { "ids.id" };
    let name_column = if v2 { "name" } else { "names.name" };
    let moniker_column = if v2 { "moniker" } else { "monikers.moniker" };
    let exact_match = query.exact || semantics == SearchSemantics::Single;
    let mut params = Vec::new();

    if let Some(value) = &query.id {
        return (
            single_field_condition(id_column, value, exact_match, &mut params),
            params,
        );
    }
    if let Some(value) = &query.name {
        return (
            single_field_condition(name_column, value, exact_match, &mut params),
            params,
        );
    }
    if let Some(value) = &query.moniker {
        return (
            single_field_condition(moniker_column, value, exact_match, &mut params),
            params,
        );
    }
    if let Some(value) = &query.tag {
        return (
            mapped_field_condition(v2, "tag", value, rowid_column, true, &mut params),
            params,
        );
    }
    if let Some(value) = &query.command {
        return (
            mapped_field_condition(v2, "command", value, rowid_column, true, &mut params),
            params,
        );
    }
    if let Some(value) = &query.query {
        if exact_match {
            let conditions = vec![
                single_field_condition(id_column, value, true, &mut params),
                single_field_condition(name_column, value, true, &mut params),
                single_field_condition(moniker_column, value, true, &mut params),
            ];
            return (format!("({})", conditions.join(" OR ")), params);
        }

        let mut conditions = vec![
            single_field_condition(id_column, value, false, &mut params),
            single_field_condition(name_column, value, false, &mut params),
            single_field_condition(moniker_column, value, false, &mut params),
        ];
        if semantics == SearchSemantics::Many {
            conditions.push(mapped_field_condition(
                v2,
                "tag",
                value,
                rowid_column,
                false,
                &mut params,
            ));
            conditions.push(mapped_field_condition(
                v2,
                "command",
                value,
                rowid_column,
                false,
                &mut params,
            ));
        }
        return (format!("({})", conditions.join(" OR ")), params);
    }

    ("1 = 1".to_string(), Vec::new())
}

fn single_field_condition(
    column: &str,
    value: &str,
    exact: bool,
    params: &mut Vec<String>,
) -> String {
    params.push(match_parameter(value, exact));
    format!("{column} LIKE ?{}", params.len())
}

fn mapped_field_condition(
    v2: bool,
    value_name: &str,
    value: &str,
    rowid_column: &str,
    exact: bool,
    params: &mut Vec<String>,
) -> String {
    let (table_name, map_table_name, map_value_column, map_owner_column) = if v2 {
        (
            format!("{value_name}s2"),
            format!("{value_name}s2_map"),
            value_name.to_string(),
            "package".to_string(),
        )
    } else {
        (
            format!("{value_name}s"),
            format!("{value_name}s_map"),
            value_name.to_string(),
            "manifest".to_string(),
        )
    };
    params.push(match_parameter(value, exact));
    let parameter = params.len();
    format!(
        "EXISTS (SELECT 1 FROM {map_table_name} JOIN {table_name} ON \
         {map_table_name}.{value_name} = {table_name}.rowid \
         WHERE {map_table_name}.{map_owner_column} = {rowid_column} \
         AND {table_name}.{map_value_column} LIKE ?{parameter})"
    )
}

fn match_parameter(value: &str, exact: bool) -> String {
    if exact {
        value.to_string()
    } else {
        format!("%{value}%")
    }
}

fn build_rest_search_body(
    query: &PackageQuery,
    info: &RestInformation,
    semantics: SearchSemantics,
) -> Result<JsonValue> {
    let mut root = serde_json::Map::new();
    root.insert(
        "MaximumResults".to_string(),
        JsonValue::from(source_fetch_results(query, semantics) as u64),
    );
    let exact_match = query.exact || semantics == SearchSemantics::Single;

    if let Some(value) = &query.query {
        if semantics == SearchSemantics::Single {
            let mut filters = vec![
                rest_filter("PackageIdentifier", value, true),
                rest_filter("PackageName", value, true),
                rest_filter("Moniker", value, true),
            ];
            append_required_rest_filters(&mut filters, info);
            root.insert("Filters".to_string(), JsonValue::Array(filters));
            return Ok(JsonValue::Object(root));
        }

        root.insert(
            "Query".to_string(),
            serde_json::json!({
                "KeyWord": value,
                "MatchType": if exact_match { "Exact" } else { "Substring" },
            }),
        );
    }

    let mut filters = Vec::new();
    if let Some(value) = &query.id {
        filters.push(rest_filter("PackageIdentifier", value, exact_match));
    }
    if let Some(value) = &query.name {
        filters.push(rest_filter("PackageName", value, exact_match));
    }
    if let Some(value) = &query.moniker {
        filters.push(rest_filter("Moniker", value, exact_match));
    }
    if let Some(value) = &query.tag {
        filters.push(rest_filter("Tag", value, true));
    }
    if let Some(value) = &query.command {
        filters.push(rest_filter("Command", value, true));
    }
    append_required_rest_filters(&mut filters, info);

    if !filters.is_empty() {
        root.insert("Filters".to_string(), JsonValue::Array(filters));
    }

    Ok(JsonValue::Object(root))
}

fn append_required_rest_filters(filters: &mut Vec<JsonValue>, info: &RestInformation) {
    if info
        .required_package_match_fields
        .iter()
        .any(|field| field.eq_ignore_ascii_case("Market"))
    {
        filters.push(rest_filter("Market", &default_market(), true));
    }
}

fn rest_filter(field: &str, value: &str, exact: bool) -> JsonValue {
    serde_json::json!({
        "PackageMatchField": field,
        "RequestMatch": {
            "KeyWord": value,
            "MatchType": if exact { "Exact" } else { "Substring" },
        }
    })
}

fn max_results(query: &PackageQuery) -> usize {
    query.count.unwrap_or(DEFAULT_MAX_RESULTS).max(1)
}

fn source_fetch_results(query: &PackageQuery, semantics: SearchSemantics) -> usize {
    match semantics {
        SearchSemantics::Many => max_results(query).max(DEFAULT_MAX_RESULTS),
        SearchSemantics::Single => max_results(query),
    }
}

fn infer_preindexed_match_criteria_v2(
    connection: &Connection,
    row: &V2SearchRow,
    query: &PackageQuery,
    semantics: SearchSemantics,
) -> Result<Option<String>> {
    infer_match_criteria(
        &row.id,
        &row.name,
        row.moniker.as_deref(),
        query,
        semantics,
        |value| {
            find_mapped_value_v2(
                connection,
                "tags2",
                "tags2_map",
                "tag",
                row.package_rowid,
                value,
                query.exact,
            )
        },
        |value| {
            find_mapped_value_v2(
                connection,
                "commands2",
                "commands2_map",
                "command",
                row.package_rowid,
                value,
                query.exact,
            )
        },
    )
}

fn infer_preindexed_match_criteria_v1(
    connection: &Connection,
    row: &V1SearchRow,
    query: &PackageQuery,
    semantics: SearchSemantics,
) -> Result<Option<String>> {
    infer_match_criteria(
        &row.id,
        &row.name,
        row.moniker.as_deref(),
        query,
        semantics,
        |value| {
            find_mapped_value_v1(
                connection,
                "tags",
                "tags_map",
                "tag",
                row.manifest_rowid,
                value,
                query.exact,
            )
        },
        |value| {
            find_mapped_value_v1(
                connection,
                "commands",
                "commands_map",
                "command",
                row.manifest_rowid,
                value,
                query.exact,
            )
        },
    )
}

fn infer_match_criteria<FTag, FCommand>(
    id: &str,
    name: &str,
    moniker: Option<&str>,
    query: &PackageQuery,
    semantics: SearchSemantics,
    find_tag: FTag,
    find_command: FCommand,
) -> Result<Option<String>>
where
    FTag: FnOnce(&str) -> Result<Option<String>>,
    FCommand: FnOnce(&str) -> Result<Option<String>>,
{
    if let Some(value) = &query.tag {
        return Ok(Some(format_match_criteria("Tag", value)));
    }
    if let Some(value) = &query.command {
        return Ok(Some(format_match_criteria("Command", value)));
    }
    if let Some(value) = &query.moniker {
        return Ok(Some(format_match_criteria(
            "Moniker",
            moniker.unwrap_or(value),
        )));
    }
    if semantics == SearchSemantics::Single {
        return Ok(None);
    }
    if let Some(value) = &query.query {
        if matches_text(id, value, query.exact) || matches_text(name, value, query.exact) {
            return Ok(None);
        }
        if let Some(moniker_value) =
            moniker.filter(|candidate| matches_text(candidate, value, query.exact))
        {
            return Ok(Some(format_match_criteria("Moniker", moniker_value)));
        }
        if let Some(tag) = find_tag(value)? {
            return Ok(Some(format_match_criteria("Tag", &tag)));
        }
        if let Some(command) = find_command(value)? {
            return Ok(Some(format_match_criteria("Command", &command)));
        }
    }
    Ok(None)
}

fn find_mapped_value_v2(
    connection: &Connection,
    table_name: &str,
    map_table_name: &str,
    value_name: &str,
    package_rowid: i64,
    query: &str,
    exact: bool,
) -> Result<Option<String>> {
    let sql = format!(
        "SELECT {table_name}.{value_name} FROM {map_table_name} \
         JOIN {table_name} ON {map_table_name}.{value_name} = {table_name}.rowid \
         WHERE {map_table_name}.package = ?1 AND {table_name}.{value_name} LIKE ?2"
    );
    let values = query_rows(
        connection,
        &sql,
        vec![
            SqlValue::Integer(package_rowid),
            SqlValue::Text(match_parameter(query, exact)),
        ],
        |row| row_string(row, 0),
    )?;
    Ok(select_best_text_match(values, query, exact))
}

fn find_mapped_value_v1(
    connection: &Connection,
    table_name: &str,
    map_table_name: &str,
    value_name: &str,
    manifest_rowid: i64,
    query: &str,
    exact: bool,
) -> Result<Option<String>> {
    let sql = format!(
        "SELECT {table_name}.{value_name} FROM {map_table_name} \
         JOIN {table_name} ON {map_table_name}.{value_name} = {table_name}.rowid \
         WHERE {map_table_name}.manifest = ?1 AND {table_name}.{value_name} LIKE ?2"
    );
    let values = query_rows(
        connection,
        &sql,
        vec![
            SqlValue::Integer(manifest_rowid),
            SqlValue::Text(match_parameter(query, exact)),
        ],
        |row| row_string(row, 0),
    )?;
    Ok(select_best_text_match(values, query, exact))
}

fn rest_match_criteria(
    item: &JsonValue,
    query: &PackageQuery,
    semantics: SearchSemantics,
) -> Option<String> {
    if let Some(value) = &query.tag {
        return Some(format_match_criteria("Tag", value));
    }
    if let Some(value) = &query.command {
        return Some(format_match_criteria("Command", value));
    }
    if let Some(value) = &query.moniker {
        return Some(format_match_criteria("Moniker", value));
    }
    if semantics == SearchSemantics::Single {
        return None;
    }
    let value = query.query.as_deref()?;
    if json_string(item, "PackageIdentifier")
        .as_deref()
        .is_some_and(|candidate| matches_text(candidate, value, query.exact))
        || json_string(item, "PackageName")
            .as_deref()
            .is_some_and(|candidate| matches_text(candidate, value, query.exact))
    {
        return None;
    }
    json_string(item, "Moniker")
        .filter(|candidate| matches_text(candidate, value, query.exact))
        .map(|candidate| format_match_criteria("Moniker", &candidate))
}

fn format_match_criteria(field: &str, value: &str) -> String {
    format!("{field}: {value}")
}

fn matches_text(candidate: &str, query: &str, exact: bool) -> bool {
    if exact {
        candidate.eq_ignore_ascii_case(query)
    } else {
        candidate
            .to_ascii_lowercase()
            .contains(&query.to_ascii_lowercase())
    }
}

fn search_match_sort_score(candidate: &SearchMatch, query: &PackageQuery) -> usize {
    let mut score = 0;

    if let Some(value) = &query.query {
        score = score.max(score_text_match(
            &candidate.name,
            value,
            query.exact,
            140,
            50,
            30,
        ));
        score = score.max(score_text_match(
            &candidate.id,
            value,
            query.exact,
            135,
            45,
            25,
        ));
        if let Some(moniker) = candidate.moniker.as_deref() {
            score = score.max(score_text_match(moniker, value, query.exact, 130, 55, 35));
        }
        if let Some((field, matched_value)) = candidate
            .match_criteria
            .as_deref()
            .and_then(parse_match_criteria)
        {
            let field_score = match field {
                "Tag" => score_text_match(matched_value, value, query.exact, 60, 50, 40),
                "Command" => score_text_match(matched_value, value, query.exact, 55, 45, 35),
                "Moniker" => score_text_match(matched_value, value, query.exact, 125, 55, 35),
                _ => 0,
            };
            score = score.max(field_score);
        }
    }

    if let Some(value) = &query.id {
        score = score.max(score_text_match(
            &candidate.id,
            value,
            query.exact,
            220,
            200,
            180,
        ));
    }
    if let Some(value) = &query.name {
        score = score.max(score_text_match(
            &candidate.name,
            value,
            query.exact,
            210,
            190,
            170,
        ));
    }
    if let Some(value) = &query.moniker {
        if let Some(moniker) = candidate.moniker.as_deref() {
            score = score.max(score_text_match(moniker, value, query.exact, 205, 185, 165));
        }
    }
    if let Some(value) = &query.tag {
        if let Some(("Tag", matched_value)) = candidate
            .match_criteria
            .as_deref()
            .and_then(parse_match_criteria)
        {
            score = score.max(score_text_match(
                matched_value,
                value,
                query.exact,
                160,
                150,
                140,
            ));
        }
    }
    if let Some(value) = &query.command {
        if let Some(("Command", matched_value)) = candidate
            .match_criteria
            .as_deref()
            .and_then(parse_match_criteria)
        {
            score = score.max(score_text_match(
                matched_value,
                value,
                query.exact,
                155,
                145,
                135,
            ));
        }
    }

    if matches!(candidate.source_kind, SourceKind::PreIndexed) {
        score += 5;
    }
    if search_match_has_unknown_version(candidate) {
        score = score.saturating_sub(40);
    }

    score
}

fn search_match_has_unknown_version(candidate: &SearchMatch) -> bool {
    candidate
        .version
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("Unknown"))
}

fn score_text_match(
    candidate: &str,
    query: &str,
    exact: bool,
    exact_score: usize,
    prefix_score: usize,
    substring_score: usize,
) -> usize {
    if candidate.eq_ignore_ascii_case(query) {
        return exact_score;
    }
    if exact {
        return 0;
    }

    let candidate_lower = candidate.to_ascii_lowercase();
    let query_lower = query.to_ascii_lowercase();
    if candidate_lower.starts_with(&query_lower) {
        prefix_score
    } else if candidate_lower.contains(&query_lower) {
        substring_score
    } else {
        0
    }
}

fn select_best_text_match<I>(values: I, query: &str, exact: bool) -> Option<String>
where
    I: IntoIterator<Item = String>,
{
    values
        .into_iter()
        .filter_map(|candidate| {
            let score = score_text_match(&candidate, query, exact, 3, 2, 1);
            (score > 0).then_some((score, candidate))
        })
        .max_by(|(left_score, left_value), (right_score, right_value)| {
            left_score
                .cmp(right_score)
                .then_with(|| right_value.len().cmp(&left_value.len()))
                .then_with(|| {
                    right_value
                        .to_ascii_lowercase()
                        .cmp(&left_value.to_ascii_lowercase())
                })
        })
        .map(|(_, candidate)| candidate)
}

fn parse_match_criteria(criteria: &str) -> Option<(&str, &str)> {
    let (field, value) = criteria.split_once(": ")?;
    Some((field, value))
}

fn parse_rest_versions(item: &JsonValue) -> Result<Vec<VersionKey>> {
    let versions = item
        .get("Versions")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| anyhow!("REST search result missing Versions"))?;
    let mut result = Vec::new();
    for version in versions {
        let value = json_string(version, "PackageVersion")
            .ok_or_else(|| anyhow!("REST version entry missing PackageVersion"))?;
        let channel = json_string(version, "Channel").unwrap_or_default();
        result.push(VersionKey {
            version: value,
            channel,
        });
    }
    Ok(result)
}

fn parse_yaml_manifest(bytes: &[u8]) -> Result<Manifest> {
    let mut merged = YamlMapping::new();
    for document in serde_yaml::Deserializer::from_slice(bytes) {
        let value =
            YamlValue::deserialize(document).context("failed to deserialize YAML document")?;
        if let Some(mapping) = value.as_mapping() {
            merge_yaml_mapping(&mut merged, mapping);
        }
    }

    let id = yaml_string_from_root(&merged, "PackageIdentifier")
        .ok_or_else(|| anyhow!("manifest missing PackageIdentifier"))?;
    let version = yaml_string_from_root(&merged, "PackageVersion")
        .ok_or_else(|| anyhow!("manifest missing PackageVersion"))?;
    let name = yaml_localized_string(&merged, "PackageName")
        .ok_or_else(|| anyhow!("manifest missing PackageName"))?;

    let installers = parse_yaml_installers(&merged);

    Ok(Manifest {
        id,
        name,
        version,
        channel: yaml_string_from_root(&merged, "Channel").unwrap_or_default(),
        publisher: yaml_localized_string(&merged, "Publisher"),
        description: yaml_localized_string(&merged, "Description")
            .or_else(|| yaml_localized_string(&merged, "ShortDescription")),
        moniker: yaml_string_from_root(&merged, "Moniker"),
        package_url: yaml_localized_string(&merged, "PackageUrl"),
        publisher_url: yaml_localized_string(&merged, "PublisherUrl"),
        publisher_support_url: yaml_localized_string(&merged, "PublisherSupportUrl"),
        license: yaml_localized_string(&merged, "License"),
        license_url: yaml_localized_string(&merged, "LicenseUrl"),
        privacy_url: yaml_localized_string(&merged, "PrivacyUrl"),
        author: yaml_localized_string(&merged, "Author"),
        copyright: yaml_localized_string(&merged, "Copyright"),
        copyright_url: yaml_localized_string(&merged, "CopyrightUrl"),
        release_notes: yaml_localized_string(&merged, "ReleaseNotes"),
        release_notes_url: yaml_localized_string(&merged, "ReleaseNotesUrl"),
        tags: yaml_string_list(&merged, "Tags"),
        package_dependencies: yaml_package_dependencies(&merged),
        documentation: yaml_documentation_list(&merged),
        installers,
    })
}

fn parse_rest_manifest(
    bytes: &[u8],
    package_id: &str,
    version: &str,
    channel: &str,
) -> Result<Manifest> {
    let root = serde_json::from_slice::<JsonValue>(bytes)
        .context("failed to deserialize REST manifest JSON")?;
    let data = root
        .get("Data")
        .ok_or_else(|| anyhow!("REST manifest response missing Data"))?;
    let versions = data
        .get("Versions")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| anyhow!("REST manifest response missing Versions"))?;
    let selected = versions
        .iter()
        .find(|item| {
            json_string(item, "PackageVersion").as_deref() == Some(version)
                && json_string(item, "Channel").unwrap_or_default() == channel
        })
        .or_else(|| versions.first())
        .ok_or_else(|| anyhow!("REST manifest response did not contain a version payload"))?;

    let default_locale = selected
        .get("DefaultLocale")
        .ok_or_else(|| anyhow!("REST manifest response missing DefaultLocale"))?;
    let name = json_string(default_locale, "PackageName")
        .ok_or_else(|| anyhow!("REST manifest response missing PackageName"))?;

    let installers = selected
        .get("Installers")
        .and_then(JsonValue::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| Installer {
                    architecture: json_string(item, "Architecture"),
                    installer_type: json_string(item, "InstallerType"),
                    url: json_string(item, "InstallerUrl"),
                    sha256: json_string(item, "InstallerSha256"),
                    product_code: json_string(item, "ProductCode"),
                    locale: json_string(item, "InstallerLocale"),
                    scope: json_string(item, "Scope"),
                    release_date: json_string(item, "ReleaseDate"),
                    package_family_name: json_string(item, "PackageFamilyName"),
                    upgrade_code: json_string(item, "UpgradeCode"),
                    commands: json_string_list(item, "Commands"),
                    package_dependencies: json_package_dependencies(item),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(Manifest {
        id: package_id.to_string(),
        name,
        version: version.to_string(),
        channel: channel.to_string(),
        publisher: json_string(default_locale, "Publisher"),
        description: json_string(default_locale, "Description")
            .or_else(|| json_string(default_locale, "ShortDescription")),
        moniker: json_string(default_locale, "Moniker"),
        package_url: json_string(default_locale, "PackageUrl"),
        publisher_url: json_string(default_locale, "PublisherUrl"),
        publisher_support_url: json_string(default_locale, "PublisherSupportUrl"),
        license: json_string(default_locale, "License"),
        license_url: json_string(default_locale, "LicenseUrl"),
        privacy_url: json_string(default_locale, "PrivacyUrl"),
        author: json_string(default_locale, "Author"),
        copyright: json_string(default_locale, "Copyright"),
        copyright_url: json_string(default_locale, "CopyrightUrl"),
        release_notes: json_string(default_locale, "ReleaseNotes"),
        release_notes_url: json_string(default_locale, "ReleaseNotesUrl"),
        tags: json_string_list(default_locale, "Tags"),
        package_dependencies: json_package_dependencies(selected),
        documentation: json_documentation_list(default_locale),
        installers,
    })
}

fn parse_yaml_installers(root: &YamlMapping) -> Vec<Installer> {
    let base = installer_defaults(root);
    if let Some(items) = root
        .get(YamlValue::from("Installers"))
        .and_then(YamlValue::as_sequence)
    {
        let installers = items
            .iter()
            .filter_map(YamlValue::as_mapping)
            .map(|item| {
                let mut merged = base.clone();
                merge_yaml_mapping(&mut merged, item);
                installer_from_yaml(&merged)
            })
            .collect::<Vec<_>>();
        if !installers.is_empty() {
            return installers;
        }
    }

    if base.is_empty() {
        Vec::new()
    } else {
        vec![installer_from_yaml(&base)]
    }
}

fn installer_defaults(root: &YamlMapping) -> YamlMapping {
    let keys = [
        "Architecture",
        "InstallerType",
        "InstallerUrl",
        "InstallerSha256",
        "ProductCode",
        "InstallerLocale",
        "Scope",
        "ReleaseDate",
        "PackageFamilyName",
        "UpgradeCode",
        "Commands",
    ];
    let mut defaults = YamlMapping::new();
    for key in keys {
        if let Some(value) = root.get(YamlValue::from(key)) {
            defaults.insert(YamlValue::from(key), value.clone());
        }
    }
    defaults
}

fn installer_from_yaml(root: &YamlMapping) -> Installer {
    Installer {
        architecture: yaml_string(root, "Architecture"),
        installer_type: yaml_string(root, "InstallerType"),
        url: yaml_string(root, "InstallerUrl"),
        sha256: yaml_string(root, "InstallerSha256"),
        product_code: yaml_string(root, "ProductCode"),
        locale: yaml_string(root, "InstallerLocale"),
        scope: yaml_string(root, "Scope"),
        release_date: yaml_string(root, "ReleaseDate"),
        package_family_name: yaml_string(root, "PackageFamilyName"),
        upgrade_code: yaml_string(root, "UpgradeCode"),
        commands: yaml_string_list(root, "Commands"),
        package_dependencies: yaml_package_dependencies(root),
    }
}

fn yaml_localized_string(root: &YamlMapping, key: &str) -> Option<String> {
    yaml_string(root, key).or_else(|| {
        root.get(YamlValue::from("DefaultLocale"))
            .and_then(YamlValue::as_mapping)
            .and_then(|mapping| yaml_string(mapping, key))
    })
}

fn yaml_string_from_root(root: &YamlMapping, key: &str) -> Option<String> {
    yaml_string(root, key)
}

fn yaml_string(root: &YamlMapping, key: &str) -> Option<String> {
    root.get(YamlValue::from(key)).and_then(yaml_scalar_string)
}

fn yaml_scalar_string(value: &YamlValue) -> Option<String> {
    match value {
        YamlValue::Null => None,
        YamlValue::Bool(value) => Some(value.to_string()),
        YamlValue::Number(value) => Some(value.to_string()),
        YamlValue::String(value) => Some(value.clone()),
        YamlValue::Tagged(tagged) => yaml_scalar_string(&tagged.value),
        YamlValue::Sequence(_) | YamlValue::Mapping(_) => None,
    }
}

fn yaml_string_list(root: &YamlMapping, key: &str) -> Vec<String> {
    root.get(YamlValue::from(key))
        .and_then(YamlValue::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(YamlValue::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn yaml_documentation_list(root: &YamlMapping) -> Vec<Documentation> {
    root.get(YamlValue::from("Documentations"))
        .and_then(YamlValue::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(YamlValue::as_mapping)
                .filter_map(|item| {
                    let url = yaml_string(item, "DocumentUrl")?;
                    Some(Documentation {
                        label: yaml_string(item, "DocumentLabel"),
                        url,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn yaml_package_dependencies(root: &YamlMapping) -> Vec<String> {
    root.get(YamlValue::from("Dependencies"))
        .and_then(YamlValue::as_mapping)
        .and_then(|deps| deps.get(YamlValue::from("PackageDependencies")))
        .and_then(YamlValue::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(YamlValue::as_mapping)
                .filter_map(|item| yaml_string(item, "PackageIdentifier"))
                .collect()
        })
        .unwrap_or_default()
}

fn json_string(value: &JsonValue, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .map(str::to_string)
}

fn json_string_list(value: &JsonValue, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(JsonValue::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(JsonValue::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn json_documentation_list(value: &JsonValue) -> Vec<Documentation> {
    value
        .get("Documentations")
        .and_then(JsonValue::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let url = json_string(item, "DocumentUrl")?;
                    Some(Documentation {
                        label: json_string(item, "DocumentLabel"),
                        url,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn json_package_dependencies(value: &JsonValue) -> Vec<String> {
    value
        .get("Dependencies")
        .and_then(JsonValue::as_object)
        .and_then(|deps| deps.get("PackageDependencies"))
        .and_then(JsonValue::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| json_string(item, "PackageIdentifier"))
                .collect()
        })
        .unwrap_or_default()
}

fn merge_yaml_mapping(target: &mut YamlMapping, source: &YamlMapping) {
    for (key, value) in source {
        target.insert(key.clone(), value.clone());
    }
}

fn extract_zip_member(bytes: &[u8], member_name: &str) -> Result<Vec<u8>> {
    let reader = Cursor::new(bytes.to_vec());
    let mut archive = ZipArchive::new(reader).context("failed to read zip payload")?;
    let mut file = archive
        .by_name(member_name)
        .with_context(|| format!("zip payload missing {member_name}"))?;
    let mut output = Vec::new();
    file.read_to_end(&mut output)
        .with_context(|| format!("failed to read {member_name} from zip payload"))?;
    Ok(output)
}

fn decompress_mszyml(bytes: &[u8]) -> Result<String> {
    if let Ok(output) = decompress_mszyml_payload(if bytes.starts_with(b"CK") {
        &bytes[2..]
    } else {
        bytes
    }) {
        return Ok(output);
    }

    if !bytes.starts_with(b"CK") {
        if let Some(offset) = bytes.windows(2).position(|window| window == b"CK") {
            return decompress_mszyml_payload(&bytes[offset + 2..]);
        }
    }

    decompress_mszyml_payload(bytes).context("failed to decompress MSZIP payload")
}

fn decompress_mszyml_payload(payload: &[u8]) -> Result<String> {
    let mut decoder = flate2::read::DeflateDecoder::new(payload);
    let mut output = String::new();
    decoder.read_to_string(&mut output)?;
    Ok(output)
}

fn cache_control_max_age(response: &Response) -> u64 {
    response
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .and_then(|header| {
            header.split(',').find_map(|part| {
                let trimmed = part.trim();
                trimmed
                    .strip_prefix("max-age=")
                    .and_then(|value| value.parse::<u64>().ok())
            })
        })
        .unwrap_or(60)
}

fn choose_contract(server_versions: &[String]) -> Option<&'static str> {
    REST_SUPPORTED_CONTRACTS.iter().copied().find(|candidate| {
        let (major, minor) = major_minor(candidate);
        server_versions.iter().any(|version| {
            let (server_major, server_minor) = major_minor(version);
            server_major == major && server_minor == minor
        })
    })
}

fn major_minor(version: &str) -> (String, String) {
    let mut parts = version.split('.');
    (
        parts.next().unwrap_or_default().to_string(),
        parts.next().unwrap_or_default().to_string(),
    )
}

fn default_market() -> String {
    std::env::var("WINGET_RS_MARKET").unwrap_or_else(|_| DEFAULT_MARKET.to_string())
}

fn select_installer(installers: &[Installer], query: &PackageQuery) -> Option<Installer> {
    let requested_locale = query.locale.as_deref();
    let requested_architecture = query.installer_architecture.as_deref();
    let requested_type = query.installer_type.as_deref();
    let requested_scope = query.install_scope.as_deref();
    let system_architecture = current_architecture();

    installers
        .iter()
        .filter(|installer| installer_matches_requested(installer, requested_type, requested_scope))
        .filter(|installer| {
            installer_matches_architecture(installer, requested_architecture, system_architecture)
        })
        .max_by_key(|installer| {
            installer_rank(
                installer,
                requested_locale,
                requested_architecture,
                system_architecture,
            )
        })
        .cloned()
}

fn installer_matches_requested(
    installer: &Installer,
    requested_type: Option<&str>,
    requested_scope: Option<&str>,
) -> bool {
    matches_optional_ci(installer.installer_type.as_deref(), requested_type)
        && matches_optional_ci(installer.scope.as_deref(), requested_scope)
}

fn installer_matches_architecture(
    installer: &Installer,
    requested_architecture: Option<&str>,
    system_architecture: &str,
) -> bool {
    let Some(architecture) = installer.architecture.as_deref() else {
        return true;
    };

    let architecture = architecture.to_ascii_lowercase();
    if let Some(requested) = requested_architecture {
        return architecture.eq_ignore_ascii_case(requested);
    }

    preferred_architectures(system_architecture)
        .iter()
        .any(|candidate| architecture.eq_ignore_ascii_case(candidate))
}

fn installer_rank(
    installer: &Installer,
    requested_locale: Option<&str>,
    requested_architecture: Option<&str>,
    system_architecture: &str,
) -> (i32, i32, i32) {
    let architecture_rank = architecture_rank(
        installer.architecture.as_deref(),
        requested_architecture.unwrap_or(system_architecture),
        requested_architecture.is_some(),
        system_architecture,
    );
    let locale_rank = locale_rank(installer.locale.as_deref(), requested_locale);
    let command_rank = if installer.commands.is_empty() { 0 } else { 1 };
    (architecture_rank, locale_rank, command_rank)
}

fn architecture_rank(
    installer_architecture: Option<&str>,
    preferred_architecture: &str,
    strict: bool,
    system_architecture: &str,
) -> i32 {
    let Some(value) = installer_architecture else {
        return 0;
    };
    if value.eq_ignore_ascii_case(preferred_architecture) {
        return 5;
    }
    if value.eq_ignore_ascii_case("neutral") {
        return 4;
    }
    if strict {
        return -1;
    }

    preferred_architectures(system_architecture)
        .iter()
        .rev()
        .position(|candidate| value.eq_ignore_ascii_case(candidate))
        .map(|index| index as i32 + 1)
        .unwrap_or(-1)
}

fn preferred_architectures(system_architecture: &str) -> &'static [&'static str] {
    match system_architecture {
        "arm64" => &["arm64", "neutral", "x64", "x86"],
        "x64" => &["x64", "neutral", "x86"],
        "x86" => &["x86", "neutral"],
        _ => &["neutral"],
    }
}

fn locale_rank(installer_locale: Option<&str>, requested_locale: Option<&str>) -> i32 {
    match (installer_locale, requested_locale) {
        (Some(installer), Some(requested)) if installer.eq_ignore_ascii_case(requested) => 3,
        (Some(installer), Some(requested))
            if installer
                .split('-')
                .next()
                .zip(requested.split('-').next())
                .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right)) =>
        {
            2
        }
        (None, Some(_)) => 1,
        (Some(_), None) => 1,
        (None, None) => 0,
        _ => -1,
    }
}

fn matches_optional_ci(value: Option<&str>, requested: Option<&str>) -> bool {
    match requested {
        Some(requested) => value.is_some_and(|value| value.eq_ignore_ascii_case(requested)),
        None => true,
    }
}

fn current_architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        _ => "neutral",
    }
}

fn select_v1_version(
    versions: &[V1VersionRow],
    requested_version: Option<&str>,
    requested_channel: Option<&str>,
) -> Result<V1VersionRow> {
    let mut ordered = versions.to_vec();
    ordered.sort_by(|left, right| {
        compare_version_and_channel(&right.version, &right.channel, &left.version, &left.channel)
    });

    if let Some(version) = requested_version {
        let channel = requested_channel.unwrap_or_default();
        return ordered
            .iter()
            .find(|candidate| {
                candidate.version.eq_ignore_ascii_case(version)
                    && (channel.is_empty() || candidate.channel.eq_ignore_ascii_case(channel))
            })
            .cloned()
            .ok_or_else(|| anyhow!("requested version {version} was not found"));
    }

    ordered
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("package had no versions"))
}

fn select_v2_version(
    versions: &[PackageVersionDataEntry],
    requested_version: Option<&str>,
) -> Result<PackageVersionDataEntry> {
    let mut ordered = versions.to_vec();
    ordered.sort_by(|left, right| compare_version(&right.version, &left.version));

    if let Some(version) = requested_version {
        return ordered
            .iter()
            .find(|candidate| candidate.version.eq_ignore_ascii_case(version))
            .cloned()
            .ok_or_else(|| anyhow!("requested version {version} was not found"));
    }

    ordered
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("package had no V2 versions"))
}

fn select_rest_version<'a>(
    versions: &'a [VersionKey],
    requested_version: Option<&str>,
    requested_channel: Option<&str>,
) -> Result<&'a VersionKey> {
    if let Some(version) = requested_version {
        let channel = requested_channel.unwrap_or_default();
        return versions
            .iter()
            .find(|candidate| {
                candidate.version.eq_ignore_ascii_case(version)
                    && (channel.is_empty() || candidate.channel.eq_ignore_ascii_case(channel))
            })
            .ok_or_else(|| anyhow!("requested version {version} was not found"));
    }

    versions
        .first()
        .ok_or_else(|| anyhow!("package had no REST versions"))
}

fn sort_versions_desc(versions: &mut [VersionKey]) {
    versions.sort_by(|left, right| {
        compare_version_and_channel(&right.version, &right.channel, &left.version, &left.channel)
    });
}

fn compare_version_and_channel(
    left_version: &str,
    left_channel: &str,
    right_version: &str,
    right_channel: &str,
) -> Ordering {
    compare_version(left_version, right_version).then_with(|| {
        left_channel
            .to_ascii_lowercase()
            .cmp(&right_channel.to_ascii_lowercase())
    })
}

fn compare_version(left: &str, right: &str) -> Ordering {
    let left_parts = tokenize_version(left);
    let right_parts = tokenize_version(right);
    let max_len = left_parts.len().max(right_parts.len());

    for index in 0..max_len {
        let left_part = left_parts.get(index).map(String::as_str).unwrap_or("0");
        let right_part = right_parts.get(index).map(String::as_str).unwrap_or("0");
        let numeric = left_part
            .parse::<u64>()
            .ok()
            .zip(right_part.parse::<u64>().ok());

        let ordering = if let Some((left_number, right_number)) = numeric {
            left_number.cmp(&right_number)
        } else {
            left_part
                .to_ascii_lowercase()
                .cmp(&right_part.to_ascii_lowercase())
        };

        if ordering != Ordering::Equal {
            return ordering;
        }
    }

    Ordering::Equal
}

fn tokenize_version(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut current_is_digit = None;

    for ch in value.chars() {
        if ch == '.' || ch == '-' || ch == '_' || ch == '+' {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            current_is_digit = None;
            continue;
        }

        let is_digit = ch.is_ascii_digit();
        match current_is_digit {
            Some(previous) if previous != is_digit => {
                parts.push(std::mem::take(&mut current));
                current.push(ch);
                current_is_digit = Some(is_digit);
            }
            Some(_) => current.push(ch),
            None => {
                current.push(ch);
                current_is_digit = Some(is_digit);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn verify_hash(expected_hash: &str, bytes: &[u8]) -> Result<()> {
    if !hash_matches(Some(expected_hash), bytes) {
        bail!("downloaded file hash did not match expected SHA256");
    }
    Ok(())
}

fn hash_matches(expected_hash: Option<&str>, bytes: &[u8]) -> bool {
    expected_hash
        .map(|expected| sha256_hex(bytes).eq_ignore_ascii_case(expected))
        .unwrap_or(true)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02X}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;
    use std::path::Path;

    #[test]
    fn parses_fixture_manifest() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("..")
            .join("src")
            .join("AppInstallerCLITests")
            .join("TestData")
            .join("ManifestV1_28-Singleton.yaml");
        let bytes = fs::read(root).expect("fixture bytes");
        let manifest = parse_yaml_manifest(&bytes).expect("manifest");
        assert_eq!(manifest.id, "microsoft.msixsdk");
        assert_eq!(manifest.version, "1.7.32");
        assert_eq!(manifest.name, "MSIX SDK");
        assert!(!manifest.installers.is_empty());
    }

    #[test]
    fn decompresses_mszyml_payload() {
        let payload =
            "sV: 1.0.0\nvD:\n  - v: 1.2.3\n    rP: manifests/test.yaml\n    s256H: ABCD\n";
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(payload.as_bytes())
            .expect("write payload");
        let compressed = encoder.finish().expect("finish payload");
        let mut mszip = b"CK".to_vec();
        mszip.extend_from_slice(&compressed);

        let decompressed = decompress_mszyml(&mszip).expect("decompress");
        let parsed =
            serde_yaml::from_str::<PackageVersionDataDocument>(&decompressed).expect("parse");
        assert_eq!(parsed.versions[0].version, "1.2.3");
    }

    #[test]
    fn decompresses_mszyml_payload_with_prefix() {
        let payload =
            "sV: 1.0.0\nvD:\n  - v: 1.2.3\n    rP: manifests/test.yaml\n    s256H: ABCD\n";
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(payload.as_bytes())
            .expect("write payload");
        let compressed = encoder.finish().expect("finish payload");
        let mut prefixed = vec![0u8; 28];
        prefixed.extend_from_slice(b"CK");
        prefixed.extend_from_slice(&compressed);

        let decompressed = decompress_mszyml(&prefixed).expect("decompress");
        let parsed =
            serde_yaml::from_str::<PackageVersionDataDocument>(&decompressed).expect("parse");
        assert_eq!(parsed.versions[0].version, "1.2.3");
    }

    #[test]
    fn compares_versions_naturally() {
        assert_eq!(compare_version("1.10.0", "1.9.9"), Ordering::Greater);
        assert_eq!(compare_version("2.0", "10.0"), Ordering::Less);
        assert_eq!(
            compare_version("1.0.0-preview2", "1.0.0-preview1"),
            Ordering::Greater
        );
    }

    #[test]
    fn search_query_many_includes_tag_and_command_conditions() {
        let query = PackageQuery {
            query: Some("terminal".to_string()),
            ..PackageQuery::default()
        };

        let (where_clause, params) =
            build_preindexed_where_clause(&query, true, SearchSemantics::Many);

        assert!(where_clause.contains("tags2"));
        assert!(where_clause.contains("commands2"));
        assert_eq!(params.len(), 5);
    }

    #[test]
    fn show_query_single_omits_tag_and_command_conditions() {
        let query = PackageQuery {
            query: Some("terminal".to_string()),
            ..PackageQuery::default()
        };

        let (where_clause, params) =
            build_preindexed_where_clause(&query, true, SearchSemantics::Single);

        assert!(!where_clause.contains("tags2"));
        assert!(!where_clause.contains("commands2"));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn explicit_tag_filter_uses_exact_match() {
        let query = PackageQuery {
            tag: Some("terminal".to_string()),
            ..PackageQuery::default()
        };

        let (where_clause, params) =
            build_preindexed_where_clause(&query, true, SearchSemantics::Many);

        assert!(where_clause.contains("tags2"));
        assert_eq!(params, vec!["terminal".to_string()]);
    }

    #[test]
    fn rest_tag_filter_uses_exact_match() {
        let query = PackageQuery {
            tag: Some("terminal".to_string()),
            ..PackageQuery::default()
        };
        let info = RestInformation {
            required_package_match_fields: Vec::new(),
            unsupported_package_match_fields: Vec::new(),
            required_query_parameters: Vec::new(),
            ..RestInformation::default()
        };

        let body = build_rest_search_body(&query, &info, SearchSemantics::Many).expect("rest body");
        let filters = body["Filters"].as_array().expect("filters");

        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0]["PackageMatchField"], "Tag");
        assert_eq!(filters[0]["RequestMatch"]["MatchType"], "Exact");
        assert_eq!(filters[0]["RequestMatch"]["KeyWord"], "terminal");
    }

    #[test]
    fn selects_installer_using_requested_filters() {
        let installers = vec![
            Installer {
                architecture: Some("x86".to_string()),
                installer_type: Some("zip".to_string()),
                url: None,
                sha256: None,
                product_code: None,
                locale: Some("en-US".to_string()),
                scope: Some("user".to_string()),
                release_date: None,
                package_family_name: None,
                upgrade_code: None,
                commands: Vec::new(),
                package_dependencies: Vec::new(),
            },
            Installer {
                architecture: Some("x64".to_string()),
                installer_type: Some("msix".to_string()),
                url: None,
                sha256: None,
                product_code: None,
                locale: Some("en-US".to_string()),
                scope: Some("user".to_string()),
                release_date: None,
                package_family_name: None,
                upgrade_code: None,
                commands: vec!["demo".to_string()],
                package_dependencies: Vec::new(),
            },
        ];
        let query = PackageQuery {
            installer_type: Some("msix".to_string()),
            installer_architecture: Some("x64".to_string()),
            install_scope: Some("user".to_string()),
            locale: Some("en-US".to_string()),
            ..PackageQuery::default()
        };

        let selected = select_installer(&installers, &query).expect("selected installer");
        assert_eq!(selected.installer_type.as_deref(), Some("msix"));
        assert_eq!(selected.architecture.as_deref(), Some("x64"));
    }

    #[test]
    fn derives_correlation_name_candidates() {
        let candidates = correlation_name_candidates("PowerToys (Preview) x64");
        assert!(candidates.contains(&"PowerToys (Preview) x64".to_string()));
        assert!(candidates.contains(&"PowerToys".to_string()));
    }

    #[test]
    fn correlates_installed_package_to_available_match() {
        let installed = InstalledPackage {
            name: "PowerToys (Preview) x64".to_string(),
            local_id: r"ARP\Machine\X64\PowerToys".to_string(),
            installed_version: "0.98.1".to_string(),
            publisher: None,
            scope: Some("Machine".to_string()),
            installer_category: Some("exe".to_string()),
            install_location: None,
            package_family_names: Vec::new(),
            product_codes: Vec::new(),
            upgrade_codes: Vec::new(),
            correlated: None,
        };
        let candidates = vec![SearchMatch {
            source_name: "winget".to_string(),
            source_kind: SourceKind::PreIndexed,
            id: "Microsoft.PowerToys".to_string(),
            name: "PowerToys".to_string(),
            moniker: None,
            version: Some("0.98.1".to_string()),
            channel: None,
            match_criteria: None,
        }];

        let correlated =
            correlate_installed_package(&installed, &candidates, true).expect("correlated");
        assert_eq!(correlated.id, "Microsoft.PowerToys");
    }

    #[test]
    fn list_query_uses_available_lookup_for_tag_filters() {
        let query = ListQuery {
            tag: Some("terminal".to_string()),
            ..ListQuery::default()
        };

        assert!(list_query_needs_available_lookup(&query));
    }

    #[test]
    fn list_tag_filter_requires_correlated_match() {
        let package = InstalledPackage {
            name: "PowerToys (Preview) x64".to_string(),
            local_id: r"ARP\Machine\X64\PowerToys".to_string(),
            installed_version: "0.98.1".to_string(),
            publisher: None,
            scope: Some("Machine".to_string()),
            installer_category: Some("exe".to_string()),
            install_location: None,
            package_family_names: Vec::new(),
            product_codes: Vec::new(),
            upgrade_codes: Vec::new(),
            correlated: None,
        };
        let query = ListQuery {
            tag: Some("powertoys".to_string()),
            ..ListQuery::default()
        };

        assert!(!list_package_matches(&package, &query));
    }

    #[test]
    fn list_lookup_ignores_display_count() {
        let query = ListQuery {
            query: Some("git".to_string()),
            count: Some(5),
            ..ListQuery::default()
        };

        let package_query = package_query_from_list_query(&query);
        assert_eq!(package_query.count, Some(LIST_LOOKUP_MAX_RESULTS));
    }

    #[test]
    fn strict_list_correlation_avoids_short_substring_matches() {
        let installed = InstalledPackage {
            name: "GitHub".to_string(),
            local_id: r"ARP\User\X64\GitHub".to_string(),
            installed_version: "1.0.0".to_string(),
            publisher: None,
            scope: Some("User".to_string()),
            installer_category: Some("exe".to_string()),
            install_location: None,
            package_family_names: Vec::new(),
            product_codes: Vec::new(),
            upgrade_codes: Vec::new(),
            correlated: None,
        };
        let candidates = vec![SearchMatch {
            source_name: "winget".to_string(),
            source_kind: SourceKind::PreIndexed,
            id: "Git.Git.PreRelease".to_string(),
            name: "Git".to_string(),
            moniker: None,
            version: Some("2.54.0".to_string()),
            channel: None,
            match_criteria: None,
        }];

        assert!(correlate_installed_package(&installed, &candidates, false).is_none());
    }

    #[test]
    fn detects_available_upgrade_from_correlated_package() {
        let package = InstalledPackage {
            name: "AzCopy v10".to_string(),
            local_id: r"ARP\Machine\X64\AzCopy".to_string(),
            installed_version: "10.32.2".to_string(),
            publisher: None,
            scope: Some("Machine".to_string()),
            installer_category: Some("exe".to_string()),
            install_location: None,
            package_family_names: Vec::new(),
            product_codes: Vec::new(),
            upgrade_codes: Vec::new(),
            correlated: Some(SearchMatch {
                source_name: "winget".to_string(),
                source_kind: SourceKind::PreIndexed,
                id: "Microsoft.Azure.AZCopy.10".to_string(),
                name: "AzCopy v10".to_string(),
                moniker: None,
                version: Some("10.32.3".to_string()),
                channel: None,
                match_criteria: None,
            }),
        };

        assert!(installed_package_has_upgrade(&package));
    }

    #[test]
    fn include_unknown_treats_unknown_version_as_upgradable_when_correlated() {
        let package = InstalledPackage {
            name: "Example Tool".to_string(),
            local_id: r"ARP\User\X64\ExampleTool".to_string(),
            installed_version: "Unknown".to_string(),
            publisher: None,
            scope: Some("User".to_string()),
            installer_category: Some("exe".to_string()),
            install_location: None,
            package_family_names: Vec::new(),
            product_codes: Vec::new(),
            upgrade_codes: Vec::new(),
            correlated: Some(SearchMatch {
                source_name: "winget".to_string(),
                source_kind: SourceKind::PreIndexed,
                id: "Contoso.ExampleTool".to_string(),
                name: "Example Tool".to_string(),
                moniker: None,
                version: Some("2.0.0".to_string()),
                channel: None,
                match_criteria: None,
            }),
        };
        let query = ListQuery {
            upgrade_only: true,
            include_unknown: true,
            ..Default::default()
        };

        assert!(installed_package_matches_upgrade_filter(&package, &query));
        assert_eq!(
            list_match_from_installed(package)
                .available_version
                .as_deref(),
            Some("2.0.0")
        );
    }

    #[test]
    fn parses_msix_package_full_name_into_version_and_family() {
        let parsed = parse_msix_package_full_name(
            "Microsoft.PowerToys.SparseApp_0.98.1.0_neutral__8wekyb3d8bbwe",
        )
        .expect("package metadata");

        assert_eq!(parsed.version, "0.98.1.0");
        assert_eq!(
            parsed.family_name,
            "Microsoft.PowerToys.SparseApp_8wekyb3d8bbwe"
        );
    }

    #[test]
    fn recognizes_windows_system_paths() {
        assert!(is_windows_system_path(r"C:\Windows\SystemApps\Contoso"));
        assert!(!is_windows_system_path(
            r"C:\Users\mamoreau\AppData\Local\PowerToys\WinUI3Apps"
        ));
    }

    #[test]
    fn search_ranking_prefers_exact_name_over_tag_match() {
        let query = PackageQuery {
            query: Some("PowerToys".to_string()),
            ..Default::default()
        };
        let exact_name = SearchMatch {
            source_name: "winget".to_string(),
            source_kind: SourceKind::PreIndexed,
            id: "Microsoft.PowerToys".to_string(),
            name: "PowerToys".to_string(),
            moniker: Some("powertoys".to_string()),
            version: Some("0.98.1".to_string()),
            channel: None,
            match_criteria: Some("Moniker: powertoys".to_string()),
        };
        let tag_match = SearchMatch {
            source_name: "winget".to_string(),
            source_kind: SourceKind::PreIndexed,
            id: "JiriPolasek.QRCodesforCommandPalette".to_string(),
            name: "QR Codes for Command Palette".to_string(),
            moniker: None,
            version: Some("0.4.0.0".to_string()),
            channel: None,
            match_criteria: Some("Tag: microsoft-powertoys".to_string()),
        };

        assert!(
            search_match_sort_score(&exact_name, &query)
                > search_match_sort_score(&tag_match, &query)
        );
    }

    #[test]
    fn select_best_text_match_prefers_exact_tag_value() {
        let values = vec![
            "microsoft-powertoys".to_string(),
            "powertoys-run".to_string(),
            "powertoys".to_string(),
        ];

        assert_eq!(
            select_best_text_match(values, "PowerToys", false).as_deref(),
            Some("powertoys")
        );
    }

    #[test]
    fn search_match_criteria_is_blank_for_name_match() {
        let query = PackageQuery {
            query: Some("PowerToys".to_string()),
            ..Default::default()
        };

        let result = infer_match_criteria(
            "Microsoft.PowerToys",
            "PowerToys",
            Some("powertoys"),
            &query,
            SearchSemantics::Many,
            |_| Ok(Some("powertoys".to_string())),
            |_| Ok(None),
        )
        .expect("match criteria");

        assert_eq!(result, None);
    }

    #[test]
    fn search_ranking_demotes_unknown_version_result() {
        let query = PackageQuery {
            query: Some("PowerToys".to_string()),
            ..Default::default()
        };
        let unknown_version = SearchMatch {
            source_name: "msstore".to_string(),
            source_kind: SourceKind::Rest,
            id: "XP89DCGQ3K6VLD".to_string(),
            name: "Microsoft PowerToys".to_string(),
            moniker: None,
            version: Some("Unknown".to_string()),
            channel: None,
            match_criteria: None,
        };
        let tag_match = SearchMatch {
            source_name: "winget".to_string(),
            source_kind: SourceKind::PreIndexed,
            id: "Riri.QRCodeforCmdPal".to_string(),
            name: "QR Code for CmdPal".to_string(),
            moniker: None,
            version: Some("0.0.1.0".to_string()),
            channel: None,
            match_criteria: Some("Tag: powertoys".to_string()),
        };

        assert!(
            search_match_sort_score(&tag_match, &query)
                > search_match_sort_score(&unknown_version, &query)
        );
    }

    #[test]
    fn search_source_fetch_results_ignore_small_display_count() {
        let query = PackageQuery {
            count: Some(5),
            ..Default::default()
        };

        assert_eq!(source_fetch_results(&query, SearchSemantics::Many), 50);
        assert_eq!(source_fetch_results(&query, SearchSemantics::Single), 5);
    }

    #[test]
    fn search_ranking_prefers_exact_tag_over_plain_name_prefix() {
        let query = PackageQuery {
            query: Some("PowerToys".to_string()),
            ..Default::default()
        };
        let prefix_name = SearchMatch {
            source_name: "winget".to_string(),
            source_kind: SourceKind::PreIndexed,
            id: "advaith.CurrencyConverterPowerToys".to_string(),
            name: "PowerToys-Run-Currency-Converter".to_string(),
            moniker: None,
            version: Some("1.5.4".to_string()),
            channel: None,
            match_criteria: None,
        };
        let exact_tag = SearchMatch {
            source_name: "winget".to_string(),
            source_kind: SourceKind::PreIndexed,
            id: "Riri.QRCodeforCmdPal".to_string(),
            name: "QR Code for CmdPal".to_string(),
            moniker: None,
            version: Some("0.0.1.0".to_string()),
            channel: None,
            match_criteria: Some("Tag: powertoys".to_string()),
        };

        assert!(
            search_match_sort_score(&exact_tag, &query)
                > search_match_sort_score(&prefix_name, &query)
        );
    }

    #[test]
    fn list_sort_prefers_main_package_then_sparse_app() {
        let main = InstalledPackage {
            name: "PowerToys (Preview) x64".to_string(),
            local_id: r"ARP\User\X64\PowerToys".to_string(),
            installed_version: "0.98.1".to_string(),
            publisher: None,
            scope: Some("User".to_string()),
            installer_category: Some("exe".to_string()),
            install_location: None,
            package_family_names: Vec::new(),
            product_codes: Vec::new(),
            upgrade_codes: Vec::new(),
            correlated: None,
        };
        let sparse = InstalledPackage {
            name: "PowerToys.SparseApp".to_string(),
            local_id: r"MSIX\Microsoft.PowerToys.SparseApp_0.98.1.0_neutral__8wekyb3d8bbwe"
                .to_string(),
            installed_version: "0.98.1.0".to_string(),
            publisher: None,
            scope: Some("User".to_string()),
            installer_category: Some("msix".to_string()),
            install_location: None,
            package_family_names: Vec::new(),
            product_codes: Vec::new(),
            upgrade_codes: Vec::new(),
            correlated: None,
        };
        let extension = InstalledPackage {
            name: "PowerToys FileLocksmith Context Menu".to_string(),
            local_id:
                r"MSIX\Microsoft.PowerToys.FileLocksmithContextMenu_0.98.1.0_neutral__8wekyb3d8bbwe"
                    .to_string(),
            installed_version: "0.98.1.0".to_string(),
            publisher: None,
            scope: Some("User".to_string()),
            installer_category: Some("msix".to_string()),
            install_location: None,
            package_family_names: Vec::new(),
            product_codes: Vec::new(),
            upgrade_codes: Vec::new(),
            correlated: None,
        };

        assert!(list_sort_weight(&main) < list_sort_weight(&sparse));
        assert!(list_sort_weight(&sparse) < list_sort_weight(&extension));
    }
}
