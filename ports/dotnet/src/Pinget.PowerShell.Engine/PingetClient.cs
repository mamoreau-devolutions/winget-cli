using Pinget.Core;
using Pinget.PowerShell.Engine.PSObjects;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Pinget.PowerShell.Engine;

public sealed class PingetClient : IDisposable
{
    private static readonly string[] SupportedAdminSettings =
    [
        "LocalManifestFiles",
        "BypassCertificatePinningForMicrosoftStore",
        "InstallerHashOverride",
        "LocalArchiveMalwareScanOverride",
        "ProxyCommandLineOptions",
    ];

    private readonly Repository _repository;

    private PingetClient(Repository repository)
    {
        _repository = repository;
    }

    public static PingetClient Open(RepositoryOptions? options = null) => new(Repository.Open(options));

    public void Dispose() => _repository.Dispose();

    public string GetVersion() => PowerShellEngineVersion.Current;

    public CollectionResult<PSSourceResult> GetSources(string? name = null)
    {
        var sources = _repository.ListSources();
        if (!string.IsNullOrWhiteSpace(name))
        {
            sources = sources
                .Where(source => string.Equals(source.Name, name, StringComparison.OrdinalIgnoreCase))
                .ToList();

            if (sources.Count == 0)
                throw new InvalidOperationException($"Source '{name}' not found.");
        }

        return new CollectionResult<PSSourceResult>(
            sources.Select(source => new PSSourceResult
            {
                Name = source.Name,
                Argument = source.Arg,
                Type = source.Kind switch
                {
                    SourceKind.Rest => "Microsoft.Rest",
                    SourceKind.PreIndexed => "Microsoft.PreIndexed.Package",
                    _ => source.Kind.ToString(),
                },
                TrustLevel = string.IsNullOrWhiteSpace(source.TrustLevel) ? "None" : source.TrustLevel,
                Explicit = source.Explicit,
                Priority = source.Priority,
                Identifier = source.Identifier,
                LastUpdate = source.LastUpdate,
                SourceVersion = source.SourceVersion,
            }).ToList(),
            []);
    }

    public void AddSource(string name, string argument, string? type, PSSourceTrustLevel trustLevel, bool explicitSource, int priority)
    {
        _repository.AddSource(
            name,
            argument,
            ParseSourceKind(type),
            trustLevel == PSSourceTrustLevel.Default ? "None" : trustLevel.ToString(),
            explicitSource,
            priority);
    }

    public void RemoveSource(string name) => _repository.RemoveSource(name);

    public void ResetSource(string? name, bool all)
    {
        if (!string.IsNullOrWhiteSpace(name))
        {
            _repository.ResetSource(name);
            return;
        }

        if (!all)
            throw new InvalidOperationException("Resetting all sources requires -All.");

        _repository.ResetSources();
    }

    public CollectionResult<PSFoundCatalogPackage> FindPackages(
        string? id,
        string? name,
        string? moniker,
        string? source,
        string[]? query,
        string? tag,
        string? command,
        uint count,
        PSPackageFieldMatchOption matchOption)
    {
        var response = _repository.Search(BuildPackageQuery(id, name, moniker, source, query, tag, command, count, matchOption));
        return new CollectionResult<PSFoundCatalogPackage>(
            response.Matches.Select(match => new PSFoundCatalogPackage(
                match.Id,
                match.Name,
                match.SourceName,
                match.Moniker,
                match.Version,
                match.MatchCriteria)).ToList(),
            response.Warnings,
            response.Truncated);
    }

    public CollectionResult<PSInstalledCatalogPackage> GetInstalledPackages(
        string? id,
        string? name,
        string? moniker,
        string? source,
        string[]? query,
        string? tag,
        string? command,
        uint count,
        PSPackageFieldMatchOption matchOption)
    {
        var response = _repository.List(BuildListQuery(id, name, moniker, source, query, tag, command, count, matchOption));
        return new CollectionResult<PSInstalledCatalogPackage>(
            response.Matches.Select(match => new PSInstalledCatalogPackage(
                match.Id,
                match.Name,
                match.SourceName ?? string.Empty,
                null,
                match.InstalledVersion,
                string.IsNullOrWhiteSpace(match.AvailableVersion) ? [] : [match.AvailableVersion],
                match.Publisher,
                match.Scope)).ToList(),
            response.Warnings,
            response.Truncated);
    }

