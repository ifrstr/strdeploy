use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use clap::Parser;
use env_logger::Builder;
use log::{debug, info, warn};
use serde::Deserialize;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Working Directory
    #[arg(short = 'd', long)]
    workdir: Option<PathBuf>,

    /// Do not actually build and push
    #[arg(long)]
    dry_run: bool,
}

#[derive(Deserialize, Debug)]
struct Config<'a> {
    tenant: &'a str,

    namespace: &'a str,

    mode: Mode,

    image: ImageConfig<'a>,
}

#[derive(Deserialize, Debug)]
enum Mode {
    #[serde(rename = "branch")]
    Branch,
}

#[derive(Deserialize, Debug)]
struct ImageConfig<'a> {
    namespace: &'a str,

    name: &'a str,
}

fn main() {
    // Init logging
    let mut log_builder = Builder::new();
    log_builder.filter_level(log::LevelFilter::Info);
    log_builder.init();
    info!("strdeploy v{}", VERSION);

    // Parsing arguments
    let cli = Cli::parse();

    // Parsing workdir
    let workdir = match cli.workdir {
        Some(w) => w,
        None => std::env::current_dir().expect("Failed to get current dir"),
    };
    info!("Working dir: {}", workdir.display());

    // Parsing project
    let mut config_path = PathBuf::from(&workdir);
    config_path.push("strdeploy.yml");
    let config_raw = fs::read_to_string(config_path).expect("Failed to read strdeploy.yml");
    let config: Config = serde_yaml::from_str(&config_raw).expect("Failed to parse strdeploy.yml");
    debug!("Config: {:?}", config);

    let tenant = match config.tenant {
        "internal" => "internal",
        x => panic!("Unknown tenant: {}", x),
    };

    let registry = match config.namespace {
        "internal" => "cr.ilharper.com",
        x => panic!("Unknown namespace: {}", x),
    };

    // Display
    info!("Tenant: {}", tenant);
    info!("Target registry: {}", registry);

    // Get git branch info
    let branch_str = String::from_utf8_lossy(
        &Command::new("git")
            .current_dir(&workdir)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .stderr(Stdio::inherit())
            .output()
            .expect("Failed to get current branch")
            .stdout,
    )
    .into_owned();
    let branch = branch_str.trim();

    if branch == "HEAD" {
        panic!("You are in 'detached HEAD' state. Cannot use 'HEAD' in branch mode. Try checkout a branch.");
    }
    info!("Target branch: {}", branch);

    // Get build number
    let build_str = String::from_utf8_lossy(
        &Command::new("git")
            .current_dir(&workdir)
            .args(["rev-list", "--count", "HEAD"])
            .stderr(Stdio::inherit())
            .output()
            .expect("Failed to get build number")
            .stdout,
    )
    .into_owned();
    let build = build_str.trim();
    info!("Build number: {}", build);

    let image = format!(
        "{}/{}/{}:{}-{}",
        registry, config.image.namespace, config.image.name, branch, build
    );

    warn!("Start building image: {}", &image);
    if cli.dry_run {
        warn!("Skipping building in dry-run.");
    } else {
        Command::new("docker")
            .current_dir(&workdir)
            .args(["build", "--force-rm", "-t", &image, "."])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to start building process")
            .wait()
            .expect("Failed to build image");
    }

    warn!("Start pushing image: {}", &image);
    if cli.dry_run {
        warn!("Skipping pushing in dry-run.");
    } else {
        Command::new("docker")
            .current_dir(&workdir)
            .args(["push", &image])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to start pushing process")
            .wait()
            .expect("Failed to push image");
    }
}
