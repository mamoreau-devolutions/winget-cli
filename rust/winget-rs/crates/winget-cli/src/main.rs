use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};
use winget_core::{
    CacheWarmResult, Documentation, ListQuery, ListResponse, PackageQuery, Repository,
    SearchResponse, ShowResult, SourceRecord, SourceUpdateResult, VersionsResult,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "winget", about = "Pure Rust subset of the winget CLI", version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(long = "info", global = true)]
    info: bool,
    #[arg(long = "output", short = 'o', global = true, value_parser = ["json", "text"])]
    output: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "ls")]
    List(ListArgs),
    Show(ShowArgs),
    Search(SearchArgs),
    #[command(alias = "update")]
    Upgrade(UpgradeArgs),
    Source {
        #[command(subcommand)]
        command: SourceCommands,
    },
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },
    Hash(HashArgs),
    Export(ExportArgs),
    #[command(name = "error")]
    ErrorLookup(ErrorArgs),
    Settings {
        #[command(subcommand)]
        command: SettingsCommands,
    },
    Features,
    Validate(ValidateArgs),
    #[command(alias = "dl")]
    Download(DownloadArgs),
    Pin {
        #[command(subcommand)]
        command: PinCommands,
    },
}

#[derive(Subcommand)]
enum SourceCommands {
    List,
    Update { source: Option<String> },
    Export,
}

#[derive(Subcommand)]
enum SettingsCommands {
    Export,
}

#[derive(Subcommand)]
enum CacheCommands {
    Warm(QueryArgs),
}

#[derive(Args)]
struct ShowArgs {
    #[command(flatten)]
    query: QueryArgs,
    #[arg(long = "versions")]
    versions: bool,
}

#[derive(Args)]
struct UpgradeArgs {
    #[arg(conflicts_with = "query_option")]
    query: Option<String>,
    #[arg(
        long = "query",
        short = 'q',
        value_name = "QUERY",
        conflicts_with = "query"
    )]
    query_option: Option<String>,
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    moniker: Option<String>,
    #[arg(long, short = 's')]
    source: Option<String>,
    #[arg(long, short = 'n')]
    count: Option<usize>,
    #[arg(long, short = 'e')]
    exact: bool,
    #[arg(long = "include-unknown", short = 'u', visible_alias = "unknown")]
    include_unknown: bool,
    #[arg(long = "include-pinned", visible_alias = "pinned")]
    include_pinned: bool,
}

#[derive(Args)]
struct HashArgs {
    file: String,
    #[arg(long)]
    msix: bool,
}

#[derive(Args)]
struct ExportArgs {
    #[arg(long, short = 'o')]
    output: String,
    #[arg(long, short = 's')]
    source: Option<String>,
    #[arg(long = "include-versions")]
    include_versions: bool,
}

#[derive(Args)]
struct ErrorArgs {
    input: String,
}

#[derive(Args)]
struct ValidateArgs {
    manifest: String,
    #[arg(long = "ignore-warnings")]
    ignore_warnings: bool,
}

#[derive(Args, Clone)]
struct DownloadArgs {
    #[command(flatten)]
    query: QueryArgs,
    #[arg(short = 'd', long = "download-directory")]
    download_directory: Option<String>,
}

#[derive(Subcommand)]
enum PinCommands {
    List,
}

#[derive(Args, Clone)]
struct QueryArgs {
    #[arg(conflicts_with = "query_option")]
    query: Option<String>,
    #[arg(
        long = "query",
        short = 'q',
        value_name = "QUERY",
        conflicts_with = "query"
    )]
    query_option: Option<String>,
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    moniker: Option<String>,
    #[arg(long, short = 's')]
    source: Option<String>,
    #[arg(long, short = 'e')]
    exact: bool,
    #[arg(long, short = 'v')]
    version: Option<String>,
    #[arg(long, short = 'c')]
    channel: Option<String>,
    #[arg(long)]
    locale: Option<String>,
    #[arg(long = "installer-type")]
    installer_type: Option<String>,
    #[arg(long = "architecture", short = 'a')]
    installer_architecture: Option<String>,
    #[arg(long = "scope")]
    install_scope: Option<String>,
}

#[derive(Args, Clone)]
struct SearchArgs {
    #[arg(conflicts_with = "query_option")]
    query: Option<String>,
    #[arg(
        long = "query",
        short = 'q',
        value_name = "QUERY",
        conflicts_with = "query"
    )]
    query_option: Option<String>,
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    moniker: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long, visible_alias = "cmd")]
    command: Option<String>,
    #[arg(long, short = 's')]
    source: Option<String>,
    #[arg(long, short = 'n')]
    count: Option<usize>,
    #[arg(long, short = 'e')]
    exact: bool,
    #[arg(long = "versions")]
    versions: bool,
}

