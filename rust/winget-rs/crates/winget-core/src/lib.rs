use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_yaml::{Mapping as YamlMapping, Value as YamlValue};
use sha2::{Digest, Sha256};
use rusqlite::{
    Connection, OpenFlags, Row as SqlRow, params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use zip::ZipArchive;

const DEFAULT_MARKET: &str = "US";
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
    pub source: Option<String>,
    pub exact: bool,
    pub version: Option<String>,
    pub channel: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub source_name: String,
    pub source_kind: SourceKind,
    pub id: String,
    pub name: String,
    pub moniker: Option<String>,
    pub version: Option<String>,
    pub channel: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchResponse {
    pub matches: Vec<SearchMatch>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionKey {
    pub version: String,
    pub channel: String,
}

#[derive(Debug, Clone)]
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
    pub release_notes: Option<String>,
    pub release_notes_url: Option<String>,
    pub tags: Vec<String>,
    pub installers: Vec<Installer>,
}

#[derive(Debug, Clone)]
pub struct Installer {
    pub architecture: Option<String>,
    pub installer_type: Option<String>,
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub product_code: Option<String>,
    pub locale: Option<String>,
    pub scope: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShowResult {
    pub package: SearchMatch,
    pub manifest: Manifest,
    pub cached_files: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct VersionsResult {
    pub package: SearchMatch,
    pub versions: Vec<VersionKey>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CacheWarmResult {
    pub package: SearchMatch,
    pub cached_files: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SourceUpdateResult {
    pub name: String,
    pub kind: SourceKind,
    pub detail: String,
}

#[derive(Debug, Clone)]
struct LocatedMatch {
    display: SearchMatch,
    source_index: usize,
    locator: MatchLocator,
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
        let (matches, warnings) = self.search_located(query, SearchSemantics::Many)?;
        Ok(SearchResponse {
            matches: matches.into_iter().map(|item| item.display).collect(),
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

        Ok(ShowResult {
            package: located.display,
            manifest,
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
        let (matches, warnings) = self.search_located(query, SearchSemantics::Single)?;

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
    ) -> Result<(Vec<LocatedMatch>, Vec<String>)> {
        let indexes = self.resolve_source_indexes(query.source.as_deref())?;
        let mut matches = Vec::new();
        let mut warnings = Vec::new();

        for index in indexes {
            match self.search_source(index, query, semantics) {
                Ok(mut source_matches) => matches.append(&mut source_matches),
                Err(error) => warnings.push(format!("{}: {error:#}", self.store.sources[index].name)),
            }
        }

        Ok((matches, warnings))
    }

    fn search_source(
        &mut self,
        source_index: usize,
        query: &PackageQuery,
        semantics: SearchSemantics,
    ) -> Result<Vec<LocatedMatch>> {
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
    ) -> Result<Vec<LocatedMatch>> {
        let connection = self.open_preindexed_connection(source_index)?;
        let source = self.source_clone(source_index);

        match query_v2_matches(&connection, query, semantics) {
            Ok(rows) => Ok(rows
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
                    },
                    source_index,
                    locator: MatchLocator::PreIndexedV2 {
                        package_rowid: row.package_rowid,
                        package_hash: row.package_hash,
                    },
                })
                .collect()),
            Err(error) if can_fallback_to_v1(&error) => {
                let rows = query_v1_matches(&connection, query, semantics)?;
                let grouped = group_v1_rows(rows);

                Ok(grouped
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
                        },
                        source_index,
                        locator: MatchLocator::PreIndexedV1 {
                            package_rowid: row.package_rowid,
                        },
                    })
                    .collect())
            }
            Err(error) => Err(error),
        }
    }

    fn search_rest(
        &mut self,
        source_index: usize,
        query: &PackageQuery,
        semantics: SearchSemantics,
    ) -> Result<Vec<LocatedMatch>> {
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

        let mut results = Vec::new();
        for item in data {
            let package_id = json_string(&item, "PackageIdentifier")
                .ok_or_else(|| anyhow!("REST search result missing PackageIdentifier"))?;
            let package_name = json_string(&item, "PackageName")
                .ok_or_else(|| anyhow!("REST search result missing PackageName"))?;
            let publisher = json_string(&item, "Publisher");
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
                    moniker: publisher,
                    version: Some(latest.version.clone()),
                    channel: if latest.channel.is_empty() {
                        None
                    } else {
                        Some(latest.channel.clone())
                    },
                },
                source_index,
                locator: MatchLocator::Rest {
                    package_id,
                    versions,
                },
            });
        }

        Ok(results)
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
    package_rowid: i64,
    version: String,
    channel: String,
    id: String,
    name: String,
    moniker: Option<String>,
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

fn query_rows<T, F>(connection: &Connection, sql: &str, params: Vec<SqlValue>, mut map: F) -> Result<Vec<T>>
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
) -> Result<Vec<V2SearchRow>> {
    let (where_clause, params) = build_preindexed_where_clause(query, true, semantics);
    let sql = format!(
        "SELECT rowid, id, name, moniker, latest_version, hash \
         FROM packages WHERE {where_clause} LIMIT 50"
    );
    query_rows(
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
            })
        },
    )
}