    public CommandResult<PSDownloadResult> DownloadPackage(
        PSCatalogPackage? inputObject,
        string? version,
        string? id,
        string? name,
        string? moniker,
        string? source,
        string[]? query,
        bool allowHashMismatch,
        bool skipDependencies,
        string? locale,
        PSPackageInstallScope scope,
        PSProcessorArchitecture architecture,
        PSPackageInstallerType installerType,
        string? downloadDirectory,
        bool skipMicrosoftStoreLicense,
        PSWindowsPlatform platform,
        string? targetOsVersion)
    {
        var warnings = new List<string>();
        AddIgnoredDownloadWarnings(warnings, allowHashMismatch, skipDependencies, skipMicrosoftStoreLicense, platform, targetOsVersion);

        var request = new InstallRequest
        {
            Query = BuildPackageQuery(
                id ?? inputObject?.Id,
                name ?? inputObject?.Name,
                moniker ?? inputObject?.Moniker,
                source ?? inputObject?.Source,
                query,
                null,
                null,
                0,
                PSPackageFieldMatchOption.EqualsCaseInsensitive) with
            {
                Version = version ?? GetCatalogVersion(inputObject),
                Locale = locale,
                InstallerArchitecture = architecture == PSProcessorArchitecture.Default ? null : architecture.ToString(),
                InstallerType = installerType == PSPackageInstallerType.Default ? null : installerType.ToString(),
                InstallScope = scope == PSPackageInstallScope.Any ? null : scope.ToString(),
            },
            SkipDependencies = skipDependencies,
        };

        var outputDirectory = string.IsNullOrWhiteSpace(downloadDirectory)
            ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Downloads")
            : downloadDirectory;

        var (manifest, installerPath) = _repository.DownloadInstaller(request, outputDirectory);
        return new CommandResult<PSDownloadResult>(
            new PSDownloadResult
            {
                Id = manifest.Id,
                Name = manifest.Name,
                Source = request.Query.Source ?? inputObject?.Source ?? string.Empty,
                CorrelationData = installerPath,
                Status = "Ok",
                Version = manifest.Version,
                DownloadDirectory = outputDirectory,
                DownloadedInstallerPath = installerPath,
            },
            warnings);
    }

    public JsonObject GetUserSettings() => LoadJsonObject(GetUserSettingsPath());

    public JsonObject SetUserSettings(JsonObject userSettings, bool merge)
    {
        var effective = merge ? MergeJsonObjects(GetUserSettings(), userSettings) : userSettings.DeepClone().AsObject();
        SaveJsonObject(GetUserSettingsPath(), effective);
        return effective;
    }

    public bool TestUserSettings(JsonObject expected, bool ignoreNotSet)
    {
        var current = GetUserSettings();
        return ignoreNotSet ? JsonContains(current, expected) : JsonNode.DeepEquals(current, expected);
    }

    public JsonObject GetAdminSettings()
    {
        var settings = LoadJsonObject(GetAdminSettingsPath());
        foreach (var name in SupportedAdminSettings)
        {
            settings.TryAdd(name, false);
        }

        return settings;
    }

    public void SetAdminSetting(string name, bool enabled)
    {
        if (!SupportedAdminSettings.Contains(name, StringComparer.Ordinal))
            throw new InvalidOperationException($"Unsupported admin setting: {name}");

        var settings = GetAdminSettings();
        settings[name] = enabled;
        SaveJsonObject(GetAdminSettingsPath(), settings);
    }

