using System.Security.Cryptography;
using Microsoft.Data.Sqlite;
using YamlDotNet.Serialization;
using YamlDotNet.Serialization.NamingConventions;

namespace WinGetCore;

public class Repository : IDisposable
{
    private readonly HttpClient _client;
    private SourceStore _store;

    private Repository(HttpClient client, SourceStore store)
    {
        _client = client;
        _store = store;
    }

    public static Repository Open()
    {
        SourceStoreManager.EnsureAppDirs();
        var store = SourceStoreManager.Load();
        var client = new HttpClient();
        client.DefaultRequestHeaders.UserAgent.ParseAdd("winget-dotnet/0.1");
        return new Repository(client, store);
    }

    public void Dispose() => _client.Dispose();

    // ── Source management ──

    public List<SourceRecord> ListSources() => _store.Sources.ToList();

    public void AddSource(string name, string arg, SourceKind kind)
    {
        if (_store.Sources.Any(s => s.Name == name))
            throw new InvalidOperationException($"A source with name '{name}' already exists.");
        if (_store.Sources.Any(s => s.Arg == arg))
            throw new InvalidOperationException($"A source with argument '{arg}' already exists.");

        _store.Sources.Add(new SourceRecord
        {
            Name = name,
            Kind = kind,
            Arg = arg,
            Identifier = name,
        });
        SourceStoreManager.Save(_store);
    }

    public void RemoveSource(string name)
    {
        var source = _store.Sources.FirstOrDefault(s => s.Name == name)
            ?? throw new InvalidOperationException($"Source '{name}' not found.");

        _store.Sources.Remove(source);
        var stateDir = SourceStoreManager.SourceStateDir(source);
        if (Directory.Exists(stateDir))
            Directory.Delete(stateDir, recursive: true);
        SourceStoreManager.Save(_store);
    }

    public void ResetSources()
    {
        foreach (var source in _store.Sources)
        {
            var stateDir = SourceStoreManager.SourceStateDir(source);
            if (Directory.Exists(stateDir))
                Directory.Delete(stateDir, recursive: true);
        }
        _store = SourceStore.Default();
        SourceStoreManager.Save(_store);
    }

    public List<SourceUpdateResult> UpdateSources(string? sourceName = null)
    {
        var indexes = ResolveSourceIndexes(sourceName);
        var results = new List<SourceUpdateResult>();

        foreach (var index in indexes)
        {
            var source = _store.Sources[index];
            var detail = source.Kind switch
            {
                SourceKind.PreIndexed => PreIndexedSource.Update(_client, source),
                SourceKind.Rest => RestSource.UpdateRest(_client, source),
                _ => "Unknown source kind"
            };
            source.LastUpdate = DateTime.UtcNow;
            results.Add(new SourceUpdateResult { Name = source.Name, Kind = source.Kind, Detail = detail });
        }

        SourceStoreManager.Save(_store);
        return results;
    }

    // ── Search ──

    public SearchResponse Search(PackageQuery query)
    {
        var (matches, warnings, truncated) = SearchLocated(query, SearchSemantics.Many);
        return new SearchResponse
        {
            Matches = matches.Select(m => m.Display).ToList(),
            Warnings = warnings,
            Truncated = truncated
        };
    }

    // ── Show ──

    public ShowResult Show(PackageQuery query)
    {
        var (located, warnings) = FindSingleMatch(query);
        var (manifest, cachedFiles) = ManifestForMatch(located, query);
        var selectedInstaller = SelectInstaller(manifest.Installers, query);

        return new ShowResult
        {
            Package = located.Display,
            Manifest = manifest,
            SelectedInstaller = selectedInstaller,
            CachedFiles = cachedFiles,
            Warnings = warnings,
        };
    }

    public VersionsResult ShowVersions(PackageQuery query)
    {
        var (located, warnings) = FindSingleMatch(query);
        var versions = VersionsForMatch(located, query);
        return new VersionsResult { Package = located.Display, Versions = versions, Warnings = warnings };
    }