#[derive(Args, Clone)]
struct ListArgs {
    #[arg(conflicts_with = "query_option")]
    query: Option<String>,
    #[arg(
        long = "query",
        short = 'q',
        value_name = "QUERY",
        conflicts_with = "query"
    )]
    query_option: Option<String>,
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    moniker: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long, visible_alias = "cmd")]
    command: Option<String>,
    #[arg(long, short = 's')]
    source: Option<String>,
    #[arg(long, short = 'n')]
    count: Option<usize>,
    #[arg(long, short = 'e')]
    exact: bool,
    #[arg(long = "scope")]
    install_scope: Option<String>,
    #[arg(long = "upgrade-available")]
    upgrade: bool,
    #[arg(long = "include-unknown", short = 'u', visible_alias = "unknown")]
    include_unknown: bool,
    #[arg(long = "include-pinned", visible_alias = "pinned")]
    include_pinned: bool,
    #[arg(long = "details")]
    details: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let json = cli.output.as_deref() == Some("json");

    if cli.info {
        print_info();
        return Ok(());
    }

    let command = match cli.command {
        Some(command) => command,
        None => {
            Cli::parse_from(["winget", "--help"]);
            return Ok(());
        }
    };

    match command {
        Commands::List(args) => {
            let mut repository = Repository::open()?;
            let details = args.details;
            let upgrade = args.upgrade;
            let result = repository.list(&args.clone().into())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_list_result(result, details, upgrade);
            }
        }
        Commands::Show(args) => {
            let mut repository = Repository::open()?;
            if args.versions {
                let result = repository.show_versions(&args.query.into())?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    print_versions(result);
                }
            } else {
                let result = repository.show(&args.query.into())?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    print_show(result);
                }
            }
        }
        Commands::Search(args) => {
            let mut repository = Repository::open()?;
            if args.versions {
                let result = repository.search_versions(&args.clone().into())?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    print_versions(result);
                }
            } else {
                let result = repository.search(&args.into())?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    print_search(result);
                }
            }
        }
        Commands::Upgrade(args) => {
            let mut repository = Repository::open()?;
            let list_query = ListQuery::from(args);
            let result = repository.list(&list_query)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_list_result(result, false, true);
            }
        }
        Commands::Source { command } => {
            let repository = Repository::open()?;
            match command {
                SourceCommands::List => print_sources(repository.list_sources()),
                SourceCommands::Update { source } => {
                    let mut repository = repository;
                    print_source_updates(repository.update_sources(source.as_deref())?)
                }
                SourceCommands::Export => print_source_export(&repository),
            }
        }
        Commands::Cache { command } => {
            let mut repository = Repository::open()?;
            match command {
                CacheCommands::Warm(args) => print_cache_warm(repository.warm_cache(&args.into())?),
            }
        }
        Commands::Hash(args) => {
            print_hash(&args.file, args.msix)?;
        }
        Commands::Export(args) => {
            let mut repository = Repository::open()?;
            do_export(&mut repository, &args)?;
        }
        Commands::ErrorLookup(args) => {
            print_error_lookup(&args.input);
        }
        Commands::Settings { command } => match command {
            SettingsCommands::Export => print_settings_export(),
        },
        Commands::Features => {
            print_features();
        }
        Commands::Validate(args) => {
            print_validate(&args.manifest, args.ignore_warnings)?;
        }
        Commands::Download(args) => {
            let mut repository = Repository::open()?;
            let result = repository.show(&args.query.into())?;
            do_download(&result, args.download_directory.as_deref())?;
        }
        Commands::Pin { command } => match command {
            PinCommands::List => {
                print_pin_list();
            }
        },
    }

    Ok(())
}

impl From<QueryArgs> for PackageQuery {
    fn from(value: QueryArgs) -> Self {
        Self {
            query: value.query.or(value.query_option),
            id: value.id,
            name: value.name,
            moniker: value.moniker,
            tag: None,
            command: None,
            source: value.source,
            count: None,
            exact: value.exact,
            version: value.version,
            channel: value.channel,
            locale: value.locale,
            installer_type: value.installer_type,
            installer_architecture: value.installer_architecture,
            install_scope: value.install_scope,
        }
    }
}

impl From<SearchArgs> for PackageQuery {
    fn from(value: SearchArgs) -> Self {
        Self {
            query: value.query.or(value.query_option),
            id: value.id,
            name: value.name,
            moniker: value.moniker,
            tag: value.tag,
            command: value.command,
            source: value.source,
            count: value.count,
            exact: value.exact,
            version: None,
            channel: None,
            locale: None,
            installer_type: None,
            installer_architecture: None,
            install_scope: None,
        }
    }
}

impl From<ListArgs> for ListQuery {
    fn from(value: ListArgs) -> Self {
        Self {
            query: value.query.or(value.query_option),
            id: value.id,
            name: value.name,
            moniker: value.moniker,
            tag: value.tag,
            command: value.command,
            source: value.source,
            count: value.count,
            exact: value.exact,
            install_scope: value.install_scope,
            upgrade_only: value.upgrade,
            include_unknown: value.include_unknown,
            include_pinned: value.include_pinned,
        }
    }
}

impl From<UpgradeArgs> for ListQuery {
    fn from(value: UpgradeArgs) -> Self {
        Self {
            query: value.query.or(value.query_option),
            id: value.id,
            name: value.name,
            moniker: value.moniker,
            tag: None,
            command: None,
            source: value.source,
            count: value.count,
            exact: value.exact,
            install_scope: None,
            upgrade_only: true,
            include_unknown: value.include_unknown,
            include_pinned: value.include_pinned,
        }
    }
}

fn print_sources(sources: Vec<SourceRecord>) {
    println!(
        "{:<12} {:<60} {}",
        "Name", "Argument", "Explicit"
    );
    for source in sources {
        println!(
            "{:<12} {:<60} {}",
            source.name, source.arg, "false"
        );
    }
}

fn print_source_updates(results: Vec<SourceUpdateResult>) {
    for result in results {
        println!("{} [{}]: {}", result.name, result.kind, result.detail);
    }
}

fn print_search(result: SearchResponse) {
    print_warnings(&result.warnings);
    if result.matches.is_empty() {
        println!("No package matched the supplied query.");
        return;
    }

    let show_match_column = result
        .matches
        .iter()
        .any(|item| item.match_criteria.as_deref().is_some());
    if show_match_column {
        println!(
            "{:<32} {:<40} {:<18} {:<24} Source",
            "Name", "Id", "Version", "Match"
        );
        for item in result.matches {
            println!(
                "{:<32} {:<40} {:<18} {:<24} {}",
                truncate(&item.name, 32),
                truncate(&item.id, 40),
                item.version.unwrap_or_default(),
                truncate(item.match_criteria.as_deref().unwrap_or_default(), 24),
                item.source_name
            );
        }
    } else {
        println!("{:<36} {:<42} {:<18} Source", "Name", "Id", "Version");
        for item in result.matches {
            println!(
                "{:<36} {:<42} {:<18} {}",
                truncate(&item.name, 36),
                truncate(&item.id, 42),
                item.version.unwrap_or_default(),
                item.source_name
            );
        }
    }

    if result.truncated {
        println!("<additional entries truncated due to result limit>");
    }
}

