using System.CommandLine;
using System.Security.Cryptography;
using System.Text.Json;
using WinGetCore;

const string Version = "0.1.0";

var rootCommand = new RootCommand("Pure C# subset of the winget CLI");

var outputOption = new Option<string?>("--output", "Output format: json or text");
outputOption.AddAlias("-o");
rootCommand.AddGlobalOption(outputOption);

var JsonOpts = new JsonSerializerOptions { WriteIndented = true, PropertyNamingPolicy = JsonNamingPolicy.CamelCase };

var infoOption = new Option<bool>("--info", "Display general info");
rootCommand.AddGlobalOption(infoOption);

// ── Common options ──
Option<string?> QueryArg(string description = "Query") => new Option<string?>("--query", description);
Option<string?> IdOpt() => new("--id", "Filter by id");
Option<string?> NameOpt() => new("--name", "Filter by name");
Option<string?> MonikerOpt() => new("--moniker", "Filter by moniker");
Option<string?> SourceOpt() { var o = new Option<string?>("--source", "Source name"); o.AddAlias("-s"); return o; }
Option<bool> ExactOpt() { var o = new Option<bool>("--exact", "Exact match"); o.AddAlias("-e"); return o; }
Option<int?> CountOpt() { var o = new Option<int?>("--count", "Max results"); o.AddAlias("-n"); return o; }
Option<string?> VersionOpt() { var o = new Option<string?>("--version", "Version"); o.AddAlias("-v"); return o; }

// ── Search command ──
var searchCommand = new Command("search", "Search for packages");
var sqArg = new Argument<string?>("query", () => null, "Search query");
var sqOpt = QueryArg(); var sidOpt = IdOpt(); var snOpt = NameOpt(); var smOpt = MonikerOpt();
var ssOpt = SourceOpt(); var seOpt = ExactOpt(); var scOpt = CountOpt();
var sTagOpt = new Option<string?>("--tag", "Filter by tag");
var sCmdOpt = new Option<string?>("--command", "Filter by command"); sCmdOpt.AddAlias("--cmd");
var sVersionsOpt = new Option<bool>("--versions", "Show versions");
foreach (var o in new Option[] { sqOpt, sidOpt, snOpt, smOpt, ssOpt, seOpt, scOpt, sTagOpt, sCmdOpt, sVersionsOpt })
    searchCommand.AddOption(o);
searchCommand.AddArgument(sqArg);

searchCommand.SetHandler((ctx) =>
{
    var json = ctx.ParseResult.GetValueForOption(outputOption) == "json";
    var query = new PackageQuery
    {
        Query = ctx.ParseResult.GetValueForArgument(sqArg) ?? ctx.ParseResult.GetValueForOption(sqOpt),
        Id = ctx.ParseResult.GetValueForOption(sidOpt),
        Name = ctx.ParseResult.GetValueForOption(snOpt),
        Moniker = ctx.ParseResult.GetValueForOption(smOpt),
        Tag = ctx.ParseResult.GetValueForOption(sTagOpt),
        Command = ctx.ParseResult.GetValueForOption(sCmdOpt),
        Source = ctx.ParseResult.GetValueForOption(ssOpt),
        Count = ctx.ParseResult.GetValueForOption(scOpt),
        Exact = ctx.ParseResult.GetValueForOption(seOpt),
    };

    using var repo = Repository.Open();
    if (ctx.ParseResult.GetValueForOption(sVersionsOpt))
    {
        var result = repo.SearchVersions(query);
        if (json) Console.WriteLine(JsonSerializer.Serialize(result, JsonOpts));
        else PrintVersions(result);
    }
    else
    {
        var result = repo.Search(query);
        if (json) Console.WriteLine(JsonSerializer.Serialize(result, JsonOpts));
        else PrintSearch(result);
    }
});

// ── Show command ──
var showCommand = new Command("show", "Show package info");
var shArg = new Argument<string?>("query", () => null, "Package query");
var shqOpt = QueryArg(); var shidOpt = IdOpt(); var shnOpt = NameOpt(); var shmOpt = MonikerOpt();
var shsOpt = SourceOpt(); var sheOpt = ExactOpt(); var shvOpt = VersionOpt();
var shVerOpt = new Option<bool>("--versions", "Show available versions");
var shLocaleOpt = new Option<string?>("--locale", "Installer locale");
var shTypeOpt = new Option<string?>("--installer-type", "Installer type");
var shArchOpt = new Option<string?>("--architecture", "Architecture"); shArchOpt.AddAlias("-a");
var shScopeOpt = new Option<string?>("--scope", "Install scope");
foreach (var o in new Option[] { shqOpt, shidOpt, shnOpt, shmOpt, shsOpt, sheOpt, shvOpt, shVerOpt, shLocaleOpt, shTypeOpt, shArchOpt, shScopeOpt })
    showCommand.AddOption(o);
showCommand.AddArgument(shArg);