    public void AssertPackageManager(string? version, bool latest, bool includePrerelease)
    {
        var issues = new List<string>();

        if (!string.IsNullOrWhiteSpace(version) &&
            !string.Equals(version, GetVersion(), StringComparison.OrdinalIgnoreCase))
        {
            issues.Add($"Requested version '{version}' does not match Pinget version '{GetVersion()}'.");
        }

        if (latest && !includePrerelease)
        {
            // Pinget is self-hosted in this workspace, so the current build is the only version we can assert.
        }

        try
        {
            _ = _repository.ListSources();
            _ = GetAdminSettings();
            _ = GetUserSettings();
        }
        catch (Exception ex)
        {
            issues.Add(ex.Message);
        }

        if (issues.Count != 0)
            throw new InvalidOperationException(string.Join(Environment.NewLine, issues));
    }

    public CommandResult<int> RepairPackageManager(string? version, bool latest, bool includePrerelease, bool allUsers, bool force)
    {
        var warnings = new List<string>();
        if (!string.IsNullOrWhiteSpace(version) &&
            !string.Equals(version, GetVersion(), StringComparison.OrdinalIgnoreCase))
        {
            warnings.Add($"Requested version '{version}' is not available; repaired current Pinget version '{GetVersion()}' instead.");
        }
        if (latest)
            warnings.Add("Latest is accepted for compatibility but Pinget repairs the current local build.");
        if (includePrerelease)
            warnings.Add("IncludePrerelease is accepted for compatibility but Pinget repairs the current local build.");
        if (allUsers)
            warnings.Add("AllUsers is accepted for compatibility but Pinget repairs the current user's app root.");
        if (force)
            warnings.Add("Force is accepted for compatibility but Pinget repair does not require process shutdown today.");

        Directory.CreateDirectory(_repository.AppRoot);
        Directory.CreateDirectory(Path.Combine(_repository.AppRoot, "sources"));
        var sourcesPath = Path.Combine(_repository.AppRoot, "sources.json");
        if (!File.Exists(sourcesPath))
            _repository.ResetSources();

        if (!File.Exists(GetUserSettingsPath()))
            SaveJsonObject(GetUserSettingsPath(), new JsonObject());

        if (!File.Exists(GetAdminSettingsPath()))
            SaveJsonObject(GetAdminSettingsPath(), GetAdminSettings());

        AssertPackageManager(null, false, false);
        return new CommandResult<int>(0, warnings);
    }

    public CommandResult<PSInstallResult> InstallPackage(
        PSCatalogPackage? inputObject,
        string? version,
        string? id,
        string? name,
        string? moniker,
        string? source,
        string[]? query,
        bool allowHashMismatch,
        string? overrideArguments,
        string? custom,
        string? location,
        string? log,
        bool force,
        string? header,
        bool skipDependencies,
        string? locale,
        PSPackageInstallScope scope,
        PSProcessorArchitecture architecture,
        PSPackageInstallMode mode,
        PSPackageInstallerType installerType)
    {
        var warnings = new List<string>();
        AddUnsupportedActionWarnings(warnings, allowHashMismatch, header);

        var request = new InstallRequest
        {
            Query = BuildPackageQuery(
                id ?? inputObject?.Id,
                name ?? inputObject?.Name,
                moniker ?? inputObject?.Moniker,
                source ?? inputObject?.Source,
                query,
                null,
                null,
                0,
                PSPackageFieldMatchOption.EqualsCaseInsensitive) with
            {
                Version = version ?? GetCatalogVersion(inputObject),
                Locale = locale,
                InstallerArchitecture = architecture == PSProcessorArchitecture.Default ? null : architecture.ToString(),
                InstallerType = installerType == PSPackageInstallerType.Default ? null : installerType.ToString(),
                InstallScope = scope == PSPackageInstallScope.Any ? null : scope.ToString(),
            },
            Override = Normalize(overrideArguments),
            Custom = Normalize(custom),
            InstallLocation = Normalize(location),
            LogPath = Normalize(log),
            Force = force,
            SkipDependencies = skipDependencies,
            Mode = ToInstallerMode(mode),
        };

        var result = _repository.Install(request);
        return new CommandResult<PSInstallResult>(ToPsInstallResult(result, request, inputObject), warnings.Concat(result.Warnings).ToList());
    }