fn print_list_result(result: ListResponse, details: bool, upgrade_only: bool) {
    let ListResponse {
        matches,
        warnings,
        truncated,
    } = result;

    print_warnings(&warnings);
    if matches.is_empty() {
        println!("No installed package found matching input criteria.");
        return;
    }

    let match_count = matches.len();

    if details {
        let total = matches.len();
        for (index, item) in matches.iter().enumerate() {
            if total > 1 {
                println!("({}/{}) {} [{}]", index + 1, total, item.name, item.id);
            } else {
                println!("{} [{}]", item.name, item.id);
            }
            print_field("Version", &item.installed_version);
            if let Some(value) = &item.publisher {
                print_field("Publisher", value);
            }
            if item.local_id != item.id {
                print_field("Local Identifier", &item.local_id);
            }
            if !item.package_family_names.is_empty() {
                print_field("Package Family Name", &item.package_family_names.join(", "));
            }
            if !item.product_codes.is_empty() {
                print_field("Product Code", &item.product_codes.join(", "));
            }
            if !item.upgrade_codes.is_empty() {
                print_field("Upgrade Code", &item.upgrade_codes.join(", "));
            }
            if let Some(value) = &item.installer_category {
                print_field("Installer Category", value);
            }
            if let Some(value) = &item.scope {
                print_field("Installed Scope", value);
            }
            if let Some(value) = &item.install_location {
                print_field("Installed Location", value);
            }
            if let Some(value) = &item.source_name {
                print_field("Source", value);
            }
            if let Some(value) = &item.available_version {
                print_field("Available", value);
            }
        }
    } else {
        let show_available = matches.iter().any(|item| {
            item.available_version
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        });
        if show_available {
            let rows = matches
                .into_iter()
                .map(|item| {
                    vec![
                        item.name,
                        item.id,
                        item.installed_version,
                        item.available_version.unwrap_or_default(),
                        item.source_name.unwrap_or_default(),
                    ]
                })
                .collect::<Vec<_>>();
            print_table(&["Name", "Id", "Version", "Available", "Source"], &rows);
        } else {
            let rows = matches
                .into_iter()
                .map(|item| {
                    vec![
                        item.name,
                        item.id,
                        item.installed_version,
                        item.source_name.unwrap_or_default(),
                    ]
                })
                .collect::<Vec<_>>();
            print_table(&["Name", "Id", "Version", "Source"], &rows);
        }
    }

    if truncated {
        println!("<additional entries truncated due to result limit>");
    }
    if upgrade_only {
        println!("{} upgrades available.", match_count);
    }
}

fn print_versions(result: VersionsResult) {
    print_warnings(&result.warnings);
    println!("Found {} [{}]", result.package.name, result.package.id);
    let show_channel = result
        .versions
        .iter()
        .any(|version| !version.channel.is_empty());
    if show_channel {
        println!("{:<20} Channel", "Version");
        for version in result.versions {
            println!("{:<20} {}", version.version, version.channel);
        }
    } else {
        println!("Version");
        println!("-------");
        for version in result.versions {
            println!("{}", version.version);
        }
    }
}

fn print_show(result: ShowResult) {
    print_warnings(&result.warnings);
    println!("Found {} [{}]", result.package.name, result.package.id);
    print_field("Version", &result.manifest.version);
    if !result.manifest.channel.is_empty() {
        print_field("Channel", &result.manifest.channel);
    }
    if let Some(value) = &result.manifest.publisher {
        print_field("Publisher", value);
    }
    if let Some(value) = &result.manifest.publisher_url {
        print_field("Publisher Url", value);
    }
    if let Some(value) = &result.manifest.publisher_support_url {
        print_field("Publisher Support Url", value);
    }
    if let Some(value) = &result.manifest.author {
        print_field("Author", value);
    }
    if let Some(value) = &result.manifest.moniker {
        print_field("Moniker", value);
    }
    if let Some(value) = &result.manifest.description {
        print_multiline_with_colon("Description", value);
    }
    if let Some(value) = &result.manifest.package_url {
        print_field("Homepage", value);
    }
    if let Some(value) = &result.manifest.license {
        print_field("License", value);
    }
    if let Some(value) = &result.manifest.license_url {
        print_field("License Url", value);
    }
    if let Some(value) = &result.manifest.privacy_url {
        print_field("Privacy Url", value);
    }
    if let Some(value) = &result.manifest.copyright {
        print_field("Copyright", value);
    }
    if let Some(value) = &result.manifest.copyright_url {
        print_field("Copyright Url", value);
    }
    if let Some(value) = &result.manifest.release_notes {
        print_multiline_with_colon("Release Notes", value);
    }
    if let Some(value) = &result.manifest.release_notes_url {
        print_field("Release Notes Url", value);
    }
    if !result.manifest.package_dependencies.is_empty() {
        print_field(
            "Dependencies",
            &result.manifest.package_dependencies.join(", "),
        );
    }
    print_documentation(&result.manifest.documentation);
    if !result.manifest.tags.is_empty() {
        print_list("Tags", &result.manifest.tags);
    }

    println!("Installer:");
    if let Some(installer) = result.selected_installer.as_ref() {
        if let Some(value) = &installer.installer_type {
            print_indented_field("Type", value);
        }
        if let Some(value) = &installer.architecture {
            print_indented_field("Architecture", value);
        }
        if let Some(value) = &installer.locale {
            print_indented_field("Locale", value);
        }
        if let Some(value) = &installer.scope {
            print_indented_field("Scope", value);
        }
        if let Some(value) = &installer.url {
            print_indented_field("Url", value);
        }
        if let Some(value) = &installer.sha256 {
            print_indented_field("Sha256", value);
        }
        if let Some(value) = &installer.product_code {
            print_indented_field("ProductCode", value);
        }
        if let Some(value) = &installer.package_family_name {
            print_indented_field("PackageFamilyName", value);
        }
        if let Some(value) = &installer.upgrade_code {
            print_indented_field("UpgradeCode", value);
        }
        if let Some(value) = &installer.release_date {
            print_indented_field("ReleaseDate", value);
        }
        if !installer.commands.is_empty() {
            print_indented_field("Commands", &installer.commands.join(", "));
        }
        if !installer.package_dependencies.is_empty() {
            print_indented_field("Dependencies", &installer.package_dependencies.join(", "));
        }
    } else if !result.manifest.installers.is_empty() {
        println!("  No applicable installer found; see logs for more details.");
    }
}