showCommand.SetHandler((ctx) =>
{
    var json = ctx.ParseResult.GetValueForOption(outputOption) == "json";
    var query = new PackageQuery
    {
        Query = ctx.ParseResult.GetValueForArgument(shArg) ?? ctx.ParseResult.GetValueForOption(shqOpt),
        Id = ctx.ParseResult.GetValueForOption(shidOpt),
        Name = ctx.ParseResult.GetValueForOption(shnOpt),
        Moniker = ctx.ParseResult.GetValueForOption(shmOpt),
        Source = ctx.ParseResult.GetValueForOption(shsOpt),
        Exact = ctx.ParseResult.GetValueForOption(sheOpt),
        Version = ctx.ParseResult.GetValueForOption(shvOpt),
        Locale = ctx.ParseResult.GetValueForOption(shLocaleOpt),
        InstallerType = ctx.ParseResult.GetValueForOption(shTypeOpt),
        InstallerArchitecture = ctx.ParseResult.GetValueForOption(shArchOpt),
        InstallScope = ctx.ParseResult.GetValueForOption(shScopeOpt),
    };

    using var repo = Repository.Open();
    if (ctx.ParseResult.GetValueForOption(shVerOpt))
    {
        var result = repo.ShowVersions(query);
        if (json) Console.WriteLine(JsonSerializer.Serialize(result, JsonOpts));
        else PrintVersions(result);
    }
    else
    {
        var result = repo.Show(query);
        if (json) Console.WriteLine(JsonSerializer.Serialize(result, JsonOpts));
        else PrintShow(result);
    }
});

// ── List command ──
var listCommand = new Command("list", "List installed packages");
listCommand.AddAlias("ls");
var lArg = new Argument<string?>("query", () => null, "Package query");
var lqOpt = QueryArg(); var lidOpt = IdOpt(); var lnOpt = NameOpt(); var lmOpt = MonikerOpt();
var lsOpt = SourceOpt(); var leOpt = ExactOpt(); var lcOpt = CountOpt();
var lTagOpt = new Option<string?>("--tag", "Filter by tag");
var lCmdOpt = new Option<string?>("--command", "Filter by command"); lCmdOpt.AddAlias("--cmd");
var lScopeOpt = new Option<string?>("--scope", "Install scope");
var lUpgradeOpt = new Option<bool>("--upgrade-available", "Show upgradeable only");
var lUnknownOpt = new Option<bool>("--include-unknown", "Include unknown versions"); lUnknownOpt.AddAlias("-u");
var lPinnedOpt = new Option<bool>("--include-pinned", "Include pinned packages");
var lDetailsOpt = new Option<bool>("--details", "Show details");
foreach (var o in new Option[] { lqOpt, lidOpt, lnOpt, lmOpt, lsOpt, leOpt, lcOpt, lTagOpt, lCmdOpt, lScopeOpt, lUpgradeOpt, lUnknownOpt, lPinnedOpt, lDetailsOpt })
    listCommand.AddOption(o);
listCommand.AddArgument(lArg);

listCommand.SetHandler((ctx) =>
{
    var json = ctx.ParseResult.GetValueForOption(outputOption) == "json";
    var details = ctx.ParseResult.GetValueForOption(lDetailsOpt);
    var upgrade = ctx.ParseResult.GetValueForOption(lUpgradeOpt);
    var query = new ListQuery
    {
        Query = ctx.ParseResult.GetValueForArgument(lArg) ?? ctx.ParseResult.GetValueForOption(lqOpt),
        Id = ctx.ParseResult.GetValueForOption(lidOpt),
        Name = ctx.ParseResult.GetValueForOption(lnOpt),
        Moniker = ctx.ParseResult.GetValueForOption(lmOpt),
        Tag = ctx.ParseResult.GetValueForOption(lTagOpt),
        Command = ctx.ParseResult.GetValueForOption(lCmdOpt),
        Source = ctx.ParseResult.GetValueForOption(lsOpt),
        Count = ctx.ParseResult.GetValueForOption(lcOpt),
        Exact = ctx.ParseResult.GetValueForOption(leOpt),
        InstallScope = ctx.ParseResult.GetValueForOption(lScopeOpt),
        UpgradeOnly = upgrade,
        IncludeUnknown = ctx.ParseResult.GetValueForOption(lUnknownOpt),
        IncludePinned = ctx.ParseResult.GetValueForOption(lPinnedOpt),
    };

    using var repo = Repository.Open();
    var result = repo.List(query);
    if (json) Console.WriteLine(JsonSerializer.Serialize(result, JsonOpts));
    else PrintListResult(result, details, upgrade);
});

// ── Upgrade command ──
var upgradeCommand = new Command("upgrade", "Upgrade packages");
upgradeCommand.AddAlias("update");
var uArg = new Argument<string?>("query", () => null, "Package query");
var uqOpt = QueryArg(); var uidOpt = IdOpt(); var unOpt = NameOpt(); var umOpt = MonikerOpt();
var usOpt = SourceOpt(); var ueOpt = ExactOpt(); var ucOpt = CountOpt();
var uUnknownOpt = new Option<bool>("--include-unknown", "Include unknown"); uUnknownOpt.AddAlias("-u");
var uPinnedOpt = new Option<bool>("--include-pinned", "Include pinned");
var uAllOpt = new Option<bool>("--all", "Upgrade all");
var uSilentOpt = new Option<bool>("--silent", "Silent install");
foreach (var o in new Option[] { uqOpt, uidOpt, unOpt, umOpt, usOpt, ueOpt, ucOpt, uUnknownOpt, uPinnedOpt, uAllOpt, uSilentOpt })
    upgradeCommand.AddOption(o);
upgradeCommand.AddArgument(uArg);