    public CommandResult<PSUninstallResult> UninstallPackage(
        PSCatalogPackage? inputObject,
        string? version,
        string? id,
        string? name,
        string? moniker,
        string? source,
        string[]? query,
        bool force,
        string? log,
        PSPackageUninstallMode mode)
    {
        var request = new UninstallRequest
        {
            Query = BuildPackageQuery(
                id ?? inputObject?.Id,
                name ?? inputObject?.Name,
                moniker ?? inputObject?.Moniker,
                source ?? inputObject?.Source,
                query,
                null,
                null,
                0,
                PSPackageFieldMatchOption.EqualsCaseInsensitive) with
            {
                Version = version ?? GetCatalogVersion(inputObject),
            },
            Force = force,
            LogPath = Normalize(log),
            Mode = ToInstallerMode(mode),
        };

        var result = _repository.Uninstall(request);
        return new CommandResult<PSUninstallResult>(ToPsUninstallResult(result, request, inputObject), result.Warnings);
    }

    public CollectionResult<PSInstallResult> UpdatePackages(
        PSCatalogPackage? inputObject,
        string? version,
        string? id,
        string? name,
        string? moniker,
        string? source,
        string[]? query,
        bool allowHashMismatch,
        string? overrideArguments,
        string? custom,
        string? location,
        string? log,
        bool force,
        string? header,
        bool skipDependencies,
        string? locale,
        PSPackageInstallScope scope,
        PSProcessorArchitecture architecture,
        PSPackageInstallMode mode,
        PSPackageInstallerType installerType,
        bool includeUnknown)
    {
        var warnings = new List<string>();
        AddUnsupportedActionWarnings(warnings, allowHashMismatch, header);

        var listQuery = new ListQuery
        {
            Id = Normalize(id ?? inputObject?.Id),
            Name = Normalize(name ?? inputObject?.Name),
            Moniker = Normalize(moniker ?? inputObject?.Moniker),
            Source = Normalize(source ?? inputObject?.Source),
            Query = JoinQuery(query),
            Version = Normalize(version),
            UpgradeOnly = true,
            IncludeUnknown = includeUnknown,
            Exact = true,
        };

        var matches = _repository.List(listQuery);
        warnings.AddRange(matches.Warnings);

        var results = new List<PSInstallResult>();
        foreach (var match in matches.Matches)
        {
            var request = new InstallRequest
            {
                Query = new PackageQuery
                {
                    Id = match.Id,
                    Source = match.SourceName,
                    Exact = true,
                    Version = Normalize(version) ?? match.AvailableVersion,
                    Locale = locale,
                    InstallerArchitecture = architecture == PSProcessorArchitecture.Default ? null : architecture.ToString(),
                    InstallerType = installerType == PSPackageInstallerType.Default ? null : installerType.ToString(),
                    InstallScope = scope == PSPackageInstallScope.Any ? null : scope.ToString(),
                },
                Override = Normalize(overrideArguments),
                Custom = Normalize(custom),
                InstallLocation = Normalize(location),
                LogPath = Normalize(log),
                Force = force,
                SkipDependencies = skipDependencies,
                Mode = ToInstallerMode(mode),
            };

            var result = _repository.Install(request);
            warnings.AddRange(result.Warnings);
            results.Add(ToPsInstallResult(result, request, new PSInstalledCatalogPackage(
                match.Id,
                match.Name,
                match.SourceName ?? string.Empty,
                null,
                match.InstalledVersion,
                string.IsNullOrWhiteSpace(match.AvailableVersion) ? [] : [match.AvailableVersion],
                match.Publisher,
                match.Scope)));
        }

        return new CollectionResult<PSInstallResult>(results, warnings, matches.Truncated);
    }