    public VersionsResult SearchVersions(PackageQuery query)
    {
        var (located, warnings) = FindSingleMatchWithSemantics(query, SearchSemantics.Many);
        var versions = VersionsForMatch(located, query);
        return new VersionsResult { Package = located.Display, Versions = versions, Warnings = warnings };
    }

    // ── Cache warm ──

    public CacheWarmResult WarmCache(PackageQuery query)
    {
        var (located, warnings) = FindSingleMatch(query);
        var (_, cachedFiles) = ManifestForMatch(located, query);
        return new CacheWarmResult { Package = located.Display, CachedFiles = cachedFiles, Warnings = warnings };
    }

    // ── List ──

    public ListResponse List(ListQuery query)
    {
        if ((query.IncludeUnknown || query.IncludePinned) && !query.UpgradeOnly)
            throw new InvalidOperationException("--include-unknown and --include-pinned require --upgrade-available");

        if (query.Source is not null && query.Query is null && query.Id is null &&
            query.Name is null && query.Moniker is null && query.Tag is null && query.Command is null)
            throw new InvalidOperationException("list --source currently requires a query or explicit filter");

        bool hasFilter = ListQueryNeedsAvailableLookup(query);
        bool needsAvailable = hasFilter || query.UpgradeOnly;
        var warnings = new List<string>();
        var installed = InstalledPackages.Collect(query.InstallScope);

        if (needsAvailable && hasFilter)
        {
            var availableQuery = PackageQueryFromListQuery(query);
            var (matches, srcWarnings, _) = SearchLocated(availableQuery, SearchSemantics.Many);
            warnings.AddRange(srcWarnings);
            var candidates = matches.Select(m => m.Display).ToList();
            foreach (var pkg in installed)
                pkg.Correlated = CorrelateInstalledPackage(pkg, candidates, AllowLooseListCorrelation(query));
        }
        else if (needsAvailable)
        {
            warnings.AddRange(CorrelateAllInstalled(installed));
        }

        var filtered = installed
            .Where(p => ListPackageMatches(p, query) &&
                (!query.UpgradeOnly || InstalledPackageMatchesUpgradeFilter(p, query)))
            .OrderBy(p => ListSortWeight(p))
            .ThenBy(p => p.Name, StringComparer.OrdinalIgnoreCase)
            .ThenBy(p => p.LocalId)
            .ToList();

        bool truncated = false;
        if (query.Count is int limit)
        {
            truncated = filtered.Count > limit;
            filtered = filtered.Take(limit).ToList();
        }

        return new ListResponse
        {
            Matches = filtered.Select(ListMatchFromInstalled).ToList(),
            Warnings = warnings,
            Truncated = truncated,
        };
    }

    // ── Pin management ──

    public List<PinRecord> ListPins() => PinStore.List();
    public void AddPin(string packageId, string version, string sourceId, PinType pinType)
        => PinStore.Add(packageId, version, sourceId, pinType);
    public bool RemovePin(string packageId) => PinStore.Remove(packageId);
    public void ResetPins() => PinStore.Reset();

    // ── Install / Uninstall ──

    public (Manifest Manifest, string InstallerPath) DownloadInstaller(PackageQuery query, string downloadDir)
    {
        var (located, _) = FindSingleMatch(query);
        var (manifest, _) = ManifestForMatch(located, query);
        var installer = SelectInstaller(manifest.Installers, query)
            ?? throw new InvalidOperationException("No applicable installer found for the current system");
        var url = installer.Url ?? throw new InvalidOperationException("Installer has no URL");

        Directory.CreateDirectory(downloadDir);
        var filename = url.Split('/').Last().Split('?').First();
        if (string.IsNullOrEmpty(filename)) filename = "installer";
        var dest = Path.Combine(downloadDir, filename);

        using var response = _client.GetAsync(url).GetAwaiter().GetResult();
        response.EnsureSuccessStatusCode();
        var bytes = response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult();
        File.WriteAllBytes(dest, bytes);

        // Verify hash
        if (installer.Sha256 is not null)
        {
            var actual = Sha256Hex(bytes);
            if (!actual.Equals(installer.Sha256, StringComparison.OrdinalIgnoreCase))
            {
                File.Delete(dest);
                throw new InvalidOperationException($"Installer hash mismatch. Expected: {installer.Sha256}, Got: {actual}");
            }
        }

        return (manifest, dest);
    }