fn print_cache_warm(result: CacheWarmResult) {
    print_warnings(&result.warnings);
    let version = result.package.version.unwrap_or_default();
    println!(
        "Warmed cache for {} [{}] {}",
        result.package.name, result.package.id, version
    );
    for path in result.cached_files {
        println!("  {}", path.display());
    }
}

fn print_warnings(warnings: &[String]) {
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
}

fn print_field(label: &str, value: &str) {
    println!("{label}: {value}");
}

fn print_indented_field(label: &str, value: &str) {
    println!("  {label}: {value}");
}

fn print_multiline_with_colon(label: &str, value: &str) {
    println!("{label}:");
    for line in value.lines() {
        println!("  {line}");
    }
}

fn print_list(label: &str, values: &[String]) {
    println!("{label}:");
    for value in values {
        println!("  {value}");
    }
}

fn print_documentation(entries: &[Documentation]) {
    if entries.is_empty() {
        return;
    }

    println!("Documentation:");
    for entry in entries {
        match entry.label.as_deref() {
            Some(label) if !label.is_empty() => println!("  {label}: {}", entry.url),
            _ => println!("  {}", entry.url),
        }
    }
}

fn print_info() {
    println!("winget-rs v{VERSION}");
    println!("Pure Rust subset of the Windows Package Manager CLI");
    println!();

    #[cfg(windows)]
    {
        use std::env;
        let os_version = get_os_version();
        let arch = env::var("PROCESSOR_ARCHITECTURE").unwrap_or_else(|_| "Unknown".to_string());
        println!("Windows: Windows.Desktop v{os_version}");
        println!("System Architecture: {arch}");
        println!();

        println!("WinGet Directories");
        println!("{}", "-".repeat(80));
        let local_app_data = env::var("LOCALAPPDATA").unwrap_or_default();
        let user_profile = env::var("USERPROFILE").unwrap_or_default();
        let source_cache = format!(
            "{}\\Packages\\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\\LocalState\\Microsoft\\Windows Package Manager",
            local_app_data
        );
        let settings_path = format!(
            "{}\\Packages\\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\\LocalState\\settings.json",
            local_app_data
        );
        let portable_links_user = format!("{}\\Microsoft\\WinGet\\Links", local_app_data);
        let downloads = format!("{}\\Downloads", user_profile);

        println!("{:<40} {}", "Source Cache", source_cache);
        println!("{:<40} {}", "User Settings", settings_path);
        println!(
            "{:<40} {}",
            "Portable Links Directory (User)", portable_links_user
        );
        println!(
            "{:<40} {}",
            "Portable Links Directory (Machine)", "C:\\Program Files\\WinGet\\Links"
        );
        println!(
            "{:<40} {}",
            "Portable Package Root (User)",
            format!("{}\\Microsoft\\WinGet\\Packages", local_app_data)
        );
        println!(
            "{:<40} {}",
            "Portable Package Root", "C:\\Program Files\\WinGet\\Packages"
        );
        println!(
            "{:<40} {}",
            "Portable Package Root (x86)", "C:\\Program Files (x86)\\WinGet\\Packages"
        );
        println!("{:<40} {}", "Installer Downloads", downloads);
    }
    #[cfg(not(windows))]
    {
        println!("Platform: {}", std::env::consts::OS);
        println!("Architecture: {}", std::env::consts::ARCH);
    }

    println!();
    println!("Links");
    println!("{}", "-".repeat(80));
    println!("{:<20} {}", "Homepage", "https://aka.ms/winget");
    println!(
        "{:<20} {}",
        "Privacy Statement", "https://aka.ms/winget-privacy"
    );
    println!(
        "{:<20} {}",
        "License Agreement", "https://aka.ms/winget-license"
    );
}

#[cfg(windows)]
fn get_os_version() -> String {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion") {
        let build: String = key.get_value("CurrentBuildNumber").unwrap_or_default();
        let ubr: u32 = key.get_value("UBR").unwrap_or(0);
        let major: u32 = key.get_value("CurrentMajorVersionNumber").unwrap_or(10);
        let minor: u32 = key.get_value("CurrentMinorVersionNumber").unwrap_or(0);
        format!("{major}.{minor}.{build}.{ubr}")
    } else {
        "Unknown".to_string()
    }
}

fn print_hash(file_path: &str, _msix: bool) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::fs;
    let data = fs::read(file_path)
        .map_err(|e| anyhow::anyhow!("failed to read file '{}': {}", file_path, e))?;
    let hash = Sha256::digest(&data);
    let hex = hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!("SHA256: {hex}");
    Ok(())
}