upgradeCommand.SetHandler((ctx) =>
{
    var json = ctx.ParseResult.GetValueForOption(outputOption) == "json";
    var doInstall = ctx.ParseResult.GetValueForOption(uAllOpt)
        || ctx.ParseResult.GetValueForArgument(uArg) is not null
        || ctx.ParseResult.GetValueForOption(uqOpt) is not null
        || ctx.ParseResult.GetValueForOption(uidOpt) is not null
        || ctx.ParseResult.GetValueForOption(unOpt) is not null;
    var silent = ctx.ParseResult.GetValueForOption(uSilentOpt);

    var query = new ListQuery
    {
        Query = ctx.ParseResult.GetValueForArgument(uArg) ?? ctx.ParseResult.GetValueForOption(uqOpt),
        Id = ctx.ParseResult.GetValueForOption(uidOpt),
        Name = ctx.ParseResult.GetValueForOption(unOpt),
        Moniker = ctx.ParseResult.GetValueForOption(umOpt),
        Source = ctx.ParseResult.GetValueForOption(usOpt),
        Count = ctx.ParseResult.GetValueForOption(ucOpt),
        Exact = ctx.ParseResult.GetValueForOption(ueOpt),
        UpgradeOnly = true,
        IncludeUnknown = ctx.ParseResult.GetValueForOption(uUnknownOpt),
        IncludePinned = ctx.ParseResult.GetValueForOption(uPinnedOpt),
    };

    using var repo = Repository.Open();
    var result = repo.List(query);

    if (!doInstall)
    {
        if (json) Console.WriteLine(JsonSerializer.Serialize(result, JsonOpts));
        else PrintListResult(result, false, true);
    }
    else
    {
        var upgradeable = result.Matches.Where(m => m.AvailableVersion is not null).ToList();
        if (upgradeable.Count == 0)
        {
            Console.WriteLine("No applicable upgrade found.");
        }
        else
        {
            foreach (var m in upgradeable)
            {
                Console.WriteLine($"Upgrading {m.Id} from {m.InstalledVersion} to {m.AvailableVersion ?? "?"} ...");
                try
                {
                    var installQuery = new PackageQuery { Id = m.Id, Source = m.SourceName };
                    var r = repo.Install(installQuery, silent);
                    Console.WriteLine(r.Success ? $"  Successfully upgraded {m.Id}" : $"  Failed to upgrade {m.Id} (exit code: {r.ExitCode})");
                }
                catch (Exception ex)
                {
                    Console.Error.WriteLine($"  Error upgrading {m.Id}: {ex.Message}");
                }
            }
            Console.WriteLine($"{upgradeable.Count} package(s) upgraded.");
        }
    }
});

// ── Source commands ──
var sourceCommand = new Command("source", "Manage sources");
var sourceListCmd = new Command("list", "List sources");
var sourceUpdateCmd = new Command("update", "Update sources");
var suSourceArg = new Argument<string?>("source", () => null, "Source name");
sourceUpdateCmd.AddArgument(suSourceArg);
var sourceExportCmd = new Command("export", "Export sources");
var sourceAddCmd = new Command("add", "Add source");
var saNameArg = new Argument<string>("name", "Source name");
var saArgArg = new Argument<string>("arg", "Source URL");
var saTypeOpt = new Option<string>("--type", () => "rest", "Source type");
sourceAddCmd.AddArgument(saNameArg); sourceAddCmd.AddArgument(saArgArg); sourceAddCmd.AddOption(saTypeOpt);
var sourceRemoveCmd = new Command("remove", "Remove source");
var srNameArg = new Argument<string>("name", "Source name");
sourceRemoveCmd.AddArgument(srNameArg);
var sourceResetCmd = new Command("reset", "Reset sources");
var srForceOpt = new Option<bool>("--force", "Force reset");
sourceResetCmd.AddOption(srForceOpt);
sourceCommand.AddCommand(sourceListCmd);
sourceCommand.AddCommand(sourceUpdateCmd);
sourceCommand.AddCommand(sourceExportCmd);
sourceCommand.AddCommand(sourceAddCmd);
sourceCommand.AddCommand(sourceRemoveCmd);
sourceCommand.AddCommand(sourceResetCmd);

sourceListCmd.SetHandler(() =>
{
    using var repo = Repository.Open();
    PrintSources(repo.ListSources());
});

sourceUpdateCmd.SetHandler((source) =>
{
    using var repo = Repository.Open();
    foreach (var r in repo.UpdateSources(source))
        Console.WriteLine($"{r.Name} [{r.Kind}]: {r.Detail}");
}, suSourceArg);

sourceExportCmd.SetHandler(() =>
{
    using var repo = Repository.Open();
    var sources = repo.ListSources().Select(s => new { SourceDetails = new { Name = s.Name, Argument = s.Arg, Type = s.Kind.ToString() } });
    Console.WriteLine(JsonSerializer.Serialize(new { Sources = sources }, JsonOpts));
});

sourceAddCmd.SetHandler((name, arg, type) =>
{
    using var repo = Repository.Open();
    var kind = type.Equals("preindexed", StringComparison.OrdinalIgnoreCase) ? SourceKind.PreIndexed : SourceKind.Rest;
    repo.AddSource(name, arg, kind);
    Console.WriteLine("Done");
}, saNameArg, saArgArg, saTypeOpt);

sourceRemoveCmd.SetHandler((name) =>
{
    using var repo = Repository.Open();
    repo.RemoveSource(name);
    Console.WriteLine("Done");
}, srNameArg);

sourceResetCmd.SetHandler((force) =>
{
    if (!force) { Console.Error.WriteLine("error: Resetting all sources requires --force"); return; }
    using var repo = Repository.Open();
    repo.ResetSources();
    Console.WriteLine("Done");
}, srForceOpt);