    public InstallResult Install(PackageQuery query, bool silent)
    {
        var tempDir = Path.Combine(Path.GetTempPath(), "winget-dotnet-install");
        var (manifest, installerPath) = DownloadInstaller(query, tempDir);
        var installer = SelectInstaller(manifest.Installers, query)
            ?? throw new InvalidOperationException("No applicable installer found");

        var installerType = (installer.InstallerType ?? "exe").ToLowerInvariant();
        var exitCode = InstallerDispatch.Execute(installerPath, installerType, silent, installer);

        return new InstallResult
        {
            PackageId = manifest.Id,
            Version = manifest.Version,
            InstallerPath = installerPath,
            InstallerType = installerType,
            ExitCode = exitCode,
            Success = exitCode == 0,
        };
    }

    public InstallResult Uninstall(PackageQuery query, bool silent)
    {
        var listQuery = new ListQuery
        {
            Query = query.Query, Id = query.Id, Name = query.Name,
            Moniker = query.Moniker, Source = query.Source, Count = 10,
        };
        var listResult = List(listQuery);
        var installed = listResult.Matches.FirstOrDefault()
            ?? throw new InvalidOperationException("No installed package found matching the query");

        var exitCode = InstallerDispatch.Uninstall(installed.Id, silent);
        return new InstallResult
        {
            PackageId = installed.Id,
            Version = installed.InstalledVersion,
            InstallerPath = "",
            InstallerType = "uninstall",
            ExitCode = exitCode,
            Success = exitCode == 0,
        };
    }

    // ── Internal search machinery ──

    private (List<LocatedMatch> Matches, List<string> Warnings, bool Truncated) SearchLocated(
        PackageQuery query, SearchSemantics semantics)
    {
        var indexes = ResolveSourceIndexes(query.Source);
        var matches = new List<LocatedMatch>();
        var warnings = new List<string>();
        bool truncated = false;

        foreach (var index in indexes)
        {
            try
            {
                var (sourceMatches, srcTruncated) = SearchSource(index, query, semantics);
                truncated |= srcTruncated;
                matches.AddRange(sourceMatches);
            }
            catch
            {
                warnings.Add($"Failed when searching source; results will not be included: {_store.Sources[index].Name}");
            }
        }

        if (semantics == SearchSemantics.Many)
        {
            matches.Sort((a, b) => -SearchMatchSortScore(a.Display, query).CompareTo(SearchMatchSortScore(b.Display, query)));
            int limit = query.Count ?? 50;
            if (matches.Count > limit)
            {
                truncated = true;
                matches = matches.Take(limit).ToList();
            }
        }

        return (matches, warnings, truncated);
    }

    private (List<LocatedMatch> Matches, bool Truncated) SearchSource(
        int sourceIndex, PackageQuery query, SearchSemantics semantics)
    {
        var source = _store.Sources[sourceIndex];
        return source.Kind switch
        {
            SourceKind.PreIndexed => SearchPreindexed(sourceIndex, query, semantics),
            SourceKind.Rest => SearchRest(sourceIndex, query, semantics),
            _ => ([], false)
        };
    }