fn do_export(repository: &mut Repository, args: &ExportArgs) -> Result<()> {
    let list_query = ListQuery {
        query: None,
        id: None,
        name: None,
        moniker: None,
        tag: None,
        command: None,
        source: args.source.clone(),
        count: None,
        exact: false,
        install_scope: None,
        upgrade_only: false,
        include_unknown: false,
        include_pinned: false,
    };
    let result = repository.list(&list_query)?;

    let packages: Vec<serde_json::Value> = result
        .matches
        .iter()
        .filter(|m| m.source_name.is_some())
        .map(|m| {
            let mut obj = serde_json::json!({
                "PackageIdentifier": m.id,
            });
            if args.include_versions {
                obj["Version"] = serde_json::Value::String(m.installed_version.clone());
            }
            if let Some(source) = &m.source_name {
                obj["SourceDetails"] = serde_json::json!({
                    "Name": source,
                    "Type": "Microsoft.PreIndexed.Package",
                    "Argument": ""
                });
            }
            obj
        })
        .collect();

    let export = serde_json::json!({
        "$schema": "https://aka.ms/winget-packages.schema.2.0.json",
        "CreationDate": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string(),
        "Sources": [{
            "SourceDetails": {
                "Name": "winget",
                "Type": "Microsoft.PreIndexed.Package",
                "Argument": "https://cdn.winget.microsoft.com/cache"
            },
            "Packages": packages
        }],
        "WinGetVersion": VERSION,
    });

    let json = serde_json::to_string_pretty(&export)?;
    std::fs::write(&args.output, &json)
        .map_err(|e| anyhow::anyhow!("failed to write export file '{}': {}", args.output, e))?;
    println!("Exported {} packages to {}", packages.len(), args.output);
    Ok(())
}

fn print_error_lookup(input: &str) {
    let code = if input.starts_with("0x") || input.starts_with("0X") {
        u32::from_str_radix(&input[2..], 16).ok()
    } else {
        input
            .parse::<u32>()
            .ok()
            .or_else(|| input.parse::<i32>().ok().map(|v| v as u32))
    };

    let code = match code {
        Some(c) => c,
        None => {
            println!("Could not parse input '{input}' as an error code.");
            return;
        }
    };

    if let Some((symbol, description)) = lookup_hresult(code) {
        // Upstream only shows the symbol for APPINSTALLER codes (0x8A15xxxx)
        if code & 0xFFFF0000 == 0x8A150000 {
            println!("0x{code:08x} : {symbol}");
        } else {
            println!("0x{code:08x}");
        }
        println!("{description}");
    } else {
        println!("0x{code:08x}");
        println!("Unknown error code");
    }
}

fn lookup_hresult(code: u32) -> Option<(&'static str, &'static str)> {
    match code {
        0x00000000 => Some(("S_OK", "Operation successful")),
        0x80004001 => Some(("E_NOTIMPL", "Not implemented")),
        0x80004002 => Some(("E_NOINTERFACE", "No such interface supported")),
        0x80004003 => Some(("E_POINTER", "Invalid pointer")),
        0x80004004 => Some(("E_ABORT", "Operation aborted")),
        0x80004005 => Some(("E_FAIL", "Unspecified error")),
        0x80070002 => Some(("E_FILENOTFOUND", "The system cannot find the file specified")),
        0x80070005 => Some(("E_ACCESSDENIED", "General access denied error")),
        0x80070057 => Some(("E_INVALIDARG", "One or more arguments are invalid")),
        0x8007000E => Some(("E_OUTOFMEMORY", "Ran out of memory")),
        // winget-specific HRESULT codes
        0x8A150001 => Some(("APPINSTALLER_CLI_ERROR_INTERNAL_ERROR", "Internal error")),
        0x8A150002 => Some(("APPINSTALLER_CLI_ERROR_INVALID_CL_ARGUMENTS", "Invalid command line arguments")),
        0x8A150003 => Some(("APPINSTALLER_CLI_ERROR_COMMAND_FAILED", "Command failed")),
        0x8A150004 => Some(("APPINSTALLER_CLI_ERROR_MANIFEST_FAILED", "Opening manifest failed")),
        0x8A150005 => Some(("APPINSTALLER_CLI_ERROR_BLOCKED_BY_POLICY", "Operation is blocked by policy")),
        0x8A150006 => Some(("APPINSTALLER_CLI_ERROR_SHELLEXEC_INSTALL_FAILED", "ShellExecute install failed")),
        0x8A150007 => Some(("APPINSTALLER_CLI_ERROR_UNSUPPORTED_MANIFESTVERSION", "Unsupported manifest version")),
        0x8A150008 => Some(("APPINSTALLER_CLI_ERROR_DOWNLOAD_FAILED", "Download of installer failed")),
        0x8A150009 => Some(("APPINSTALLER_CLI_ERROR_CANNOT_WRITE_TO_UPLEVEL_INDEX", "Cannot write to the package index")),
        0x8A15000A => Some(("APPINSTALLER_CLI_ERROR_INDEX_INTEGRITY_COMPROMISED", "Index integrity compromised")),
        0x8A15000B => Some(("APPINSTALLER_CLI_ERROR_SOURCES_INVALID", "Sources are invalid")),
        0x8A15000C => Some(("APPINSTALLER_CLI_ERROR_SOURCE_NAME_ALREADY_EXISTS", "Source name already exists")),
        0x8A15000D => Some(("APPINSTALLER_CLI_ERROR_INVALID_SOURCE_TYPE", "Invalid source type")),
        0x8A15000E => Some(("APPINSTALLER_CLI_ERROR_PACKAGE_IS_BUNDLE", "Package is a bundle")),
        0x8A15000F => Some(("APPINSTALLER_CLI_ERROR_SOURCE_DATA_MISSING", "Source data is missing")),
        0x8A150010 => Some(("APPINSTALLER_CLI_ERROR_NO_APPLICABLE_INSTALLER", "None of the installers are applicable for the current system")),
        0x8A150011 => Some(("APPINSTALLER_CLI_ERROR_INSTALLER_HASH_MISMATCH", "Installer hash does not match")),
        0x8A150012 => Some(("APPINSTALLER_CLI_ERROR_SOURCE_NAME_DOES_NOT_EXIST", "Source name does not exist")),
        0x8A150013 => Some(("APPINSTALLER_CLI_ERROR_SOURCE_ARG_ALREADY_EXISTS", "Source argument already exists")),
        0x8A150014 => Some(("APPINSTALLER_CLI_ERROR_NO_APPLICATIONS_FOUND", "No applications found")),
        0x8A150015 => Some(("APPINSTALLER_CLI_ERROR_NO_SOURCES_DEFINED", "No sources defined")),
        0x8A150016 => Some(("APPINSTALLER_CLI_ERROR_MULTIPLE_APPLICATIONS_FOUND", "Multiple applications found")),
        0x8A150017 => Some(("APPINSTALLER_CLI_ERROR_NO_MANIFEST_FOUND", "No manifest found matching input criteria")),
        0x8A150019 => Some(("APPINSTALLER_CLI_ERROR_NO_RANGES_PROCESSED", "No ranges processed")),
        0x8A15001A => Some(("APPINSTALLER_CLI_ERROR_EXPERIMENTAL_FEATURE_DISABLED", "This feature is disabled by Group Policy")),
        0x8A15001B => Some(("APPINSTALLER_CLI_ERROR_MSSTORE_BLOCKED_BY_POLICY", "This feature is blocked by Group Policy")),
        0x8A15001C => Some(("APPINSTALLER_CLI_ERROR_MSSTORE_APP_BLOCKED_BY_POLICY", "This Microsoft Store app is blocked by Group Policy")),
        0x8A150022 => Some(("APPINSTALLER_CLI_ERROR_UPDATE_NOT_APPLICABLE", "Upgrade version is not newer than installed version")),
        0x8A150023 => Some(("APPINSTALLER_CLI_ERROR_UPDATE_ALL_HAS_FAILURE", "At least one package had a failure during upgrade --all")),
        0x8A150024 => Some(("APPINSTALLER_CLI_ERROR_INSTALLER_SECURITY_CHECK_FAILED", "Installer failed security check")),
        0x8A15002B => Some(("APPINSTALLER_CLI_ERROR_PACKAGE_ALREADY_INSTALLED", "The package is already installed")),
        0x8A150038 => Some(("APPINSTALLER_CLI_ERROR_PINNED_CERTIFICATE_MISMATCH", "Certificate pinning mismatch")),
        _ => None,
    }
}