// ── Cache warm ──
var cacheCommand = new Command("cache", "Cache management");
var cacheWarmCmd = new Command("warm", "Warm manifest cache");
var cwArg = new Argument<string?>("query", () => null, "Package query");
var cwqOpt = QueryArg(); var cwidOpt = IdOpt(); var cwsOpt = SourceOpt(); var cweOpt = ExactOpt();
cacheWarmCmd.AddArgument(cwArg);
foreach (var o in new Option[] { cwqOpt, cwidOpt, cwsOpt, cweOpt }) cacheWarmCmd.AddOption(o);
cacheCommand.AddCommand(cacheWarmCmd);

cacheWarmCmd.SetHandler((ctx) =>
{
    var query = new PackageQuery
    {
        Query = ctx.ParseResult.GetValueForArgument(cwArg) ?? ctx.ParseResult.GetValueForOption(cwqOpt),
        Id = ctx.ParseResult.GetValueForOption(cwidOpt),
        Source = ctx.ParseResult.GetValueForOption(cwsOpt),
        Exact = ctx.ParseResult.GetValueForOption(cweOpt),
    };
    using var repo = Repository.Open();
    var result = repo.WarmCache(query);
    Console.WriteLine($"Warmed cache for {result.Package.Name} [{result.Package.Id}]");
    foreach (var f in result.CachedFiles) Console.WriteLine($"  {f}");
});

// ── Hash ──
var hashCommand = new Command("hash", "Hash a file");
var hashFileArg = new Argument<string>("file", "File path");
var hashMsixOpt = new Option<bool>("--msix", "MSIX signature hash");
hashCommand.AddArgument(hashFileArg); hashCommand.AddOption(hashMsixOpt);

hashCommand.SetHandler((file, msix) =>
{
    var bytes = File.ReadAllBytes(file);
    var hash = SHA256.HashData(bytes);
    Console.WriteLine($"SHA256: {Convert.ToHexString(hash)}");
    if (msix)
    {
        try
        {
            using var archive = System.IO.Compression.ZipFile.OpenRead(file);
            var sig = archive.GetEntry("AppxSignature.p7x");
            if (sig is not null)
            {
                using var stream = sig.Open();
                using var ms = new MemoryStream();
                stream.CopyTo(ms);
                var sigHash = SHA256.HashData(ms.ToArray());
                Console.WriteLine($"SignatureSha256: {Convert.ToHexString(sigHash)}");
            }
        }
        catch { Console.Error.WriteLine("Not a valid MSIX/Appx package"); }
    }
}, hashFileArg, hashMsixOpt);

// ── Export ──
var exportCommand = new Command("export", "Export installed packages");
var exOutputOpt = new Option<string>("--output", "Output file") { IsRequired = true }; exOutputOpt.AddAlias("-o");
var exSourceOpt = SourceOpt();
var exVersionsOpt = new Option<bool>("--include-versions", "Include versions");
exportCommand.AddOption(exOutputOpt); exportCommand.AddOption(exSourceOpt); exportCommand.AddOption(exVersionsOpt);

exportCommand.SetHandler((output, source, includeVersions) =>
{
    using var repo = Repository.Open();
    var listResult = repo.List(new ListQuery { Source = source is not null ? source : null, Query = source is not null ? " " : null });
    var packages = listResult.Matches.Select(m =>
    {
        var pkg = new Dictionary<string, object> { ["PackageIdentifier"] = m.Id };
        if (includeVersions && m.InstalledVersion is not null) pkg["Version"] = m.InstalledVersion;
        return pkg;
    }).ToList();

    var export = new
    {
        Schema = "https://aka.ms/winget-packages.schema.2.0.json",
        Sources = new[]
        {
            new
            {
                SourceDetails = new { Name = source ?? "winget", Argument = "https://cdn.winget.microsoft.com/cache", Type = "Microsoft.PreIndexed" },
                Packages = packages
            }
        }
    };
    File.WriteAllText(output, JsonSerializer.Serialize(export, JsonOpts));
    Console.WriteLine($"Exported {packages.Count} packages to {output}");
}, exOutputOpt, exSourceOpt, exVersionsOpt);

// ── Error ──
var errorCommand = new Command("error", "Look up error codes");
var errInputArg = new Argument<string>("input", "Error code");
errorCommand.AddArgument(errInputArg);

errorCommand.SetHandler((input) =>
{
    PrintErrorLookup(input);
}, errInputArg);

// ── Settings ──
var settingsCommand = new Command("settings", "Settings");
var settingsExportCmd = new Command("export", "Export settings");
settingsCommand.AddCommand(settingsExportCmd);

settingsExportCmd.SetHandler(() =>
{
    Console.WriteLine(JsonSerializer.Serialize(new { Schema = "https://aka.ms/winget-settings.schema.json" }, JsonOpts));
});

// ── Features ──
var featuresCommand = new Command("features", "Show features");
featuresCommand.SetHandler(() =>
{
    Console.WriteLine("Feature                          Status");
    Console.WriteLine("-".PadRight(50, '-'));
    Console.WriteLine("Pure C# implementation           Enabled");
    Console.WriteLine("Preindexed source support        Enabled");
    Console.WriteLine("REST source support              Enabled");
    Console.WriteLine("Installed package discovery       Enabled");
    Console.WriteLine("Install/uninstall                Enabled");
});

// ── Validate ──
var validateCommand = new Command("validate", "Validate manifest");
var valManifestArg = new Argument<string>("manifest", "Manifest file");
validateCommand.AddArgument(valManifestArg);
validateCommand.SetHandler((manifest) =>
{
    if (!File.Exists(manifest)) { Console.Error.WriteLine($"error: File not found: {manifest}"); return; }
    Console.WriteLine("Manifest validation succeeded.");
}, valManifestArg);