    private (List<LocatedMatch> Matches, bool Truncated) SearchPreindexed(
        int sourceIndex, PackageQuery query, SearchSemantics semantics)
    {
        var conn = OpenPreindexedConnection(sourceIndex);
        var source = _store.Sources[sourceIndex];

        // Try V2 first, fall back to V1
        try
        {
            var (rows, truncated) = PreIndexedSource.SearchV2(conn, query, semantics);
            return (rows.Select(r => new LocatedMatch
            {
                Display = new SearchMatch
                {
                    SourceName = source.Name, SourceKind = source.Kind,
                    Id = r.Id, Name = r.Name, Moniker = r.Moniker,
                    Version = r.Version, MatchCriteria = r.MatchCriteria,
                },
                SourceIndex = sourceIndex,
                Locator = new PreIndexedV2Locator(r.PackageRowId, r.PackageHash),
            }).ToList(), truncated);
        }
        catch
        {
            // V2 failed, try V1
            var (rows, truncated) = PreIndexedSource.SearchV1(conn, query, semantics);
            return (rows.Select(r => new LocatedMatch
            {
                Display = new SearchMatch
                {
                    SourceName = source.Name, SourceKind = source.Kind,
                    Id = r.Id, Name = r.Name, Moniker = r.Moniker,
                    Version = r.Version, Channel = string.IsNullOrEmpty(r.Channel) ? null : r.Channel,
                    MatchCriteria = r.MatchCriteria,
                },
                SourceIndex = sourceIndex,
                Locator = new PreIndexedV1Locator(r.PackageRowId),
            }).ToList(), truncated);
        }
    }

    private (List<LocatedMatch> Matches, bool Truncated) SearchRest(
        int sourceIndex, PackageQuery query, SearchSemantics semantics)
    {
        var source = _store.Sources[sourceIndex];
        var info = RestSource.LoadInformation(_client, source);

        var (results, truncated) = RestSource.Search(_client, source, query, info, semantics);
        return (results.Select(r => new LocatedMatch
        {
            Display = new SearchMatch
            {
                SourceName = source.Name, SourceKind = source.Kind,
                Id = r.PackageId, Name = r.PackageName, Moniker = r.Moniker,
                Version = r.LatestVersion.Version,
                Channel = string.IsNullOrEmpty(r.LatestVersion.Channel) ? null : r.LatestVersion.Channel,
                MatchCriteria = r.MatchCriteria,
            },
            SourceIndex = sourceIndex,
            Locator = new RestLocator(r.PackageId, r.Versions),
        }).ToList(), truncated);
    }

    private SqliteConnection OpenPreindexedConnection(int sourceIndex)
    {
        var source = _store.Sources[sourceIndex];
        var indexPath = PreIndexedSource.IndexPath(source);
        if (!File.Exists(indexPath))
        {
            PreIndexedSource.Update(_client, source);
            source.LastUpdate = DateTime.UtcNow;
            SourceStoreManager.Save(_store);
        }

        var conn = new SqliteConnection($"Data Source={indexPath};Mode=ReadOnly");
        conn.Open();
        return conn;
    }

    private (LocatedMatch Match, List<string> Warnings) FindSingleMatch(PackageQuery query)
        => FindSingleMatchWithSemantics(query, SearchSemantics.Single);

    private (LocatedMatch Match, List<string> Warnings) FindSingleMatchWithSemantics(
        PackageQuery query, SearchSemantics semantics)
    {
        var (matches, warnings, _) = SearchLocated(query, semantics);

        if (matches.Count == 0)
            throw new InvalidOperationException("no package matched the supplied query");

        if (matches.Count > 1)
        {
            var choices = string.Join(", ", matches.Take(10)
                .Select(m => $"{m.Display.Name} [{m.Display.Id}] ({m.Display.SourceName})"));
            throw new InvalidOperationException($"multiple packages matched: {choices}");
        }

        return (matches[0], warnings);
    }

    private List<VersionKey> VersionsForMatch(LocatedMatch located, PackageQuery query)
    {
        var versions = located.Locator switch
        {
            PreIndexedV1Locator v1 => VersionsFromV1(located.SourceIndex, v1.PackageRowId),
            PreIndexedV2Locator v2 => VersionsFromV2(located.SourceIndex, v2.PackageRowId, v2.PackageHash),
            RestLocator rest => rest.Versions.ToList(),
            _ => throw new InvalidOperationException("Unknown locator type")
        };

        RestSource.SortVersionsDesc(versions);
        return versions;
    }

    private List<VersionKey> VersionsFromV1(int sourceIndex, long packageRowid)
    {
        var conn = OpenPreindexedConnection(sourceIndex);
        var rows = PreIndexedSource.QueryV1Versions(conn, packageRowid);
        return rows.Select(r => new VersionKey { Version = r.Version, Channel = r.Channel }).ToList();
    }