fn print_settings_export() {
    #[cfg(windows)]
    {
        use std::env;
        let local_app_data = env::var("LOCALAPPDATA").unwrap_or_default();
        let settings_path = format!(
            "{}\\Packages\\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\\LocalState\\settings.json",
            local_app_data
        );

        // Read admin settings from registry
        let mut admin_settings = serde_json::Map::new();
        {
            use winreg::RegKey;
            use winreg::enums::HKEY_LOCAL_MACHINE;
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            if let Ok(key) =
                hklm.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppInstaller")
            {
                let setting_names = [
                    "LocalManifestFiles",
                    "BypassCertificatePinningForMicrosoftStore",
                    "InstallerHashOverride",
                    "LocalArchiveMalwareScanOverride",
                    "ProxyCommandLineOptions",
                    "DefaultProxy",
                ];
                for name in setting_names {
                    let enabled: bool = key.get_value::<u32, _>(name).unwrap_or(0) != 0;
                    admin_settings.insert(name.to_string(), serde_json::Value::Bool(enabled));
                }
            }
        }

        let export = serde_json::json!({
            "$schema": "https://aka.ms/winget-settings-export.schema.json",
            "adminSettings": admin_settings,
            "userSettingsFile": settings_path,
        });

        println!("{}", serde_json::to_string_pretty(&export).unwrap());
    }
    #[cfg(not(windows))]
    {
        println!("{{}}");
    }
}

fn print_features() {
    println!("The following experimental features are in progress.");
    println!("They can be configured through the settings file 'winget settings'.");
    println!();

    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_LOCAL_MACHINE;

        let features = [
            (
                "Configuration (configure)",
                "configuration",
                "https://aka.ms/winget-settings",
            ),
            ("Direct MSI", "directMSI", "https://aka.ms/winget-settings"),
            (
                "Windows Feature Dependencies",
                "windowsFeature",
                "https://aka.ms/winget-settings",
            ),
            ("Resume", "resume", "https://aka.ms/winget-settings"),
            ("Repair", "repair", "https://aka.ms/winget-settings"),
            (
                "Side-by-side installation",
                "sideBySide",
                "https://aka.ms/winget-settings",
            ),
            ("Pinning", "pinning", "https://aka.ms/winget-settings"),
        ];

        // Try reading user settings to see which features are enabled
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let settings_path = format!(
            "{}\\Packages\\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\\LocalState\\settings.json",
            local_app_data
        );
        let user_settings: serde_json::Value = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::Value::Null);

        let experimental_features = user_settings
            .get("experimentalFeatures")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        // Check group policy
        let gp_disabled: bool = {
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            hklm.open_subkey("SOFTWARE\\Policies\\Microsoft\\Windows\\AppInstaller")
                .and_then(|key| key.get_value::<u32, _>("EnableExperimentalFeatures"))
                .map(|v| v == 0)
                .unwrap_or(false)
        };

        println!(
            "{:<40} {:<10} {:<30} {}",
            "Feature", "Status", "Property", "Link"
        );
        println!("{}", "-".repeat(100));
        for (display_name, property, link) in features {
            let enabled = if gp_disabled {
                false
            } else {
                experimental_features
                    .get(property)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            };
            let status = if enabled { "Enabled" } else { "Disabled" };
            println!("{display_name:<40} {status:<10} {property:<30} {link}");
        }
    }
    #[cfg(not(windows))]
    {
        println!("Feature listing is only available on Windows.");
    }
}