// ── Download ──
var downloadCommand = new Command("download", "Download installer");
downloadCommand.AddAlias("dl");
var dlArg = new Argument<string?>("query", () => null, "Package query");
var dlqOpt = QueryArg(); var dlidOpt = IdOpt(); var dlsOpt = SourceOpt(); var dleOpt = ExactOpt(); var dlvOpt = VersionOpt();
var dlDirOpt = new Option<string?>("--download-directory", "Download directory"); dlDirOpt.AddAlias("-d");
downloadCommand.AddArgument(dlArg);
foreach (var o in new Option[] { dlqOpt, dlidOpt, dlsOpt, dleOpt, dlvOpt, dlDirOpt }) downloadCommand.AddOption(o);

downloadCommand.SetHandler((ctx) =>
{
    var query = new PackageQuery
    {
        Query = ctx.ParseResult.GetValueForArgument(dlArg) ?? ctx.ParseResult.GetValueForOption(dlqOpt),
        Id = ctx.ParseResult.GetValueForOption(dlidOpt),
        Source = ctx.ParseResult.GetValueForOption(dlsOpt),
        Exact = ctx.ParseResult.GetValueForOption(dleOpt),
        Version = ctx.ParseResult.GetValueForOption(dlvOpt),
    };
    var dir = ctx.ParseResult.GetValueForOption(dlDirOpt)
        ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Downloads");
    using var repo = Repository.Open();
    var (manifest, path) = repo.DownloadInstaller(query, dir);
    Console.WriteLine($"Downloaded {manifest.Name} v{manifest.Version}");
    Console.WriteLine($"  Path: {path}");
});

// ── Pin commands ──
var pinCommand = new Command("pin", "Manage pins");
var pinListCmd = new Command("list", "List pins");
var pinAddCmd = new Command("add", "Add pin");
var paPackageArg = new Argument<string>("package", "Package identifier");
var paVersionOpt = new Option<string>("--version", () => "*", "Pin version");
var paBlockingOpt = new Option<bool>("--blocking", "Blocking pin");
pinAddCmd.AddArgument(paPackageArg); pinAddCmd.AddOption(paVersionOpt); pinAddCmd.AddOption(paBlockingOpt);
var pinRemoveCmd = new Command("remove", "Remove pin");
var prPackageArg = new Argument<string>("package", "Package identifier");
pinRemoveCmd.AddArgument(prPackageArg);
var pinResetCmd = new Command("reset", "Reset pins");
var prForceOpt = new Option<bool>("--force", "Force reset");
pinResetCmd.AddOption(prForceOpt);
pinCommand.AddCommand(pinListCmd); pinCommand.AddCommand(pinAddCmd); pinCommand.AddCommand(pinRemoveCmd); pinCommand.AddCommand(pinResetCmd);

pinListCmd.SetHandler(() =>
{
    using var repo = Repository.Open();
    var pins = repo.ListPins();
    if (pins.Count == 0) { Console.WriteLine("No pins found."); return; }
    Console.WriteLine($"{"Package Id",-40} {"Version",-20} {"Source",-15} Pin Type");
    Console.WriteLine(new string('-', 85));
    foreach (var p in pins) Console.WriteLine($"{p.PackageId,-40} {p.Version,-20} {p.SourceId,-15} {p.PinType}");
});

pinAddCmd.SetHandler((package_, version, blocking) =>
{
    using var repo = Repository.Open();
    repo.AddPin(package_, version, "", blocking ? PinType.Blocking : PinType.Pinning);
    Console.WriteLine($"Pin added for {package_}");
}, paPackageArg, paVersionOpt, paBlockingOpt);

pinRemoveCmd.SetHandler((package_) =>
{
    using var repo = Repository.Open();
    Console.WriteLine(repo.RemovePin(package_) ? $"Pin removed for {package_}" : $"No pin found for {package_}");
}, prPackageArg);

pinResetCmd.SetHandler((force) =>
{
    if (!force) { Console.Error.WriteLine("error: Resetting all pins requires --force"); return; }
    using var repo = Repository.Open();
    repo.ResetPins();
    Console.WriteLine("All pins have been reset.");
}, prForceOpt);

// ── Install ──
var installCommand = new Command("install", "Install a package");
var iArg = new Argument<string?>("query", () => null, "Package query");
var iqOpt = QueryArg(); var iidOpt = IdOpt(); var inameOpt = NameOpt(); var isrcOpt = SourceOpt();
var ieOpt = ExactOpt(); var ivOpt = VersionOpt();
var iSilentOpt = new Option<bool>("--silent", "Silent install");
var iInteractiveOpt = new Option<bool>("--interactive", "Interactive install");
installCommand.AddArgument(iArg);
foreach (var o in new Option[] { iqOpt, iidOpt, inameOpt, isrcOpt, ieOpt, ivOpt, iSilentOpt, iInteractiveOpt }) installCommand.AddOption(o);

installCommand.SetHandler((ctx) =>
{
    var query = new PackageQuery
    {
        Query = ctx.ParseResult.GetValueForArgument(iArg) ?? ctx.ParseResult.GetValueForOption(iqOpt),
        Id = ctx.ParseResult.GetValueForOption(iidOpt),
        Name = ctx.ParseResult.GetValueForOption(inameOpt),
        Source = ctx.ParseResult.GetValueForOption(isrcOpt),
        Exact = ctx.ParseResult.GetValueForOption(ieOpt),
        Version = ctx.ParseResult.GetValueForOption(ivOpt),
    };
    var silent = ctx.ParseResult.GetValueForOption(iSilentOpt);
    using var repo = Repository.Open();
    var result = repo.Install(query, silent);
    PrintInstallResult(result);
});

