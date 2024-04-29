#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![allow(clippy::mutable_key_type)]

#[macro_use]
extern crate tracing;

use clap::Parser;
use color_eyre::eyre::{ensure, Result};
use itertools::Itertools;
use std::{fs, path::PathBuf};

mod build;
use build::build_benchmarks;

mod exec;
use exec::validate_executable;

mod metadata;
use metadata::{find_benchmarks, find_runners, BenchmarkDefaults};

mod results;
use results::{print_results, record_results};

mod run;
use run::run_benchmarks_on_runners;

/// Ethereum Virtual Machine Benchmark
#[derive(Debug, Parser)]
struct Cli {
    /// Path to use as the base for benchmarks searching
    #[arg(long, default_value = "./benchmarks")]
    benchmark_search_path: PathBuf,

    /// Names of benchmarks to run.
    #[arg(long, default_value = None)]
    benchmarks: Option<Vec<String>>,

    /// Path to use as the base for runners searching
    #[arg(short, long, default_value = "./runners")]
    runner_search_path: PathBuf,

    /// Names of runners to use.
    #[arg(long, default_value = None)]
    runners: Option<Vec<String>>,

    /// Output path for build artifacts and other things
    #[arg(short, long, default_value = "./outputs")]
    output_path: PathBuf,

    /// Name of the output file, will not overwrite.
    /// Default means to use the current datetime.
    #[arg(long, default_value = None)]
    output_file_name: Option<String>,

    /// Path to a Docker executable (this is used for solc)
    #[arg(long)]
    docker_executable: Option<PathBuf>,

    /// Path to a CPython executable (this is used for runners)
    #[arg(long)]
    cpython_executable: Option<PathBuf>,

    /// Path to a PyPy executable (this is used for runners)
    #[arg(long)]
    pypy_executable: Option<PathBuf>,

    /// Path to a NPM executable (this is used for runners)
    #[arg(long)]
    npm_executable: Option<PathBuf>,

    /// Path to benchmark metadata schema
    #[arg(long, default_value = "./benchmarks/schema.json")]
    benchmark_metadata_schema: PathBuf,

    /// Name of benchmark metadata file to search for
    #[arg(long, default_value = "benchmark.evm-bench.json")]
    benchmark_metadata_name: String,

    /// Path to runner metadata schema
    #[arg(long, default_value = "./runners/schema.json")]
    runner_metadata_schema: PathBuf,

    /// Name of benchmark metadata file to search
    #[arg(long, default_value = "runner.evm-bench.json")]
    runner_metadata_name: String,

    /// Default solc version to use if none specified in the benchmark metadata
    #[arg(long, default_value = "stable")]
    default_solc_version: String,

    /// Default number of runs to use if none specified in the benchmark metadata
    #[arg(long, default_value = "10")]
    default_num_runs: u64,

    /// Default calldata to use if none specified in the benchmark metadata
    #[arg(long, default_value = "")]
    default_calldata_str: String,

    /// Always build benchmarks, even if they are already built
    #[arg(long)]
    force_build: bool,
}

fn main() -> Result<()> {
    let _ = color_eyre::install();
    let _ = init_tracing_subscriber();

    let cli = Cli::parse();

    let docker_executable = validate_executable("docker", cli.docker_executable.as_deref())?;
    let _ = validate_executable("cargo", None)?;
    let _ = validate_executable("poetry", None)?;
    let _ = validate_executable("python3", cli.cpython_executable.as_deref())?;
    let _ = validate_executable("pypy3", cli.pypy_executable.as_deref())?;
    let _ = validate_executable("npm", cli.npm_executable.as_deref())?;

    let default_calldata = alloy_primitives::hex::decode(&cli.default_calldata_str)?;

    let benchmarks_path = cli.benchmark_search_path.canonicalize()?;
    let mut benchmarks = find_benchmarks(
        &cli.benchmark_metadata_name,
        &cli.benchmark_metadata_schema,
        &benchmarks_path,
        BenchmarkDefaults {
            solc_version: cli.default_solc_version,
            num_runs: cli.default_num_runs,
            calldata: default_calldata.into(),
        },
    )?;
    if let Some(arg_benchmarks) = &cli.benchmarks {
        let known = benchmarks.iter().map(|r| &r.name);
        let unknown = arg_benchmarks
            .iter()
            .filter(|&arg| !known.clone().any(|r| arg == r))
            .collect::<Vec<_>>();
        ensure!(unknown.is_empty(), "unknown benchmarks(s): {}", unknown.iter().format(", "));
        benchmarks.retain(|b| arg_benchmarks.contains(&b.name));
    }
    benchmarks.sort_by(|a, b| a.name.cmp(&b.name));

    let runners_path = cli.runner_search_path.canonicalize()?;
    let mut runners =
        find_runners(&cli.runner_metadata_name, &cli.runner_metadata_schema, &runners_path, ())?;
    if let Some(arg_runners) = &cli.runners {
        let known = runners.iter().map(|r| &r.name);
        let unknown =
            arg_runners.iter().filter(|&arg| !known.clone().any(|r| arg == r)).collect::<Vec<_>>();
        ensure!(unknown.is_empty(), "unknown runner(s): {}", unknown.iter().format(", "));
        runners.retain(|r| arg_runners.contains(&r.name));
    }
    runners.sort_by(|a, b| a.name.cmp(&b.name));

    fs::create_dir_all(&cli.output_path)?;
    let outputs_path = cli.output_path.canonicalize()?;

    let builds_path = outputs_path.join("build");
    fs::create_dir_all(&builds_path)?;
    let built_benchmarks =
        build_benchmarks(&benchmarks, &docker_executable, &builds_path, cli.force_build)?;

    let results = run_benchmarks_on_runners(&built_benchmarks, &runners)?;

    let results_path = outputs_path.join("results");
    fs::create_dir_all(&results_path)?;
    let result_file_path = record_results(&results_path, cli.output_file_name, &results)?;
    print_results(&result_file_path)?;

    Ok(())
}

fn init_tracing_subscriber() -> Result<(), tracing_subscriber::util::TryInitError> {
    use tracing_subscriber::prelude::*;
    tracing_subscriber::Registry::default()
        .with(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_error::ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer())
        .try_init()
}