fn print_source_export(repository: &Repository) {
    let sources = repository.list_sources();
    let source_array: Vec<serde_json::Value> = sources
        .iter()
        .map(|s| {
            serde_json::json!({
                "Name": s.name,
                "Type": s.kind.to_string(),
                "Arg": s.arg,
                "Data": s.identifier,
                "Identifier": s.identifier,
                "TrustLevel": "Default"
            })
        })
        .collect();
    let export = serde_json::json!({
        "Sources": source_array,
    });
    println!("{}", serde_json::to_string_pretty(&export).unwrap());
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if headers.is_empty() {
        return;
    }

    let mut widths = headers
        .iter()
        .map(|header| display_width(header))
        .collect::<Vec<_>>();
    let mut has_data = vec![false; headers.len()];

    for row in rows {
        for (index, value) in row.iter().enumerate() {
            if !value.is_empty() {
                has_data[index] = true;
                widths[index] = widths[index].max(display_width(value));
            }
        }
    }

    for (index, width) in widths.iter_mut().enumerate() {
        if !has_data[index] {
            *width = 0;
        }
    }

    let mut space_after = vec![true; headers.len()];
    if let Some(last) = space_after.last_mut() {
        *last = false;
    }
    for index in (1..headers.len()).rev() {
        if widths[index] == 0 {
            space_after[index - 1] = false;
        } else {
            break;
        }
    }

    let mut total_required = table_total_width(&widths, &space_after);
    let console_width = get_console_width();
    if total_required >= console_width {
        let mut extra = (total_required - console_width) + 1;
        while extra > 0 {
            let mut target_index = 0;
            let mut target_width = widths[0];
            for (index, width) in widths.iter().copied().enumerate().skip(1) {
                if width > target_width {
                    target_index = index;
                    target_width = width;
                }
            }

            if widths[target_index] > 1 {
                widths[target_index] -= 1;
            }
            extra -= 1;
        }

        total_required = console_width.saturating_sub(1);
    }

    let header_row = headers
        .iter()
        .map(|header| header.to_string())
        .collect::<Vec<_>>();
    print_table_line(&header_row, &widths, &space_after);
    println!("{}", "-".repeat(total_required));
    for row in rows {
        print_table_line(row, &widths, &space_after);
    }
}

fn print_table_line(values: &[String], widths: &[usize], space_after: &[bool]) {
    let mut line = String::new();

    for (index, value) in values.iter().enumerate() {
        let width = widths[index];
        if width == 0 {
            continue;
        }

        let value_width = display_width(value);
        if value_width > width {
            line.push_str(&truncate(value, width));
            if space_after[index] {
                line.push(' ');
            }
        } else {
            line.push_str(value);
            if space_after[index] {
                line.push_str(&" ".repeat(width - value_width + 1));
            }
        }
    }

    println!("{line}");
}

fn table_total_width(widths: &[usize], space_after: &[bool]) -> usize {
    widths
        .iter()
        .zip(space_after.iter())
        .map(|(width, space)| width + usize::from(*space))
        .sum()
}

fn get_console_width() -> usize {
    get_console_width_impl()
}

fn display_width(value: &str) -> usize {
    value.chars().count()
}

#[cfg(windows)]
fn get_console_width_impl() -> usize {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_OUTPUT_HANDLE};

    unsafe {
        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if let Some(width) = try_console_width(stdout_handle) {
            return width;
        }

        let mut conout = "CONOUT$\0".encode_utf16().collect::<Vec<_>>();
        let console_handle = CreateFileW(
            conout.as_mut_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if console_handle.is_null() || console_handle == INVALID_HANDLE_VALUE {
            return 119;
        }

        let width = try_console_width(console_handle).unwrap_or(119);
        CloseHandle(console_handle);
        width
    }
}

#[cfg(windows)]
unsafe fn try_console_width(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<usize> {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        CONSOLE_SCREEN_BUFFER_INFO, GetConsoleScreenBufferInfo,
    };

    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return None;
    }

    let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { zeroed() };
    if unsafe { GetConsoleScreenBufferInfo(handle, &mut info) } == 0 {
        return None;
    }

    usize::try_from(info.dwSize.X)
        .ok()
        .and_then(|width| width.checked_sub(2))
        .filter(|width| *width > 0)
}

#[cfg(not(windows))]
fn get_console_width_impl() -> usize {
    120
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }

    let mut output = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    output.push('…');
    output
}

fn print_validate(manifest_path: &str, ignore_warnings: bool) -> Result<()> {
    use std::path::Path;

    let path = Path::new(manifest_path);
    if !path.exists() {
        bail!("Path does not exist: {manifest_path}");
    }

    let files: Vec<std::path::PathBuf> = if path.is_dir() {
        std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|ext| ext == "yaml" || ext == "yml")
            })
            .collect()
    } else {
        vec![path.to_path_buf()]
    };

    if files.is_empty() {
        bail!("No YAML manifest files found in: {manifest_path}");
    }

    let mut all_errors: Vec<String> = Vec::new();
    let mut all_warnings: Vec<String> = Vec::new();

    // Schema base path — relative to the repo root or binary
    let schema_base = find_schema_dir();

    for file in &files {
        let content = std::fs::read_to_string(file)?;
        let yaml_value: serde_json::Value = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("YAML parse error in {}: {e}", file.display()))?;

        let manifest_type = yaml_value
            .get("ManifestType")
            .and_then(|v| v.as_str())
            .unwrap_or("singleton")
            .to_lowercase();
        let manifest_version = yaml_value
            .get("ManifestVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("0.1.0");

        let schema_version = map_manifest_version(manifest_version);
        let schema_file_name = format!("manifest.{manifest_type}.{manifest_version}.json");

        if let Some(ref base) = schema_base {
            let schema_path = base.join(&schema_version).join(&schema_file_name);
            if schema_path.exists() {
                let schema_content = std::fs::read_to_string(&schema_path)?;
                let schema_json: serde_json::Value = serde_json::from_str(&schema_content)?;

                let validator = match jsonschema::validator_for(&schema_json) {
                    Ok(v) => v,
                    Err(e) => {
                        all_warnings.push(format!(
                            "  {}: could not compile schema {}: {e}",
                            file.display(),
                            schema_path.display()
                        ));
                        continue;
                    }
                };

                let result = validator.validate(&yaml_value);
                if let Err(error) = result {
                    let msg = format!(
                        "  {}: {} (at {})",
                        file.display(),
                        error,
                        error.instance_path
                    );
                    all_errors.push(msg);
                }
            } else {
                all_warnings.push(format!(
                    "  {}: schema not found: {}",
                    file.display(),
                    schema_path.display()
                ));
            }
        } else {
            // No schema dir — just do basic YAML parse validation
            all_warnings.push(format!(
                "  {}: schema directory not found, performing YAML-only validation",
                file.display()
            ));
        }

        // Basic field checks
        if manifest_type == "singleton" || manifest_type == "installer" {
            if yaml_value.get("Installers").is_none() {
                all_errors.push(format!(
                    "  {}: required field 'Installers' is missing",
                    file.display()
                ));
            }
        }
        if manifest_type == "singleton" || manifest_type == "defaultlocale" {
            for field in ["PackageIdentifier", "PackageVersion"] {
                if yaml_value.get(field).is_none()
                    && manifest_version != "0.1.0"
                {
                    all_errors.push(format!(
                        "  {}: required field '{field}' is missing",
                        file.display()
                    ));
                }
            }
        }
    }

    if !all_warnings.is_empty() && !ignore_warnings {
        println!("Manifest validation warning.");
        for w in &all_warnings {
            println!("{w}");
        }
    }

    if !all_errors.is_empty() {
        println!("Manifest validation failed.");
        for e in &all_errors {
            println!("{e}");
        }
        std::process::exit(1);
    }

    if all_warnings.is_empty() || ignore_warnings {
        println!("Manifest validation succeeded.");
    }

    Ok(())
}

