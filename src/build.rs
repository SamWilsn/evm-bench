use crate::metadata::Benchmark;
use color_eyre::eyre::{ensure, Result};
use itertools::Itertools;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use users::{get_current_gid, get_current_uid};

#[derive(Clone, Debug)]
struct BuildContext {
    docker_executable: PathBuf,
    contract_path: PathBuf,
    contract_context_path: PathBuf,
    build_path: PathBuf,
}

#[derive(Debug)]
pub struct BuildResult {
    pub contract_bin_path: PathBuf,
}

#[derive(Debug)]
pub struct BuiltBenchmark {
    pub benchmark: Benchmark,
    pub result: BuildResult,
}

fn build_benchmark(
    benchmark: &Benchmark,
    force: bool,
    build_context: &BuildContext,
) -> Result<BuiltBenchmark> {
    let contract_name = benchmark.contract.file_name().unwrap().to_string_lossy().to_string();

    info!(
        "building benchmark {} ({contract_name} w/ solc@{})...",
        benchmark.name, benchmark.solc_version
    );

    let relative_contract_path =
        build_context.contract_path.strip_prefix(&build_context.contract_context_path)?;

    let docker_contract_context_path = PathBuf::from("/benchmark");
    let docker_contract_path = docker_contract_context_path.join(relative_contract_path);
    let docker_build_path = PathBuf::from("/build");

    fs::create_dir_all(&build_context.build_path)?;

    let contract_bin_path = build_context.build_path.join(&contract_name).with_extension("bin");

    if !force && contract_bin_path.exists() {
        debug!("benchmark {} already built", benchmark.name);
        return Ok(BuiltBenchmark {
            benchmark: benchmark.clone(),
            result: BuildResult { contract_bin_path },
        });
    }

    let mut cmd = Command::new(&build_context.docker_executable);
    cmd.arg("run");
    cmd.arg("-u").arg(&format!("{}:{}", get_current_uid(), get_current_gid()));
    cmd.arg("-v").arg(&format!(
        "{}:{}",
        build_context.contract_context_path.display(),
        docker_contract_context_path.display()
    ));
    cmd.arg("-v").arg(&format!(
        "{}:{}",
        build_context.build_path.display(),
        docker_build_path.display()
    ));
    cmd.arg(format!("ethereum/solc:{}", benchmark.solc_version));
    cmd.arg("-o").arg(&docker_build_path);
    cmd.args(["--optimize", "--optimize-runs=1000000"]);
    cmd.args(["--abi", "--bin", "--bin-runtime", "--overwrite"]);
    cmd.arg(docker_contract_path);
    trace!("cmd: {cmd:?}");
    let out = cmd.output()?;
    trace!("stdout: {}", String::from_utf8_lossy(&out.stdout));
    trace!("stderr: {}", String::from_utf8_lossy(&out.stderr));
    ensure!(out.status.success(), "could not build benchmark: {out:#?}");

    debug!("built benchmark {}", benchmark.name);
    Ok(BuiltBenchmark { benchmark: benchmark.clone(), result: BuildResult { contract_bin_path } })
}

pub fn build_benchmarks(
    benchmarks: &[Benchmark],
    docker_executable: &Path,
    builds_path: &Path,
    force: bool,
) -> Result<Vec<BuiltBenchmark>> {
    info!("building {} benchmarks...", benchmarks.len());
    debug!("benchmarks: {}", benchmarks.iter().map(|b| &b.name).format(", "));

    let mut results = Vec::<BuiltBenchmark>::with_capacity(benchmarks.len());
    for benchmark in benchmarks {
        results.push(build_benchmark(
            benchmark,
            force,
            &BuildContext {
                docker_executable: docker_executable.to_path_buf(),
                contract_path: benchmark.contract.clone(),
                contract_context_path: benchmark.build_context.clone(),
                build_path: builds_path.join(&benchmark.name),
            },
        )?);
    }

    debug!("built {} benchmarks", benchmarks.len());
    Ok(results)
}
