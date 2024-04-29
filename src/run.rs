use crate::{
    build::BuiltBenchmark,
    metadata::{Benchmark, Runner},
};
use alloy_primitives::hex;
use color_eyre::eyre::{ensure, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, process::Command, time::Duration};

type BenchmarkResults = HashMap<Benchmark, RunResult>;
pub type Results = HashMap<Runner, BenchmarkResults>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunResult {
    pub run_times: Vec<Duration>,
}

impl RunResult {
    pub fn average(&self) -> Option<Duration> {
        if self.run_times.is_empty() {
            return None;
        }
        Some(self.sum() / self.run_times.len() as u32)
    }

    fn sum(&self) -> Duration {
        self.run_times.iter().sum()
    }
}

pub fn run_benchmarks_on_runners(
    benchmarks: &Vec<BuiltBenchmark>,
    runners: &Vec<Runner>,
) -> Result<Results> {
    info!("running {} benchmarks on {} runners...", benchmarks.len(), runners.len());
    debug!("runners: {}", runners.iter().map(|r| &r.name).format(", "));
    debug!("benchmarks: {}", benchmarks.iter().map(|b| &b.benchmark.name).format(", "));

    let mut results = Results::with_capacity(runners.len());
    for runner in runners {
        results.insert(runner.clone(), run_benchmarks_on_runner(runner, benchmarks));
    }

    debug!("ran {} benchmarks ({} successful)", benchmarks.len(), results.len());
    Ok(results)
}

fn run_benchmarks_on_runner(runner: &Runner, benchmarks: &[BuiltBenchmark]) -> BenchmarkResults {
    info!("running benchmarks on {}...", runner.name);

    // NOTE: It is expected that this map contains all benchmarks.
    let mut results = BenchmarkResults::with_capacity(benchmarks.len());
    for benchmark in benchmarks {
        let result = match run_benchmark_on_runner(benchmark, runner) {
            Ok(res) => res,
            Err(e) => {
                warn!(
                    "could not run benchmark {} on runner {}: {e}",
                    benchmark.benchmark.name, runner.name
                );
                RunResult::default()
            }
        };
        results.insert(benchmark.benchmark.clone(), result);
    }

    debug!("ran {} benchmarks on {} ({} successful)", benchmarks.len(), runner.name, results.len());
    results
}

fn run_benchmark_on_runner(benchmark: &BuiltBenchmark, runner: &Runner) -> Result<RunResult> {
    info!("running benchmark {}...", benchmark.benchmark.name);
    debug!(
        "running {} times using code {} with calldata {}...",
        benchmark.benchmark.num_runs,
        benchmark.result.contract_bin_path.file_name().unwrap().to_string_lossy(),
        hex::encode(&benchmark.benchmark.calldata),
    );

    let mut cmd = Command::new(&runner.entry);
    cmd.arg("--contract-code-path").arg(&benchmark.result.contract_bin_path);
    cmd.arg("--calldata").arg(&hex::encode(&benchmark.benchmark.calldata));
    cmd.arg("--num-runs").arg(&benchmark.benchmark.num_runs.to_string());
    trace!("cmd: {cmd:?}");
    let out = cmd.output()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    trace!("stdout: {stdout}");
    trace!("stderr: {}", String::from_utf8_lossy(&out.stderr));
    ensure!(out.status.success(), "could not run benchmark: {}", out.status);

    let mut run_times: Vec<Duration> = Vec::with_capacity(16);
    for line in stdout.trim().lines() {
        let millis: f64 = line.parse()?;
        run_times.push(Duration::try_from_secs_f64(millis / 1000.0)?);
    }

    debug!("ran benchmark {}", benchmark.benchmark.name);
    Ok(RunResult { run_times })
}