fn find_schema_dir() -> Option<std::path::PathBuf> {
    // Try relative to the current exe first, then walk up looking for schemas/JSON/manifests
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..10 {
            if let Some(ref d) = dir {
                let candidate = d.join("schemas").join("JSON").join("manifests");
                if candidate.is_dir() {
                    return Some(candidate);
                }
                dir = d.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }
    // Try from cwd
    let mut dir = std::env::current_dir().ok();
    for _ in 0..10 {
        if let Some(ref d) = dir {
            let candidate = d.join("schemas").join("JSON").join("manifests");
            if candidate.is_dir() {
                return Some(candidate);
            }
            dir = d.parent().map(|p| p.to_path_buf());
        } else {
            break;
        }
    }
    None
}

fn map_manifest_version(version: &str) -> String {
    // Map version strings to schema directory names
    // "1.6.0" -> "v1.6.0", "0.1.0" -> "preview", "latest" -> "latest"
    if version == "0.1.0" {
        "preview".to_string()
    } else if version.starts_with("1.") {
        format!("v{version}")
    } else {
        "latest".to_string()
    }
}

fn do_download(result: &ShowResult, download_dir: Option<&str>) -> Result<()> {
    let installer = result
        .manifest
        .installers
        .first()
        .ok_or_else(|| anyhow::anyhow!("No installer found for package"))?;

    let url = installer
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("No installer URL found"))?;

    let dir = match download_dir {
        Some(d) => std::path::PathBuf::from(d),
        None => std::env::current_dir()?,
    };

    std::fs::create_dir_all(&dir)?;

    // Derive filename from URL
    let filename = url
        .rsplit('/')
        .next()
        .unwrap_or("installer")
        .split('?')
        .next()
        .unwrap_or("installer");
    let dest = dir.join(filename);

    println!(
        "Downloading {} v{} ...",
        result.manifest.id, result.manifest.version
    );
    println!("  URL: {url}");
    println!("  Destination: {}", dest.display());

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let response = client.get(url).send()?;
    if !response.status().is_success() {
        bail!(
            "Download failed: HTTP {}",
            response.status()
        );
    }
    let bytes = response.bytes()?;
    std::fs::write(&dest, &bytes)?;

    // Verify hash if available
    if let Some(ref expected_sha) = installer.sha256 {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = hasher.finalize();
        let actual_sha = hash
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if actual_sha.eq_ignore_ascii_case(expected_sha) {
            println!("  SHA256 verified: {expected_sha}");
        } else {
            println!("  SHA256 MISMATCH! Expected: {expected_sha}, Got: {actual_sha}");
            std::process::exit(1);
        }
    }

    println!("Download complete: {}", dest.display());
    Ok(())
}

fn print_pin_list() {
    // Pinning database is at %LOCALAPPDATA%\Microsoft\WinGet\pins.db
    // For now, try to open and list any pins, or say none found
    let pins_path = dirs::data_local_dir()
        .map(|d| d.join("Microsoft").join("WinGet").join("pins.db"));

    match pins_path {
        Some(ref path) if path.exists() => {
            match rusqlite::Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            ) {
                Ok(conn) => {
                    // Try to read pin entries
                    let mut stmt = match conn.prepare(
                        "SELECT package_id, version, source_id, pin_type FROM pin",
                    ) {
                        Ok(s) => s,
                        Err(_) => {
                            println!("No pins found.");
                            return;
                        }
                    };

                    let rows: Vec<(String, String, String, i64)> = stmt
                        .query_map([], |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                            ))
                        })
                        .ok()
                        .map(|r| r.filter_map(|r| r.ok()).collect())
                        .unwrap_or_default();

                    if rows.is_empty() {
                        println!("No pins found.");
                        return;
                    }

                    println!(
                        "{:<40} {:<20} {:<15} {}",
                        "Package Id", "Version", "Source", "Pin Type"
                    );
                    println!("{}", "-".repeat(85));
                    for (id, version, source, pin_type) in &rows {
                        let type_str = match pin_type {
                            0 => "Pinning",
                            1 => "Blocking",
                            2 => "Gating",
                            _ => "Unknown",
                        };
                        println!(
                            "{:<40} {:<20} {:<15} {}",
                            id, version, source, type_str
                        );
                    }
                }
                Err(e) => {
                    println!("Could not open pins database: {e}");
                }
            }
        }
        _ => {
            println!("No pins found.");
        }
    }
}