    public CommandResult<PSRepairResult> RepairPackage(
        PSCatalogPackage? inputObject,
        string? version,
        string? id,
        string? name,
        string? moniker,
        string? source,
        string[]? query,
        bool allowHashMismatch,
        bool force,
        string? log,
        PSPackageRepairMode mode)
    {
        var warnings = new List<string>();
        if (allowHashMismatch)
            warnings.Add("AllowHashMismatch is accepted for compatibility but is not implemented by Pinget repair.");

        warnings.Add("Pinget repair currently re-runs the package install flow for the selected installed package.");

        var listQuery = new ListQuery
        {
            Id = Normalize(id ?? inputObject?.Id),
            Name = Normalize(name ?? inputObject?.Name),
            Moniker = Normalize(moniker ?? inputObject?.Moniker),
            Source = Normalize(source ?? inputObject?.Source),
            Query = JoinQuery(query),
            Version = Normalize(version) ?? GetCatalogVersion(inputObject),
            Exact = true,
        };

        var installed = _repository.List(listQuery);
        warnings.AddRange(installed.Warnings);
        if (installed.Matches.Count == 0)
            throw new InvalidOperationException("No installed package matched the supplied repair query.");
        if (installed.Matches.Count > 1)
            throw new InvalidOperationException("Multiple installed packages matched the supplied repair query.");

        var match = installed.Matches[0];
        var request = new InstallRequest
        {
            Query = new PackageQuery
            {
                Id = match.Id,
                Source = match.SourceName,
                Exact = true,
                Version = Normalize(version) ?? match.InstalledVersion,
            },
            Force = force,
            LogPath = Normalize(log),
            Mode = ToInstallerMode(mode),
        };

        var result = _repository.Install(request);
        warnings.AddRange(result.Warnings);
        return new CommandResult<PSRepairResult>(ToPsRepairResult(result, request, new PSInstalledCatalogPackage(
            match.Id,
            match.Name,
            match.SourceName ?? string.Empty,
            null,
            match.InstalledVersion,
            string.IsNullOrWhiteSpace(match.AvailableVersion) ? [] : [match.AvailableVersion],
            match.Publisher,
            match.Scope)), warnings);
    }

    private static PackageQuery BuildPackageQuery(
        string? id,
        string? name,
        string? moniker,
        string? source,
        string[]? query,
        string? tag,
        string? command,
        uint count,
        PSPackageFieldMatchOption matchOption)
    {
        return new PackageQuery
        {
            Id = Normalize(id),
            Name = Normalize(name),
            Moniker = Normalize(moniker),
            Source = Normalize(source),
            Query = JoinQuery(query),
            Tag = Normalize(tag),
            Command = Normalize(command),
            Count = count == 0 ? null : (int)count,
            Exact = UsesExactMatch(matchOption),
        };
    }

    private static ListQuery BuildListQuery(
        string? id,
        string? name,
        string? moniker,
        string? source,
        string[]? query,
        string? tag,
        string? command,
        uint count,
        PSPackageFieldMatchOption matchOption)
    {
        return new ListQuery
        {
            Id = Normalize(id),
            Name = Normalize(name),
            Moniker = Normalize(moniker),
            Source = Normalize(source),
            Query = JoinQuery(query),
            Tag = Normalize(tag),
            Command = Normalize(command),
            Count = count == 0 ? null : (int)count,
            Exact = UsesExactMatch(matchOption),
        };
    }

