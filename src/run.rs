use crate::{
    build::BuiltBenchmark,
    metadata::{Benchmark, Runner},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    error,
    process::Command,
    time::Duration,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunResult {
    pub run_times: Vec<Duration>,
}

type BenchmarkResults = HashMap<Runner, RunResult>;
pub type Results = HashMap<Benchmark, BenchmarkResults>;

fn run_benchmark_on_runner(
    benchmark: &BuiltBenchmark,
    runner: &Runner,
) -> Result<RunResult, Box<dyn error::Error>> {
    log::info!(
        "running benchmark {} on runner {}...",
        benchmark.benchmark.name,
        runner.name
    );
    log::debug!(
        "running {} times using code {} with calldata {}...",
        benchmark.benchmark.num_runs,
        benchmark
            .result
            .contract_bin_path
            .file_name()
            .unwrap()
            .to_string_lossy(),
        hex::encode(&benchmark.benchmark.calldata),
    );

    let mut cmd = Command::new(&runner.entry);
    cmd.arg("--contract-code-path")
        .arg(&benchmark.result.contract_bin_path);
    cmd.arg("--calldata")
        .arg(&hex::encode(&benchmark.benchmark.calldata));
    cmd.arg("--num-runs")
        .arg(&benchmark.benchmark.num_runs.to_string());
    log::trace!("cmd: {cmd:?}");
    let out = cmd.output()?;
    let stdout = String::from_utf8(out.stdout).unwrap();
    log::trace!("stdout: {}", stdout);
    log::trace!("stderr: {}", String::from_utf8(out.stderr).unwrap());
    if !out.status.success() {
        return Err(out.status.to_string().into());
    }

    let mut times: Vec<Duration> = Vec::new();
    for line in stdout.trim().lines() {
        let millis: f64 = line.parse()?;
        times.push(Duration::try_from_secs_f64(millis / 1000.0)?);
    }

    log::debug!(
        "ran benchmark {} on runner {}",
        benchmark.benchmark.name,
        runner.name
    );
    Ok(RunResult { run_times: times })
}

fn run_benchmark_on_runners(
    benchmark: &BuiltBenchmark,
    runners: &Vec<Runner>,
) -> Result<BenchmarkResults, Box<dyn error::Error>> {
    let runner_names = runners
        .iter()
        .map(|b| b.name.clone())
        .collect::<HashSet<_>>();

    log::info!(
        "running benchmark {} on {} runners...",
        benchmark.benchmark.name,
        runners.len()
    );
    log::debug!(
        "runners: {}",
        runner_names.iter().cloned().collect::<Vec<_>>().join(", ")
    );

    let mut results = HashMap::<Runner, RunResult>::new();
    for runner in runners {
        let result = match run_benchmark_on_runner(benchmark, runner) {
            Ok(res) => res,
            Err(e) => {
                log::warn!(
                    "could not run benchmark {} on runner {}: {e}",
                    benchmark.benchmark.name,
                    runner.name
                );
                continue;
            }
        };
        results.insert(runner.clone(), result);
    }

    log::debug!(
        "ran benchmark {} on {} runners ({} successful)",
        benchmark.benchmark.name,
        runners.len(),
        results.len()
    );
    Ok(results)
}

pub fn run_benchmarks_on_runners(
    benchmarks: &Vec<BuiltBenchmark>,
    runners: &Vec<Runner>,
) -> Result<Results, Box<dyn error::Error>> {
    let benchmark_names = benchmarks
        .iter()
        .map(|b| b.benchmark.name.clone())
        .collect::<HashSet<_>>();

    log::info!("running {} benchmarks...", benchmarks.len());
    log::debug!(
        "benchmarks: {}",
        benchmark_names
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut results: HashMap<Benchmark, HashMap<Runner, RunResult>> = HashMap::new();
    for benchmark in benchmarks {
        let result = match run_benchmark_on_runners(benchmark, &runners) {
            Ok(res) => res,
            Err(e) => {
                log::warn!(
                    "could not run benchmark {} on runners: {e}",
                    benchmark.benchmark.name
                );
                continue;
            }
        };

        results.insert(benchmark.benchmark.clone(), result);
    }

    log::debug!(
        "ran {} benchmarks ({} successful)",
        benchmarks.len(),
        results.len()
    );
    Ok(results)
}