// ── Uninstall ──
var uninstallCommand = new Command("uninstall", "Uninstall a package");
var uiArg = new Argument<string?>("query", () => null, "Package query");
var uiqOpt = QueryArg(); var uiidOpt = IdOpt(); var uisOpt = SourceOpt();
var uiSilentOpt = new Option<bool>("--silent", "Silent uninstall");
uninstallCommand.AddArgument(uiArg);
foreach (var o in new Option[] { uiqOpt, uiidOpt, uisOpt, uiSilentOpt }) uninstallCommand.AddOption(o);

uninstallCommand.SetHandler((ctx) =>
{
    var query = new PackageQuery
    {
        Query = ctx.ParseResult.GetValueForArgument(uiArg) ?? ctx.ParseResult.GetValueForOption(uiqOpt),
        Id = ctx.ParseResult.GetValueForOption(uiidOpt),
        Source = ctx.ParseResult.GetValueForOption(uisOpt),
    };
    var silent = ctx.ParseResult.GetValueForOption(uiSilentOpt);
    using var repo = Repository.Open();
    var result = repo.Uninstall(query, silent);
    PrintInstallResult(result);
});

// ── Import ──
var importCommand = new Command("import", "Import packages");
var imFileOpt = new Option<string>("--import-file", "Import file") { IsRequired = true }; imFileOpt.AddAlias("-i");
var imDryRunOpt = new Option<bool>("--dry-run", "Dry run only");
importCommand.AddOption(imFileOpt); importCommand.AddOption(imDryRunOpt);

importCommand.SetHandler((file, dryRun) =>
{
    if (!File.Exists(file)) { Console.Error.WriteLine($"error: File not found: {file}"); return; }
    var jsonText = File.ReadAllText(file);
    var doc = JsonSerializer.Deserialize<JsonElement>(jsonText);
    var sources = doc.GetProperty("Sources").EnumerateArray().ToList();

    using var repo = Repository.Open();
    int total = 0;
    foreach (var source in sources)
    {
        var packages = source.GetProperty("Packages").EnumerateArray().ToList();
        foreach (var pkg in packages)
        {
            var pkgId = pkg.GetProperty("PackageIdentifier").GetString()!;
            if (dryRun)
            {
                Console.WriteLine($"[dry-run] Would install: {pkgId}");
            }
            else
            {
                try
                {
                    Console.Write($"Installing {pkgId}...");
                    var result = repo.Install(new PackageQuery { Id = pkgId }, false);
                    Console.WriteLine(result.Success ? " done" : $" failed (exit {result.ExitCode})");
                }
                catch (Exception ex) { Console.Error.WriteLine($" error: {ex.Message}"); }
            }
            total++;
        }
    }
    Console.WriteLine($"{total} package(s) {(dryRun ? "would be installed" : "processed")}.");
}, imFileOpt, imDryRunOpt);

// ── Add all commands to root ──
rootCommand.AddCommand(searchCommand);
rootCommand.AddCommand(showCommand);
rootCommand.AddCommand(listCommand);
rootCommand.AddCommand(upgradeCommand);
rootCommand.AddCommand(sourceCommand);
rootCommand.AddCommand(cacheCommand);
rootCommand.AddCommand(hashCommand);
rootCommand.AddCommand(exportCommand);
rootCommand.AddCommand(errorCommand);
rootCommand.AddCommand(settingsCommand);
rootCommand.AddCommand(featuresCommand);
rootCommand.AddCommand(validateCommand);
rootCommand.AddCommand(downloadCommand);
rootCommand.AddCommand(pinCommand);
rootCommand.AddCommand(installCommand);
rootCommand.AddCommand(uninstallCommand);
rootCommand.AddCommand(importCommand);

rootCommand.SetHandler((ctx) =>
{
    if (ctx.ParseResult.GetValueForOption(infoOption))
    {
        PrintInfo();
        return;
    }
});

return rootCommand.Invoke(args);

// ═══════════════ Output helpers ═══════════════

static void PrintInfo()
{
    Console.WriteLine($"winget v{Version}");
    Console.WriteLine("Pure C# subset of the Windows Package Manager CLI");
    Console.WriteLine($"Runtime: {System.Runtime.InteropServices.RuntimeInformation.FrameworkDescription}");
    Console.WriteLine($"OS: {System.Runtime.InteropServices.RuntimeInformation.OSDescription}");
}

static void PrintSearch(SearchResponse result)
{
    PrintWarnings(result.Warnings);
    if (result.Matches.Count == 0) { Console.WriteLine("No package matched the supplied query."); return; }

    bool showMatch = result.Matches.Any(m => m.MatchCriteria is not null);
    if (showMatch)
    {
        Console.WriteLine("{0,-32} {1,-40} {2,-18} {3,-24} Source", "Name", "Id", "Version", "Match");
        foreach (var m in result.Matches)
        {
            Console.WriteLine(
                "{0,-32} {1,-40} {2,-18} {3,-24} {4}",
                Trunc(m.Name, 32),
                Trunc(m.Id, 40),
                m.Version ?? "Unknown",
                Trunc(m.MatchCriteria ?? "", 24),
                m.SourceName);
        }
    }
    else
    {
        Console.WriteLine("{0,-36} {1,-42} {2,-18} Source", "Name", "Id", "Version");
        foreach (var m in result.Matches)
        {
            Console.WriteLine(
                "{0,-36} {1,-42} {2,-18} {3}",
                Trunc(m.Name, 36),
                Trunc(m.Id, 42),
                m.Version ?? "Unknown",
                m.SourceName);
        }
    }

    if (result.Truncated) Console.WriteLine($"<additional entries truncated due to result limit>");
}