    private static void AddIgnoredDownloadWarnings(
        List<string> warnings,
        bool allowHashMismatch,
        bool skipDependencies,
        bool skipMicrosoftStoreLicense,
        PSWindowsPlatform platform,
        string? targetOsVersion)
    {
        if (allowHashMismatch)
            warnings.Add("AllowHashMismatch is not implemented by Pinget export and was ignored.");
        if (skipDependencies)
            warnings.Add("SkipDependencies does not affect Pinget export and was ignored.");
        if (skipMicrosoftStoreLicense)
            warnings.Add("SkipMicrosoftStoreLicense is not implemented by Pinget export and was ignored.");
        if (platform != PSWindowsPlatform.Default)
            warnings.Add("Platform-specific export selection is not implemented by Pinget export and was ignored.");
        if (!string.IsNullOrWhiteSpace(targetOsVersion))
            warnings.Add("TargetOSVersion is not implemented by Pinget export and was ignored.");
    }

    private static void AddUnsupportedActionWarnings(List<string> warnings, bool allowHashMismatch, string? header)
    {
        if (allowHashMismatch)
            warnings.Add("AllowHashMismatch is accepted for compatibility but is not implemented by Pinget actions.");
        if (!string.IsNullOrWhiteSpace(header))
            warnings.Add("Header is accepted for compatibility but is not implemented by Pinget actions.");
    }

    private static string? GetCatalogVersion(PSCatalogPackage? inputObject) => inputObject switch
    {
        PSFoundCatalogPackage found => found.Version,
        PSInstalledCatalogPackage installed when installed.AvailableVersions.Count > 0 => installed.AvailableVersions[0],
        PSInstalledCatalogPackage installed => installed.InstalledVersion,
        _ => null,
    };

    private static InstallerMode ToInstallerMode(PSPackageInstallMode mode) => mode switch
    {
        PSPackageInstallMode.Silent => InstallerMode.Silent,
        PSPackageInstallMode.Interactive => InstallerMode.Interactive,
        _ => InstallerMode.SilentWithProgress,
    };

    private static InstallerMode ToInstallerMode(PSPackageUninstallMode mode) => mode switch
    {
        PSPackageUninstallMode.Silent => InstallerMode.Silent,
        PSPackageUninstallMode.Interactive => InstallerMode.Interactive,
        _ => InstallerMode.SilentWithProgress,
    };

    private static InstallerMode ToInstallerMode(PSPackageRepairMode mode) => mode switch
    {
        PSPackageRepairMode.Silent => InstallerMode.Silent,
        PSPackageRepairMode.Interactive => InstallerMode.Interactive,
        _ => InstallerMode.SilentWithProgress,
    };

    private static PSInstallResult ToPsInstallResult(InstallResult result, InstallRequest request, PSCatalogPackage? inputObject)
    {
        var identity = ResolveIdentity(result.PackageId, request.Query.Name ?? inputObject?.Name, request.Query.Source ?? inputObject?.Source);
        return new PSInstallResult
        {
            Id = result.PackageId,
            Name = identity.Name,
            Source = identity.Source,
            CorrelationData = result.InstallerPath,
            InstallerErrorCode = unchecked((uint)result.ExitCode),
            RebootRequired = false,
            Status = result.Success ? "Ok" : result.NoOp ? "NoOp" : "Error",
        };
    }

    private static PSUninstallResult ToPsUninstallResult(InstallResult result, UninstallRequest request, PSCatalogPackage? inputObject)
    {
        var identity = ResolveIdentity(result.PackageId, request.Query.Name ?? inputObject?.Name, request.Query.Source ?? inputObject?.Source);
        return new PSUninstallResult
        {
            Id = result.PackageId,
            Name = identity.Name,
            Source = identity.Source,
            CorrelationData = result.InstallerPath,
            UninstallerErrorCode = unchecked((uint)result.ExitCode),
            RebootRequired = false,
            Status = result.Success ? "Ok" : result.NoOp ? "NoOp" : "Error",
        };
    }

    private static PSRepairResult ToPsRepairResult(InstallResult result, InstallRequest request, PSCatalogPackage? inputObject)
    {
        var identity = ResolveIdentity(result.PackageId, request.Query.Name ?? inputObject?.Name, request.Query.Source ?? inputObject?.Source);
        return new PSRepairResult
        {
            Id = result.PackageId,
            Name = identity.Name,
            Source = identity.Source,
            CorrelationData = result.InstallerPath,
            RepairErrorCode = unchecked((uint)result.ExitCode),
            RebootRequired = false,
            Status = result.Success ? "Ok" : result.NoOp ? "NoOp" : "Error",
        };
    }

