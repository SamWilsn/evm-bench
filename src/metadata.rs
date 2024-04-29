use alloy_primitives::Bytes;
use color_eyre::eyre::{ensure, Result};
use glob::glob;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub trait MetadataParser
where
    Self: Sized,
{
    type Defaults;

    fn parse_schema_from_file(schema_path: &Path) -> Result<serde_json::Value> {
        let schema = fs::read_to_string(schema_path)?;
        Ok(serde_json::from_str(&schema)?)
    }

    fn parse_from_file(
        schema: &serde_json::Value,
        json_path: &Path,
        defaults: &Self::Defaults,
    ) -> Result<Self> {
        let json = fs::read_to_string(json_path)?;
        let json = serde_json::from_str(&json)?;
        Self::parse(json_path.parent().unwrap(), schema, json, defaults)
    }

    fn parse(
        base_path: &Path,
        schema: &serde_json::Value,
        json: serde_json::Value,
        defaults: &Self::Defaults,
    ) -> Result<Self> {
        ensure!(jsonschema::is_valid(schema, &json), "JSON does not abide by the schema");
        Self::parse_inner(base_path, json, defaults)
    }

    fn parse_inner(
        base_path: &Path,
        json: serde_json::Value,
        defaults: &Self::Defaults,
    ) -> Result<Self>;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Benchmark {
    pub name: String,
    pub solc_version: String,
    pub num_runs: u64,
    pub contract: PathBuf,
    pub build_context: PathBuf,
    pub calldata: Bytes,
}

pub struct BenchmarkDefaults {
    pub solc_version: String,
    pub num_runs: u64,
    pub calldata: Bytes,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PartialBenchmark {
    pub name: String,
    #[serde(default)]
    pub solc_version: Option<String>,
    #[serde(default)]
    pub num_runs: Option<u64>,
    pub contract: PathBuf,
    #[serde(default)]
    pub build_context: Option<PathBuf>,
    #[serde(default)]
    pub calldata: Option<Bytes>,
}

impl PartialBenchmark {
    fn resolve(self, base_path: &Path, defaults: &BenchmarkDefaults) -> Result<Benchmark> {
        Ok(Benchmark {
            name: self.name,
            solc_version: self.solc_version.unwrap_or_else(|| defaults.solc_version.clone()),
            num_runs: self.num_runs.unwrap_or(defaults.num_runs),
            contract: base_path.join(&self.contract).canonicalize()?,
            build_context: match self.build_context {
                Some(context) => base_path.join(context).canonicalize()?,
                None => base_path.to_path_buf(),
            },
            calldata: self.calldata.unwrap_or_else(|| defaults.calldata.clone()),
        })
    }
}

impl MetadataParser for Benchmark {
    type Defaults = BenchmarkDefaults;

    fn parse_inner(
        base_path: &Path,
        json: serde_json::Value,
        defaults: &Self::Defaults,
    ) -> Result<Self> {
        trace!("parsing benchmark metadata...");
        let partial: PartialBenchmark = serde_json::from_value(json)?;
        let benchmark = partial.resolve(base_path, defaults)?;
        debug!("parsed benchmark metadata: {}", &benchmark.name);
        trace!("benchmark metadata: {:?}", benchmark);
        Ok(benchmark)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Runner {
    pub name: String,
    pub entry: PathBuf,
}

impl MetadataParser for Runner {
    type Defaults = ();

    fn parse_inner(base_path: &Path, json: serde_json::Value, &(): &()) -> Result<Self> {
        trace!("parsing runner metadata...");
        let mut runner: Runner = serde_json::from_value(json)?;
        runner.entry = base_path.join(&runner.entry).canonicalize()?;
        debug!("parsed runner metadata: {}", &runner.name);
        trace!("runner metadata: {:?}", runner);
        Ok(runner)
    }
}

fn find_metadata<T: MetadataParser>(
    file_name: &str,
    schema_path: &Path,
    search_path: &Path,
    defaults: T::Defaults,
) -> Result<Vec<T>> {
    let schema = Benchmark::parse_schema_from_file(schema_path)?;

    let search_path = search_path.canonicalize()?;
    ensure!(search_path.is_dir(), "{} is not a directory", search_path.display());

    let glob_path = search_path.join("**").join(file_name);
    let glob_string = glob_path.to_string_lossy();
    debug!("globbing in {glob_string}");
    Ok(glob(&glob_string)?
        .flat_map(|entry| entry.inspect_err(|e| warn!("error globbing {glob_string}: {e}")).ok())
        .flat_map(|path| {
            let stripped_path = path.strip_prefix(&search_path).unwrap_or(&path);
            debug!("found {}", stripped_path.display());
            match T::parse_from_file(&schema, &path, &defaults) {
                Ok(res) => {
                    debug!("parsed {}", stripped_path.display());
                    Some(res)
                }
                Err(e) => {
                    warn!("error parsing {}: {e}", stripped_path.display());
                    None
                }
            }
        })
        .collect())
}

pub fn find_benchmarks(
    file_name: &str,
    schema_path: &Path,
    search_path: &Path,
    benchmark_defaults: BenchmarkDefaults,
) -> Result<Vec<Benchmark>> {
    let benchmarks =
        find_metadata::<Benchmark>(file_name, schema_path, search_path, benchmark_defaults)?;
    ensure!(!benchmarks.is_empty(), "no benchmarks found");

    let benchmark_names = benchmarks.iter().map(|b| &b.name).collect::<HashSet<_>>();
    ensure!(benchmarks.len() == benchmark_names.len(), "found duplicate benchmark names");

    info!("found {} benchmarks: {}", benchmarks.len(), benchmark_names.iter().format(", "));
    Ok(benchmarks)
}

pub fn find_runners(
    file_name: &str,
    schema_path: &Path,
    search_path: &Path,
    runner_defaults: (),
) -> Result<Vec<Runner>> {
    let runners = find_metadata::<Runner>(file_name, schema_path, search_path, runner_defaults)?;
    ensure!(!runners.is_empty(), "no runners found");

    let runner_names = runners.iter().map(|b| &b.name).collect::<HashSet<_>>();
    ensure!(runners.len() == runner_names.len(), "found duplicate runners names");

    info!("found {} runners: {}", runners.len(), runner_names.iter().format(", "));
    Ok(runners)
}
