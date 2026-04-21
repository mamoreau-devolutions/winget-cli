use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use winget_core::{
    CacheWarmResult, PackageQuery, Repository, SearchResponse, ShowResult, SourceRecord,
    SourceUpdateResult, VersionsResult,
};

#[derive(Parser)]
#[command(name = "winget-rs", about = "Pure Rust subset of the winget CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Show(ShowArgs),
    Search(QueryArgs),
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
    query: Option<String>,
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    moniker: Option<String>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    exact: bool,
    #[arg(long)]
    version: Option<String>,
    #[arg(long)]
    channel: Option<String>,
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
        Commands::Show(args) => {
            if args.versions {
                print_versions(repository.show_versions(&args.query.into())?);
            } else {
                print_show(repository.show(&args.query.into())?);
            }
        }
        Commands::Search(args) => {
            print_search(repository.search(&args.into())?);
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
            query: value.query,
            id: value.id,
            name: value.name,
            moniker: value.moniker,
            source: value.source,
            exact: value.exact,
            version: value.version,
            channel: value.channel,
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

fn print_versions(result: VersionsResult) {
    print_warnings(&result.warnings);
    println!(
        "{} [{}] ({})",
        result.package.name, result.package.id, result.package.source_name
    );
    println!("{:<20} Channel", "Version");
    for version in result.versions {
        println!("{:<20} {}", version.version, version.channel);
    }
}

fn print_show(result: ShowResult) {
    print_warnings(&result.warnings);
    println!(
        "{} [{}] ({})",
        result.package.name, result.package.id, result.package.source_name
    );
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
        print_multiline("Description", value);
    }
    if let Some(value) = &result.manifest.package_url {
        print_field("Package Url", value);
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
    if let Some(value) = &result.manifest.release_notes {
        print_multiline("Release Notes", value);
    }
    if let Some(value) = &result.manifest.release_notes_url {
        print_field("Release Notes Url", value);
    }
    if !result.manifest.tags.is_empty() {
        print_field("Tags", &result.manifest.tags.join(", "));
    }

    if let Some(installer) = result.manifest.installers.first() {
        println!("Installer");
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
        if let Some(value) = &installer.release_date {
            print_indented_field("ReleaseDate", value);
        }
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
    println!("{label:<22} {value}");
}

fn print_indented_field(label: &str, value: &str) {
    println!("  {label:<20} {value}");
}

fn print_multiline(label: &str, value: &str) {
    println!("{label}");
    for line in value.lines() {
        println!("  {line}");
    }
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