    private static (string Name, string? Source) ResolveIdentity(string id, string? name, string? source) =>
        (string.IsNullOrWhiteSpace(name) ? id : name, Normalize(source));

    private string GetUserSettingsPath() => Path.Combine(_repository.AppRoot, "user-settings.json");

    private string GetAdminSettingsPath() => Path.Combine(_repository.AppRoot, "admin-settings.json");

    private static JsonObject LoadJsonObject(string path)
    {
        if (!File.Exists(path))
            return new JsonObject();

        var node = JsonNode.Parse(File.ReadAllText(path));
        return node as JsonObject ?? new JsonObject();
    }

    private static void SaveJsonObject(string path, JsonObject value)
    {
        var directory = Path.GetDirectoryName(path);
        if (!string.IsNullOrWhiteSpace(directory))
            Directory.CreateDirectory(directory);

        File.WriteAllText(path, value.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
    }

    private static JsonObject MergeJsonObjects(JsonObject current, JsonObject update)
    {
        var merged = current.DeepClone().AsObject();
        foreach (var entry in update)
        {
            if (entry.Value is JsonObject updateObject &&
                merged[entry.Key] is JsonObject currentObject)
            {
                merged[entry.Key] = MergeJsonObjects(currentObject, updateObject);
            }
            else
            {
                merged[entry.Key] = entry.Value?.DeepClone();
            }
        }

        return merged;
    }

    private static bool JsonContains(JsonNode current, JsonNode expected)
    {
        if (expected is JsonObject expectedObject)
        {
            var currentObject = current as JsonObject;
            if (currentObject is null)
                return false;

            foreach (var entry in expectedObject)
            {
                if (!currentObject.TryGetPropertyValue(entry.Key, out var currentChild) ||
                    currentChild is null ||
                    entry.Value is null ||
                    !JsonContains(currentChild, entry.Value))
                {
                    return false;
                }
            }

            return true;
        }

        if (expected is JsonArray expectedArray)
        {
            var currentArray = current as JsonArray;
            if (currentArray is null || currentArray.Count != expectedArray.Count)
                return false;

            for (var i = 0; i < expectedArray.Count; i++)
            {
                if (currentArray[i] is null || expectedArray[i] is null || !JsonContains(currentArray[i]!, expectedArray[i]!))
                    return false;
            }

            return true;
        }

        return JsonNode.DeepEquals(current, expected);
    }

    private static bool UsesExactMatch(PSPackageFieldMatchOption matchOption) =>
        matchOption is PSPackageFieldMatchOption.Equals or PSPackageFieldMatchOption.EqualsCaseInsensitive;

    private static string? JoinQuery(string[]? query)
    {
        if (query is null || query.Length == 0)
            return null;

        var values = query.Where(value => !string.IsNullOrWhiteSpace(value)).ToArray();
        return values.Length == 0 ? null : string.Join(' ', values);
    }

    private static string? Normalize(string? value) => string.IsNullOrWhiteSpace(value) ? null : value;

    private static SourceKind ParseSourceKind(string? value)
    {
        if (string.IsNullOrWhiteSpace(value) ||
            string.Equals(value, "rest", StringComparison.OrdinalIgnoreCase) ||
            string.Equals(value, "Microsoft.Rest", StringComparison.OrdinalIgnoreCase))
        {
            return SourceKind.Rest;
        }

        if (string.Equals(value, "preindexed", StringComparison.OrdinalIgnoreCase) ||
            string.Equals(value, "Microsoft.PreIndexed.Package", StringComparison.OrdinalIgnoreCase))
        {
            return SourceKind.PreIndexed;
        }

        throw new InvalidOperationException($"Unsupported source type: {value}");
    }
}