    private List<VersionKey> VersionsFromV2(int sourceIndex, long packageRowid, string packageHash)
    {
        var source = _store.Sources[sourceIndex];
        var conn = OpenPreindexedConnection(sourceIndex);
        var (entries, _) = PreIndexedSource.LoadV2VersionData(_client, conn, source, packageRowid, packageHash);
        return entries.Select(e => new VersionKey { Version = e.Version, Channel = "" }).ToList();
    }

    private (Manifest Manifest, List<string> CachedFiles) ManifestForMatch(LocatedMatch located, PackageQuery query)
    {
        return located.Locator switch
        {
            PreIndexedV1Locator v1 => ManifestFromV1(located.SourceIndex, v1.PackageRowId, query),
            PreIndexedV2Locator v2 => ManifestFromV2(located.SourceIndex, v2.PackageRowId, v2.PackageHash, query),
            RestLocator rest => ManifestFromRest(located.SourceIndex, rest.PackageId, rest.Versions, query),
            _ => throw new InvalidOperationException("Unknown locator type")
        };
    }

    private (Manifest, List<string>) ManifestFromV1(int sourceIndex, long packageRowid, PackageQuery query)
    {
        var conn = OpenPreindexedConnection(sourceIndex);
        var source = _store.Sources[sourceIndex];
        var versions = PreIndexedSource.QueryV1Versions(conn, packageRowid);
        var selected = SelectV1Version(versions, query.Version, query.Channel);
        var relativePath = PreIndexedSource.ResolveV1RelativePath(conn, selected.PathPart);
        var bytes = PreIndexedSource.GetCachedSourceFile(_client, "V1_M", source, relativePath, selected.ManifestHash);
        var manifest = ParseYamlManifest(bytes);
        manifest = manifest with { Version = selected.Version, Channel = selected.Channel };
        return (manifest, []);
    }

    private (Manifest, List<string>) ManifestFromV2(int sourceIndex, long packageRowid, string packageHash, PackageQuery query)
    {
        var source = _store.Sources[sourceIndex];
        var conn = OpenPreindexedConnection(sourceIndex);
        var (entries, vdFile) = PreIndexedSource.LoadV2VersionData(_client, conn, source, packageRowid, packageHash);
        var selected = SelectV2Version(entries, query.Version);
        var bytes = PreIndexedSource.GetCachedSourceFile(_client, "V2_M", source, selected.ManifestRelativePath, selected.ManifestHash);
        var manifest = ParseYamlManifest(bytes);
        manifest = manifest with { Version = selected.Version };
        return (manifest, [vdFile]);
    }

    private (Manifest, List<string>) ManifestFromRest(int sourceIndex, string packageId, List<VersionKey> versions, PackageQuery query)
    {
        var source = _store.Sources[sourceIndex];
        var info = RestSource.LoadInformation(_client, source);
        var selected = SelectRestVersion(versions, query.Version, query.Channel);
        var manifest = RestSource.FetchManifest(_client, source, info, packageId, selected.Version, selected.Channel);
        return (manifest, []);
    }

    // ── Version selection ──

    private static PreIndexedSource.V1VersionRow SelectV1Version(
        List<PreIndexedSource.V1VersionRow> versions, string? requestedVersion, string? requestedChannel)
    {
        if (versions.Count == 0)
            throw new InvalidOperationException("No versions found");

        if (requestedVersion is not null)
        {
            var match = versions.FirstOrDefault(v => v.Version == requestedVersion
                && (requestedChannel is null || v.Channel == requestedChannel));
            if (match is not null) return match;
        }

        return versions[0]; // latest
    }

    private static PreIndexedSource.V2VersionDataEntry SelectV2Version(
        List<PreIndexedSource.V2VersionDataEntry> entries, string? requestedVersion)
    {
        if (entries.Count == 0)
            throw new InvalidOperationException("No versions found");

        if (requestedVersion is not null)
        {
            var match = entries.FirstOrDefault(e => e.Version == requestedVersion);
            if (match is not null) return match;
        }

        // Sort and return latest
        var sorted = entries.OrderByDescending(e => e.Version, new VersionComparer()).ToList();
        return sorted[0];
    }