fn query_v1_matches(
    connection: &Connection,
    query: &PackageQuery,
    semantics: SearchSemantics,
) -> Result<Vec<V1SearchRow>> {
    let (where_clause, params) = build_preindexed_where_clause(query, false, semantics);
    let sql = format!(
        "SELECT manifest.id, versions.version, channels.channel, ids.id, names.name, monikers.moniker \
         FROM manifest \
         JOIN ids ON manifest.id = ids.rowid \
         JOIN names ON manifest.name = names.rowid \
         LEFT JOIN monikers ON manifest.moniker = monikers.rowid \
         JOIN versions ON manifest.version = versions.rowid \
         JOIN channels ON manifest.channel = channels.rowid \
         WHERE {where_clause}"
    );
    query_rows(
        connection,
        &sql,
        params.into_iter().map(SqlValue::Text).collect(),
        |row| {
            Ok(V1SearchRow {
                package_rowid: row_i64(row, 0)?,
                version: row_string(row, 1)?,
                channel: row_string(row, 2)?,
                id: row_string(row, 3)?,
                name: row_string(row, 4)?,
                moniker: row_opt_string(row, 5)?,
            })
        },
    )
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
    let moniker_column = if v2 {
        "moniker"
    } else {
        "monikers.moniker"
    };
    let id_column = if v2 { "id" } else { "ids.id" };
    let name_column = if v2 { "name" } else { "names.name" };
    let exact_match = query.exact || semantics == SearchSemantics::Single;

    if let Some(value) = &query.id {
        return single_field_where(id_column, value, exact_match);
    }
    if let Some(value) = &query.name {
        return single_field_where(name_column, value, exact_match);
    }
    if let Some(value) = &query.moniker {
        return single_field_where(moniker_column, value, exact_match);
    }
    if let Some(value) = &query.query {
        if exact_match {
            let sql =
                format!("({id_column} LIKE ?1 OR {name_column} LIKE ?2 OR {moniker_column} LIKE ?3)");
            return (sql, vec![value.clone(), value.clone(), value.clone()]);
        }

        let like = format!("%{value}%");
        let sql =
            format!("({id_column} LIKE ?1 OR {name_column} LIKE ?2 OR {moniker_column} LIKE ?3)");
        return (sql, vec![like.clone(), like.clone(), like]);
    }

    ("1 = 1".to_string(), Vec::new())
}

fn single_field_where(column: &str, value: &str, exact: bool) -> (String, Vec<String>) {
    if exact {
        (format!("{column} LIKE ?1"), vec![value.to_string()])
    } else {
        (format!("{column} LIKE ?1"), vec![format!("%{value}%")])
    }
}

fn build_rest_search_body(
    query: &PackageQuery,
    info: &RestInformation,
    semantics: SearchSemantics,
) -> Result<JsonValue> {
    let mut root = serde_json::Map::new();
    root.insert("MaximumResults".to_string(), JsonValue::from(50));
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
        release_notes: yaml_localized_string(&merged, "ReleaseNotes"),
        release_notes_url: yaml_localized_string(&merged, "ReleaseNotesUrl"),
        tags: yaml_string_list(&merged, "Tags"),
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
        release_notes: json_string(default_locale, "ReleaseNotes"),
        release_notes_url: json_string(default_locale, "ReleaseNotesUrl"),
        tags: json_string_list(default_locale, "Tags"),
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
    root.get(YamlValue::from(key))
        .and_then(yaml_scalar_string)
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
}
