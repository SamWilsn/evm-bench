use crate::{
    metadata::{Benchmark, Runner},
    run::{Results, RunResult},
};
use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use tabled::{builder::Builder, settings::Style};

#[derive(Serialize, Deserialize)]
struct ResultsFormatted {
    benchmarks: HashMap<String, Benchmark>,
    runners: HashMap<String, Runner>,
    runs: HashMap<String, HashMap<String, RunResult>>,
}

pub fn record_results(
    results_path: &Path,
    result_file_name: Option<String>,
    results: &Results,
) -> Result<PathBuf> {
    debug!("writing all results out...");

    fs::create_dir_all(results_path)?;

    let runners: HashSet<&Runner> = results.values().flat_map(HashMap::keys).collect();

    let results_formatted = ResultsFormatted {
        benchmarks: results.keys().map(|b| (b.name.clone(), b.clone())).collect(),
        runners: runners.into_iter().map(|r| (r.name.clone(), r.clone())).collect(),
        runs: results
            .iter()
            .map(|(b, br)| {
                (b.name.clone(), br.iter().map(|(r, rr)| (r.name.clone(), rr.clone())).collect())
            })
            .collect(),
    };

    let result_file_name = result_file_name.unwrap_or_else(|| {
        format!("{}.evm-bench.results.json", chrono::offset::Utc::now().to_rfc3339())
    });
    let result_file_path = results_path.join(result_file_name);
    {
        let file = fs::File::create_new(&result_file_path)?;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &results_formatted)?;
        writer.flush()?;
    }

    info!("wrote out results to {}", result_file_path.display());
    Ok(result_file_path)
}

pub fn print_results(results_file_path: &Path) -> Result<()> {
    info!("reading and parsing results from {}...", results_file_path.display());
    let results =
        serde_json::from_str::<ResultsFormatted>(&fs::read_to_string(results_file_path)?)?;
    debug!("read and parsed results from {}", results_file_path.display());

    let mut runner_names: Vec<_> = results.runners.keys().cloned().collect();
    runner_names.sort();

    let mut runs = results.runs.into_iter().collect::<Vec<_>>();
    runs.sort_by_key(|(b, _)| b.clone());

    let mut runner_times = HashMap::<String, Vec<Duration>>::new();
    for (run_name, benchmark_runs) in &runs {
        for runner_name in &runner_names {
            let Some(run) = benchmark_runs.get(runner_name) else {
                warn!("no runs for {run_name}/{runner_name}");
                continue;
            };
            let avg_run_time =
                run.run_times.iter().sum::<Duration>().div_f64(run.run_times.len() as f64);
            runner_times.entry(runner_name.clone()).or_default().push(avg_run_time);
        }
    }
    runner_names.sort_by_key(|name| runner_times[name].iter().sum::<Duration>());

    let mut builder = Builder::default();

    builder.push_record(std::iter::once("").chain(runner_names.iter().map(String::as_str)));

    let average_runner_times = runner_times
        .iter()
        .map(|(name, times)| (name.clone(), times.iter().sum::<Duration>()))
        .collect::<HashMap<String, Duration>>();
    let mut record = vec!["**sum**".to_string()];
    record.extend(
        runner_names
            .iter()
            .map(|runner_name| average_runner_times.get(runner_name))
            .map(|val| Some(format!("{:>9.3?}", val?)))
            .map(|s| s.unwrap_or_default()),
    );
    builder.push_record(record);

    let min_runner_time = average_runner_times.values().min().unwrap();
    let mut record = vec!["**relative**".to_string()];
    record.extend(
        runner_names
            .iter()
            .map(|runner_name| {
                Some(
                    average_runner_times.get(runner_name)?.as_secs_f64()
                        / min_runner_time.as_secs_f64(),
                )
            })
            .map(|val| Some(format!("{:>9.3?}x", val?)))
            .map(|s| s.unwrap_or_default()),
    );
    builder.push_record(record);

    for (benchmark_name, benchmark_runs) in runs.iter() {
        let vals = runner_names.iter().map(|runner_name| {
            let run = benchmark_runs.get(runner_name)?;
            let avg_run_time =
                run.run_times.iter().sum::<Duration>().div_f64(run.run_times.len() as f64);
            runner_times.entry(runner_name.clone()).or_default().push(avg_run_time);
            Some(avg_run_time)
        });

        let mut record = vec![benchmark_name.clone()];
        record
            .extend(vals.map(|val| Some(format!("{:>9.3?}", val?))).map(|s| s.unwrap_or_default()));
        builder.push_record(record);
    }

    let mut table = builder.build();
    table.with(Style::markdown());
    println!("{}", table);

    Ok(())
}