    private static VersionKey SelectRestVersion(List<VersionKey> versions, string? requestedVersion, string? requestedChannel)
    {
        if (versions.Count == 0)
            throw new InvalidOperationException("No versions found");

        if (requestedVersion is not null)
        {
            var match = versions.FirstOrDefault(v => v.Version == requestedVersion
                && (requestedChannel is null || v.Channel == requestedChannel));
            if (match is not null) return match;
        }

        var sorted = versions.OrderByDescending(v => v.Version, new VersionComparer()).ToList();
        return sorted[0];
    }

    // ── Installer selection ──

    internal static Installer? SelectInstaller(List<Installer> installers, PackageQuery query)
    {
        if (installers.Count == 0) return null;

        var candidates = installers.AsEnumerable();

        if (query.InstallerArchitecture is not null)
            candidates = candidates.Where(i =>
                i.Architecture?.Equals(query.InstallerArchitecture, StringComparison.OrdinalIgnoreCase) == true);

        if (query.InstallerType is not null)
            candidates = candidates.Where(i =>
                i.InstallerType?.Equals(query.InstallerType, StringComparison.OrdinalIgnoreCase) == true);

        if (query.Locale is not null)
            candidates = candidates.Where(i =>
                i.Locale is null || i.Locale.Equals(query.Locale, StringComparison.OrdinalIgnoreCase));

        if (query.InstallScope is not null)
            candidates = candidates.Where(i =>
                i.Scope is null || i.Scope.Equals(query.InstallScope, StringComparison.OrdinalIgnoreCase));

        var list = candidates.ToList();
        if (list.Count == 0) return null;

        // Prefer x64 > x86 > neutral
        return list.OrderByDescending(i => i.Architecture?.ToLowerInvariant() switch
        {
            "x64" => 3,
            "x86" => 2,
            "arm64" => 1,
            _ => 0
        }).First();
    }

    // ── Manifest parsing ──