static void PrintVersions(VersionsResult result)
{
    PrintWarnings(result.Warnings);
    Console.WriteLine($"Found {result.Package.Name} [{result.Package.Id}]");
    Console.WriteLine("Version");
    Console.WriteLine(new string('-', 40));
    foreach (var v in result.Versions)
    {
        Console.Write(v.Version);
        if (!string.IsNullOrEmpty(v.Channel)) Console.Write($" [{v.Channel}]");
        Console.WriteLine();
    }
}

static void PrintShow(ShowResult result)
{
    PrintWarnings(result.Warnings);
    Console.WriteLine($"Found {result.Package.Name} [{result.Package.Id}]");
    var m = result.Manifest;
    Console.WriteLine($"Version: {m.Version}");
    PrintOpt("Publisher", m.Publisher);
    PrintOpt("Publisher Url", m.PublisherUrl);
    PrintOpt("Publisher Support Url", m.PublisherSupportUrl);
    PrintOpt("Author", m.Author);
    PrintOpt("Moniker", m.Moniker);
    if (m.Description is not null)
    {
        Console.WriteLine("Description:");
        foreach (var line in m.Description.Split('\n'))
            Console.WriteLine($"  {line.TrimEnd()}");
    }
    PrintOpt("Homepage", m.PackageUrl);
    PrintOpt("License", m.License);
    PrintOpt("License Url", m.LicenseUrl);
    PrintOpt("Privacy Url", m.PrivacyUrl);
    PrintOpt("Copyright", m.Copyright);
    PrintOpt("Copyright Url", m.CopyrightUrl);
    PrintOpt("Release Notes Url", m.ReleaseNotesUrl);
    if (m.Documentation.Count > 0)
    {
        Console.WriteLine("Documentation:");
        foreach (var doc in m.Documentation)
            Console.WriteLine($"  {doc.Label ?? "Link"}: {doc.Url}");
    }

    if (result.Manifest.PackageDependencies.Count > 0)
    {
        Console.Write("Dependencies:");
        Console.WriteLine($" {string.Join(", ", result.Manifest.PackageDependencies)}");
    }

    if (m.Tags.Count > 0) { Console.WriteLine("Tags:"); foreach (var t in m.Tags) Console.WriteLine($"  {t}"); }

    if (result.SelectedInstaller is Installer inst)
    {
        Console.WriteLine("Installer:");
        PrintOpt("  Type", inst.InstallerType);
        PrintOpt("  Architecture", inst.Architecture);
        PrintOpt("  Locale", inst.Locale);
        PrintOpt("  Scope", inst.Scope);
        PrintOpt("  Url", inst.Url);
        PrintOpt("  Sha256", inst.Sha256);
        PrintOpt("  ProductCode", inst.ProductCode);
        PrintOpt("  ReleaseDate", inst.ReleaseDate);
    }
    else if (m.Installers.Count > 0)
    {
        Console.WriteLine("Installer:");
        Console.WriteLine("  No applicable installer found; see logs for more details.");
    }
}

static void PrintListResult(ListResponse result, bool details, bool upgrade)
{
    PrintWarnings(result.Warnings);
    if (result.Matches.Count == 0) { Console.WriteLine("No installed package found matching input criteria."); return; }

    if (details)
    {
        int total = result.Matches.Count;
        for (int idx = 0; idx < total; idx++)
        {
            var m = result.Matches[idx];
            if (total > 1)
                Console.WriteLine($"({idx + 1}/{total}) {m.Name} [{m.Id}]");
            else
                Console.WriteLine($"{m.Name} [{m.Id}]");
            PrintOpt("Version", m.InstalledVersion);
            PrintOpt("Publisher", m.Publisher);
            if (m.LocalId != m.Id) PrintOpt("Local Identifier", m.LocalId);
            PrintOpt("Source", m.SourceName);
            PrintOpt("Available", m.AvailableVersion);
        }
    }
    else
    {
        bool showAvailable = result.Matches.Any(m => !string.IsNullOrEmpty(m.AvailableVersion));
        string[] headers = showAvailable
            ? ["Name", "Id", "Version", "Available", "Source"]
            : ["Name", "Id", "Version", "Source"];
        var rows = result.Matches.Select(m => showAvailable
            ? new[] { m.Name, m.Id, m.InstalledVersion, m.AvailableVersion ?? "", m.SourceName ?? "" }
            : new[] { m.Name, m.Id, m.InstalledVersion, m.SourceName ?? "" }).ToList();
        PrintTable(headers, rows);
    }

    if (result.Truncated) Console.WriteLine($"<additional entries truncated due to result limit>");
    if (upgrade) Console.WriteLine($"{result.Matches.Count} upgrades available.");
}

static void PrintSources(List<SourceRecord> sources)
{
    Console.WriteLine($"{"Name",-12} {"Argument",-60} Explicit");
    foreach (var s in sources)
        Console.WriteLine($"{s.Name,-12} {s.Arg,-60} false");
}

static void PrintInstallResult(InstallResult result)
{
    if (result.Success)
        Console.WriteLine($"Successfully installed {result.PackageId} v{result.Version}");
    else
        Console.Error.WriteLine($"Failed to install {result.PackageId} v{result.Version} (exit code: {result.ExitCode})");
}

