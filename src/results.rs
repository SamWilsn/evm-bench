use crate::{
    metadata::{Benchmark, Runner},
    run::{Results, RunResult},
};
use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use tabled::{builder::Builder, settings::Style, Table};

#[derive(Serialize, Deserialize)]
pub(crate) struct ResultsFormatted {
    benchmarks: HashMap<String, Benchmark>,
    runners: HashMap<String, Runner>,
    runs: HashMap<String, HashMap<String, RunResult>>,
}

impl ResultsFormatted {
    pub fn new(results: &Results) -> Self {
        Self {
            benchmarks: results.keys().map(|b| (b.name.clone(), b.clone())).collect(),
            runners: results
                .values()
                .flat_map(HashMap::keys)
                .map(|r| (r.name.clone(), r.clone()))
                .collect(),
            runs: results
                .iter()
                .map(|(b, br)| {
                    (
                        b.name.clone(),
                        br.iter().map(|(r, rr)| (r.name.clone(), rr.clone())).collect(),
                    )
                })
                .collect(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        info!("reading and parsing results from {}...", path.display());
        let s = fs::read_to_string(path)?;
        let results = serde_json::from_str::<ResultsFormatted>(&s)?;
        debug!("read and parsed results from {}", path.display());
        Ok(results)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let file = fs::File::create_new(&path)?;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, self)?;
        writer.flush()?;
        info!("wrote out results to {}", path.display());
        Ok(())
    }

    pub fn print(&self) {
        println!("{}", self.table());
    }

    pub fn table(&self) -> Table {
        let mut runner_names: Vec<_> = self.runners.keys().collect();
        runner_names.sort();

        let mut runs = self.runs.iter().collect::<Vec<_>>();
        runs.sort_by(|(a, _), (b, _)| a.cmp(b));

        let mut runner_times = HashMap::<String, Vec<Duration>>::new();
        for (run_name, benchmark_runs) in &runs {
            for &runner_name in &runner_names {
                let Some(run) = benchmark_runs.get(runner_name) else {
                    warn!("no runs for {run_name}/{runner_name}");
                    continue;
                };
                let avg_run_time =
                    run.run_times.iter().sum::<Duration>().div_f64(run.run_times.len() as f64);
                runner_times.entry(runner_name.clone()).or_default().push(avg_run_time);
            }
        }
        runner_names.sort_by_key(|&name| runner_times[name].iter().sum::<Duration>());

        let mut builder = Builder::default();

        // Header.
        builder.push_record(std::iter::once("").chain(runner_names.iter().map(|s| s.as_str())));

        // Sum of all times.
        let average_runner_times = runner_times
            .iter()
            .map(|(name, times)| (name, times.iter().sum::<Duration>()))
            .collect::<HashMap<&String, Duration>>();
        let mut record = vec!["**sum**".to_string()];
        record.extend(
            runner_names
                .iter()
                .map(|&runner_name| average_runner_times.get(runner_name))
                .map(|val| Some(format!("{:>9.3?}", val?)))
                .map(|s| s.unwrap_or_default()),
        );
        builder.push_record(record);

        // Relative times.
        let min_runner_time =
            average_runner_times.values().min().copied().unwrap_or(Duration::from_secs(1));
        let mut record = vec!["**relative**".to_string()];
        record.extend(
            runner_names
                .iter()
                .map(|&name| {
                    average_runner_times.get(name).map(|time| {
                        format!("{:>9.3?}", time.as_secs_f64() / min_runner_time.as_secs_f64())
                    })
                })
                .map(Option::unwrap_or_default),
        );
        builder.push_record(record);

        // Individual runs.
        for &(benchmark_name, benchmark_runs) in runs.iter() {
            let vals = runner_names.iter().map(|&runner_name| {
                let run = benchmark_runs.get(runner_name)?;
                let avg_run_time =
                    run.run_times.iter().sum::<Duration>().div_f64(run.run_times.len() as f64);
                runner_times.entry(runner_name.clone()).or_default().push(avg_run_time);
                Some(avg_run_time)
            });

            let mut record = vec![benchmark_name.clone()];
            record.extend(
                vals.map(|val| val.map(|time| format!("{time:>9.3?}")).unwrap_or_default()),
            );
            builder.push_record(record);
        }

        let mut table = builder.build();
        table.with(Style::markdown());
        table
    }
}

pub fn record_results(
    results_path: &Path,
    result_file_name: Option<String>,
    results: &Results,
) -> Result<PathBuf> {
    debug!("writing all results out...");

    let result_file_name = result_file_name.unwrap_or_else(|| {
        format!("{}.evm-bench.results.json", chrono::offset::Utc::now().to_rfc3339())
    });
    let result_file_path = results_path.join(result_file_name);

    fs::create_dir_all(results_path)?;
    ResultsFormatted::new(results).save(&result_file_path)?;

    Ok(result_file_path)
}

pub fn print_results(results_file_path: &Path) -> Result<()> {
    let results = ResultsFormatted::load(results_file_path)?;
    results.print();
    Ok(())
}
