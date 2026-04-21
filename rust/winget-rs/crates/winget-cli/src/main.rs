use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use winget_core::{
    CacheWarmResult, Documentation, ListQuery, ListResponse, PackageQuery, Repository,
    SearchResponse, ShowResult, SourceRecord, SourceUpdateResult, VersionsResult,
};

#[derive(Parser)]
#[command(name = "winget", about = "Pure Rust subset of the winget CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "ls")]
    List(ListArgs),
    Show(ShowArgs),
    Search(SearchArgs),
    Source {
        #[command(subcommand)]
        command: SourceCommands,
    },
    Cache {
        #[command(subcommand)]
        command: CacheCommands,
    },
}

#[derive(Subcommand)]
enum SourceCommands {
    List,
    Update { source: Option<String> },
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
    let mut repository = Repository::open()?;

    match cli.command {
        Commands::List(args) => {
            print_list_result(
                repository.list(&args.clone().into())?,
                args.details,
                args.upgrade,
            );
        }
        Commands::Show(args) => {
            if args.versions {
                print_versions(repository.show_versions(&args.query.into())?);
            } else {
                print_show(repository.show(&args.query.into())?);
            }
        }
        Commands::Search(args) => {
            if args.versions {
                print_versions(repository.search_versions(&args.clone().into())?);
            } else {
                print_search(repository.search(&args.into())?);
            }
        }
        Commands::Source { command } => match command {
            SourceCommands::List => print_sources(repository.list_sources()),
            SourceCommands::Update { source } => {
                print_source_updates(repository.update_sources(source.as_deref())?)
            }
        },
        Commands::Cache { command } => match command {
            CacheCommands::Warm(args) => print_cache_warm(repository.warm_cache(&args.into())?),
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

fn print_sources(sources: Vec<SourceRecord>) {
    println!(
        "{:<12} {:<11} {:<30} Argument",
        "Name", "Type", "Identifier"
    );
    for source in sources {
        println!(
            "{:<12} {:<11} {:<30} {}",
            source.name, source.kind, source.identifier, source.arg
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
        println!("No installed package found.");
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