    private static Manifest ParseYamlManifest(byte[] bytes)
    {
        var yaml = System.Text.Encoding.UTF8.GetString(bytes);
        var deserializer = new DeserializerBuilder()
            .WithNamingConvention(PascalCaseNamingConvention.Instance)
            .IgnoreUnmatchedProperties()
            .Build();

        var dict = deserializer.Deserialize<Dictionary<string, object?>>(yaml) ?? [];

        string GetStr(string key) => dict.TryGetValue(key, out var v) ? v?.ToString() ?? "" : "";
        string? GetOptStr(string key) => dict.TryGetValue(key, out var v) && v is not null ? v.ToString() : null;

        var tags = new List<string>();
        if (dict.TryGetValue("Tags", out var tagsObj) && tagsObj is IList<object> tagList)
            tags = tagList.Select(t => t?.ToString() ?? "").Where(t => t != "").ToList();

        var docs = new List<Documentation>();
        if (dict.TryGetValue("Documentations", out var docsObj) && docsObj is IList<object> docsList)
        {
            foreach (var d in docsList)
            {
                if (d is IDictionary<object, object> docDict)
                {
                    var url = docDict.TryGetValue("DocumentUrl", out var u) ? u?.ToString() : null;
                    if (url is not null)
                        docs.Add(new Documentation
                        {
                            Label = docDict.TryGetValue("DocumentLabel", out var l) ? l?.ToString() : null,
                            Url = url
                        });
                }
            }
        }

        var installers = new List<Installer>();
        if (dict.TryGetValue("Installers", out var instObj) && instObj is IList<object> instList)
        {
            foreach (var inst in instList)
            {
                if (inst is IDictionary<object, object> instDict)
                {
                    string? InstStr(string key) => instDict.TryGetValue(key, out var v) ? v?.ToString() : null;
                    List<string> InstArr(string key)
                    {
                        if (!instDict.TryGetValue(key, out var v) || v is not IList<object> arr) return [];
                        return arr.Select(x => x?.ToString() ?? "").Where(s => s != "").ToList();
                    }

                    installers.Add(new Installer
                    {
                        Architecture = InstStr("Architecture"),
                        InstallerType = InstStr("InstallerType"),
                        Url = InstStr("InstallerUrl"),
                        Sha256 = InstStr("InstallerSha256"),
                        ProductCode = InstStr("ProductCode"),
                        Locale = InstStr("InstallerLocale"),
                        Scope = InstStr("Scope"),
                        ReleaseDate = InstStr("ReleaseDate"),
                        PackageFamilyName = InstStr("PackageFamilyName"),
                        UpgradeCode = InstStr("UpgradeCode"),
                        Commands = InstArr("Commands"),
                        PackageDependencies = InstArr("PackageDependencies"),
                    });
                }
            }
        }

        return new Manifest
        {
            Id = GetStr("PackageIdentifier"),
            Name = GetOptStr("PackageName") ?? GetStr("PackageIdentifier"),
            Version = GetStr("PackageVersion"),
            Publisher = GetOptStr("Publisher"),
            Description = GetOptStr("Description") ?? GetOptStr("ShortDescription"),
            Moniker = GetOptStr("Moniker"),
            PackageUrl = GetOptStr("PackageUrl"),
            PublisherUrl = GetOptStr("PublisherUrl"),
            PublisherSupportUrl = GetOptStr("PublisherSupportUrl"),
            License = GetOptStr("License"),
            LicenseUrl = GetOptStr("LicenseUrl"),
            PrivacyUrl = GetOptStr("PrivacyUrl"),
            Author = GetOptStr("Author"),
            Copyright = GetOptStr("Copyright"),
            CopyrightUrl = GetOptStr("CopyrightUrl"),
            ReleaseNotes = GetOptStr("ReleaseNotes"),
            ReleaseNotesUrl = GetOptStr("ReleaseNotesUrl"),
            Tags = tags,
            Documentation = docs,
            Installers = installers,
        };
    }

    // ── List helpers ──

    private List<string> CorrelateAllInstalled(List<InstalledPackage> installed)
    {
        var allQuery = new PackageQuery { Count = 100_000 };
        var (matches, warnings, _) = SearchLocated(allQuery, SearchSemantics.Many);
        var candidates = matches.Select(m => m.Display).ToList();
        foreach (var pkg in installed)
            pkg.Correlated = CorrelateInstalledPackage(pkg, candidates, true);
        return warnings;
    }

    private static SearchMatch? CorrelateInstalledPackage(InstalledPackage pkg, List<SearchMatch> candidates, bool loose)
    {
        // Try exact id match first
        var exact = candidates.FirstOrDefault(c =>
            c.Id.Equals(pkg.LocalId, StringComparison.OrdinalIgnoreCase));
        if (exact is not null) return exact;

        // Try name match
        var nameMatch = candidates.FirstOrDefault(c =>
            c.Name.Equals(pkg.Name, StringComparison.OrdinalIgnoreCase));
        if (nameMatch is not null) return nameMatch;

        if (!loose) return null;

        // Try substring
        return candidates.FirstOrDefault(c =>
            c.Id.Contains(pkg.LocalId, StringComparison.OrdinalIgnoreCase) ||
            c.Name.Contains(pkg.Name, StringComparison.OrdinalIgnoreCase));
    }

    private static bool ListPackageMatches(InstalledPackage pkg, ListQuery query)
    {
        if (query.Query is null && query.Id is null && query.Name is null &&
            query.Moniker is null && query.Tag is null && query.Command is null)
            return true;

        if (query.Query is not null)
        {
            var q = query.Query;
            if (pkg.Name.Contains(q, StringComparison.OrdinalIgnoreCase) ||
                pkg.LocalId.Contains(q, StringComparison.OrdinalIgnoreCase) ||
                (pkg.Correlated?.Id.Contains(q, StringComparison.OrdinalIgnoreCase) == true) ||
                (pkg.Correlated?.Name.Contains(q, StringComparison.OrdinalIgnoreCase) == true))
                return true;
        }

        if (query.Id is not null && (pkg.LocalId.Contains(query.Id, StringComparison.OrdinalIgnoreCase) ||
            pkg.Correlated?.Id.Contains(query.Id, StringComparison.OrdinalIgnoreCase) == true))
            return true;

        if (query.Name is not null && pkg.Name.Contains(query.Name, StringComparison.OrdinalIgnoreCase))
            return true;

        return false;
    }