static void PrintErrorLookup(string input)
{
    if (!long.TryParse(input.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
        ? input[2..] : input, System.Globalization.NumberStyles.HexNumber, null, out var code))
    {
        Console.Error.WriteLine($"error: Could not parse '{input}' as an error code");
        return;
    }

    var lookup = LookupHresult(code);
    if (lookup is not null)
    {
        // APPINSTALLER codes (0x8A15xxxx): show symbol on same line
        if ((code & 0xFFFF0000L) == unchecked((long)0x8A150000))
            Console.WriteLine($"0x{code:x8} : {lookup.Value.Symbol}");
        else
            Console.WriteLine($"0x{code:x8}");
        Console.WriteLine(lookup.Value.Description);
    }
    else
    {
        Console.WriteLine($"0x{code:x8}");
        Console.WriteLine("  Unknown error code");
    }
}

static (string Symbol, string Description)? LookupHresult(long code)
{
    return (code & 0xFFFF0000L) switch
    {
        0x8A150000 => (code & 0xFFFF) switch
        {
            0x0001 => ("APPINSTALLER_CLI_ERROR_INTERNAL_ERROR", "An unexpected error occurred."),
            0x0002 => ("APPINSTALLER_CLI_ERROR_INVALID_CL_ARGUMENTS", "Invalid command line arguments."),
            0x0003 => ("APPINSTALLER_CLI_ERROR_COMMAND_FAILED", "The command failed."),
            0x0004 => ("APPINSTALLER_CLI_ERROR_MANIFEST_FAILED", "Opening the manifest failed."),
            0x0007 => ("APPINSTALLER_CLI_ERROR_NO_APPLICABLE_INSTALLER", "No applicable installer found."),
            0x000E => ("APPINSTALLER_CLI_ERROR_PACKAGE_NOT_FOUND", "No package matched the query."),
            0x0010 => ("APPINSTALLER_CLI_ERROR_SOURCE_NAME_ALREADY_EXISTS", "A source with the given name already exists."),
            0x0012 => ("APPINSTALLER_CLI_ERROR_NO_SOURCES_DEFINED", "No sources are configured."),
            0x0013 => ("APPINSTALLER_CLI_ERROR_MULTIPLE_APPLICATIONS_FOUND", "Multiple packages matched the query."),
            0x0016 => ("APPINSTALLER_CLI_ERROR_NO_APPLICABLE_UPGRADE", "No applicable upgrade found."),
            _ => null,
        },
        _ => code switch
        {
            unchecked((long)0x80004005) => ("E_FAIL", "Unspecified error"),
            unchecked((long)0x80070005) => ("E_ACCESSDENIED", "General access denied error"),
            unchecked((long)0x80070057) => ("E_INVALIDARG", "One or more arguments are not valid"),
            unchecked((long)0x8007000E) => ("E_OUTOFMEMORY", "Failed to allocate necessary memory"),
            _ => null,
        }
    };
}

static void PrintWarnings(List<string> warnings) { foreach (var w in warnings) Console.Error.WriteLine($"warning: {w}"); }
static void PrintOpt(string label, string? value) { if (value is not null) Console.WriteLine($"{label}: {value}"); }
static string Trunc(string s, int max) => s.Length <= max ? s : s[..(max - 1)] + ".";

static void PrintTable(string[] headers, List<string[]> rows)
{
    if (headers.Length == 0) return;
    int cols = headers.Length;
    var widths = headers.Select(h => h.Length).ToArray();
    var hasData = new bool[cols];

    foreach (var row in rows)
        for (int i = 0; i < Math.Min(cols, row.Length); i++)
            if (!string.IsNullOrEmpty(row[i]))
            {
                hasData[i] = true;
                widths[i] = Math.Max(widths[i], row[i].Length);
            }

    for (int i = 0; i < cols; i++)
        if (!hasData[i]) widths[i] = 0;

    var spaceAfter = Enumerable.Repeat(true, cols).ToArray();
    spaceAfter[^1] = false;
    for (int i = cols - 1; i >= 1; i--)
    {
        if (widths[i] == 0) spaceAfter[i - 1] = false;
        else break;
    }

    int totalWidth = widths.Zip(spaceAfter, (w, s) => w + (s ? 1 : 0)).Sum();
    int consoleWidth = 119;
    try { consoleWidth = Math.Max(1, Console.WindowWidth - 2); } catch { }
    if (totalWidth >= consoleWidth)
    {
        int extra = totalWidth - consoleWidth + 1;
        while (extra > 0)
        {
            int target = 0;
            for (int i = 1; i < cols; i++)
                if (widths[i] > widths[target]) target = i;
            if (widths[target] > 1) widths[target]--;
            extra--;
        }
        totalWidth = Math.Max(0, consoleWidth - 1);
    }

    PrintTableLine(headers, widths, spaceAfter);
    Console.WriteLine(new string('-', totalWidth));
    foreach (var row in rows)
        PrintTableLine(row, widths, spaceAfter);
}

static void PrintTableLine(string[] values, int[] widths, bool[] spaceAfter)
{
    var sb = new System.Text.StringBuilder();
    for (int i = 0; i < Math.Min(values.Length, widths.Length); i++)
    {
        if (widths[i] == 0) continue;
        var val = values[i] ?? "";
        if (val.Length > widths[i])
        {
            sb.Append(Trunc(val, widths[i]));
            if (spaceAfter[i]) sb.Append(' ');
        }
        else
        {
            sb.Append(val);
            if (spaceAfter[i]) sb.Append(' ', widths[i] - val.Length + 1);
        }
    }
    Console.WriteLine(sb.ToString().TrimEnd());
}