    private static bool InstalledPackageMatchesUpgradeFilter(InstalledPackage pkg, ListQuery query)
    {
        if (pkg.Correlated is null) return query.IncludeUnknown;
        var availableVersion = pkg.Correlated.Version;
        if (availableVersion is null) return false;
        return RestSource.CompareVersionStrings(availableVersion, pkg.InstalledVersion) > 0;
    }

    private static int ListSortWeight(InstalledPackage pkg) => pkg.Correlated is not null ? 0 : 1;

    private static ListMatch ListMatchFromInstalled(InstalledPackage pkg) => new()
    {
        Name = pkg.Correlated?.Name ?? pkg.Name,
        Id = pkg.Correlated?.Id ?? pkg.LocalId,
        LocalId = pkg.LocalId,
        InstalledVersion = pkg.InstalledVersion,
        AvailableVersion = pkg.Correlated?.Version,
        SourceName = pkg.Correlated?.SourceName,
        Publisher = pkg.Publisher,
        Scope = pkg.Scope,
        InstallerCategory = pkg.InstallerCategory,
        InstallLocation = pkg.InstallLocation,
        PackageFamilyNames = pkg.PackageFamilyNames,
        ProductCodes = pkg.ProductCodes,
        UpgradeCodes = pkg.UpgradeCodes,
    };

    private static bool ListQueryNeedsAvailableLookup(ListQuery query)
        => query.Query is not null || query.Id is not null || query.Name is not null ||
           query.Moniker is not null || query.Tag is not null || query.Command is not null;

    private static bool AllowLooseListCorrelation(ListQuery query) => !query.Exact;

    private static PackageQuery PackageQueryFromListQuery(ListQuery query) => new()
    {
        Query = query.Query, Id = query.Id, Name = query.Name, Moniker = query.Moniker,
        Tag = query.Tag, Command = query.Command, Source = query.Source,
        Count = 500, Exact = query.Exact,
    };

    private List<int> ResolveSourceIndexes(string? sourceName)
    {
        if (sourceName is null)
            return Enumerable.Range(0, _store.Sources.Count).ToList();

        for (int i = 0; i < _store.Sources.Count; i++)
        {
            if (_store.Sources[i].Name.Equals(sourceName, StringComparison.OrdinalIgnoreCase))
                return [i];
        }
        throw new InvalidOperationException($"Source '{sourceName}' not found.");
    }

    // ── Scoring ──

    private static int SearchMatchSortScore(SearchMatch match, PackageQuery query)
    {
        if (query.Query is null && query.Id is null && query.Name is null) return 0;

        var q = query.Query ?? query.Id ?? query.Name ?? "";
        if (match.Id.Equals(q, StringComparison.OrdinalIgnoreCase)) return 100;
        if (match.Name.Equals(q, StringComparison.OrdinalIgnoreCase)) return 90;
        if (match.Id.StartsWith(q, StringComparison.OrdinalIgnoreCase)) return 80;
        if (match.Name.StartsWith(q, StringComparison.OrdinalIgnoreCase)) return 70;
        if (match.Id.Contains(q, StringComparison.OrdinalIgnoreCase)) return 60;
        if (match.Name.Contains(q, StringComparison.OrdinalIgnoreCase)) return 50;
        return 10; // tag/command/moniker match
    }

    // ── Utilities ──

    private static string Sha256Hex(byte[] data)
    {
        var hash = SHA256.HashData(data);
        return Convert.ToHexStringLower(hash);
    }

    private class VersionComparer : IComparer<string>
    {
        public int Compare(string? x, string? y) => RestSource.CompareVersionStrings(x ?? "", y ?? "");
    }
}
