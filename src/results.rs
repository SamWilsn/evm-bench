use crate::{
    metadata::{Benchmark, Runner},
    run::{Results, RunResult},
};
use color_eyre::eyre::Result;
use comfy_table::{presets, Cell, CellAlignment, Cells, Table};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    iter,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultsFormatted {
    /// benchmark.name -> benchmark
    benchmarks: HashMap<String, Benchmark>,
    /// runner.name -> runner
    runners: HashMap<String, Runner>,
    /// runner.name -> benchmark.name -> result
    runs: HashMap<String, HashMap<String, RunResult>>,
}

impl ResultsFormatted {
    pub fn new(results: &Results) -> Self {
        Self {
            benchmarks: results
                .values()
                .flat_map(HashMap::keys)
                .map(|b| (b.name.clone(), b.clone()))
                .collect(),
            runners: results.keys().map(|r| (r.name.clone(), r.clone())).collect(),
            runs: results
                .iter()
                .map(|(r, br)| {
                    (
                        r.name.clone(),
                        br.iter().map(|(b, rr)| (b.name.clone(), rr.clone())).collect(),
                    )
                })
                .collect(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        info!("reading and parsing results from {} ...", path.display());
        let s = fs::read_to_string(path)?;
        let results = serde_json::from_str::<ResultsFormatted>(&s)?;
        debug!("read and parsed results from {}", path.display());
        Ok(results)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let file = fs::File::create_new(path)?;
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
        let mut runner_times = self
            .runs
            .iter()
            .map(|(runner_name, benchmark_runs)| {
                let mut avg =
                    benchmark_runs.values().flat_map(|run| run.average()).sum::<Duration>();
                if avg == Duration::default() {
                    avg = Duration::from_secs(999);
                }
                (runner_name, avg)
            })
            .collect::<Vec<_>>();
        runner_times.sort_by_key(|(_, time)| *time);
        let runner_names = || runner_times.iter().map(|(name, _)| *name);
        let average_runner_times = || runner_times.iter().map(|(_, time)| time);

        let mut table = Table::new();
        table.load_preset(presets::ASCII_MARKDOWN);

        // Header.
        {
            let header = runner_names().map(String::as_str);
            let mut cells = Cells::from(iter::once("").chain(header));
            for cell in &mut cells.0 {
                *cell = std::mem::replace(cell, Cell::new("")).set_alignment(CellAlignment::Center);
            }
            table.set_header(cells);
        }

        // Sum of all average times.
        {
            let row = average_runner_times().map(|time| format!("{time:.3?}"));
            table.add_row(iter::once("**sum**".to_string()).chain(row));
        }

        // Relative times.
        {
            let min = average_runner_times().min();
            let min = min.map(|min| min.as_secs_f64()).unwrap_or(1.0);
            let row = average_runner_times().map(|t| format!("{:.3?}x", t.as_secs_f64() / min));
            table.add_row(iter::once("**relative**".to_string()).chain(row));
        }

        // Individual runs.
        let mut benchmark_names: Vec<_> = self.benchmarks.keys().collect();
        benchmark_names.sort();
        for &benchmark_name in &benchmark_names {
            let mut row = Vec::with_capacity(self.runners.len() + 1);
            row.push(benchmark_name.clone());
            for runner_name in runner_names() {
                let run = &self.runs[runner_name][benchmark_name];
                let time = run.average().map(|time| format!("{time:.3?}")).unwrap_or_default();
                row.push(time);
            }
            table.add_row(row);
        }

        let mut columns = table.column_iter_mut();
        columns.next().unwrap().set_cell_alignment(CellAlignment::Center);
        for column in columns {
            column.set_cell_alignment(CellAlignment::Right);
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    fn example_results() -> ResultsFormatted {
        let s = r#"{"benchmarks":{"snailtracer":{"name":"snailtracer","solc_version":"0.4.26","num_runs":1,"contract":"/home/doni/github/danipopes/evm-bench/benchmarks/snailtracer/SnailTracer.sol","build_context":"/home/doni/github/danipopes/evm-bench/benchmarks/snailtracer","calldata":"0x30627b7c"},"erc20.approval-transfer":{"name":"erc20.approval-transfer","solc_version":"stable","num_runs":5,"contract":"/home/doni/github/danipopes/evm-bench/benchmarks/erc20/approval-transfer/ERC20ApprovalTransfer.sol","build_context":"/home/doni/github/danipopes/evm-bench/benchmarks/erc20","calldata":"0x30627b7c"},"erc20.mint":{"name":"erc20.mint","solc_version":"stable","num_runs":5,"contract":"/home/doni/github/danipopes/evm-bench/benchmarks/erc20/mint/ERC20Mint.sol","build_context":"/home/doni/github/danipopes/evm-bench/benchmarks/erc20","calldata":"0x30627b7c"},"ten-thousand-hashes":{"name":"ten-thousand-hashes","solc_version":"stable","num_runs":5,"contract":"/home/doni/github/danipopes/evm-bench/benchmarks/ten-thousand-hashes/TenThousandHashes.sol","build_context":"/home/doni/github/danipopes/evm-bench/benchmarks/ten-thousand-hashes","calldata":"0x30627b7c"},"erc20.transfer":{"name":"erc20.transfer","solc_version":"stable","num_runs":5,"contract":"/home/doni/github/danipopes/evm-bench/benchmarks/erc20/transfer/ERC20Transfer.sol","build_context":"/home/doni/github/danipopes/evm-bench/benchmarks/erc20","calldata":"0x30627b7c"}},"runners":{"py-evm.pypy":{"name":"py-evm.pypy","entry":"/home/doni/github/danipopes/evm-bench/runners/py-evm/pypy/entry.sh"},"py-evm.cpython":{"name":"py-evm.cpython","entry":"/home/doni/github/danipopes/evm-bench/runners/py-evm/cpython/entry.sh"},"revm":{"name":"revm","entry":"/home/doni/github/danipopes/evm-bench/runners/revm/entry.sh"},"geth":{"name":"geth","entry":"/home/doni/github/danipopes/evm-bench/runners/geth/entry.sh"},"pyrevm":{"name":"pyrevm","entry":"/home/doni/github/danipopes/evm-bench/runners/pyrevm/entry.sh"},"ethereumjs":{"name":"ethereumjs","entry":"/home/doni/github/danipopes/evm-bench/runners/ethereumjs/entry.sh"},"evmone":{"name":"evmone","entry":"/home/doni/github/danipopes/evm-bench/runners/evmone/entry.sh"}},"runs":{"pyrevm":{"erc20.approval-transfer":{"run_times":[{"secs":0,"nanos":6213942},{"secs":0,"nanos":6126792},{"secs":0,"nanos":5973983},{"secs":0,"nanos":5851282},{"secs":0,"nanos":5906892}]},"ten-thousand-hashes":{"run_times":[{"secs":0,"nanos":3496145},{"secs":0,"nanos":3477746},{"secs":0,"nanos":3316006},{"secs":0,"nanos":3304595},{"secs":0,"nanos":3309506}]},"erc20.mint":{"run_times":[{"secs":0,"nanos":5259613},{"secs":0,"nanos":5173433},{"secs":0,"nanos":4930204},{"secs":0,"nanos":4880953},{"secs":0,"nanos":4854174}]},"snailtracer":{"run_times":[{"secs":0,"nanos":38593150}]},"erc20.transfer":{"run_times":[{"secs":0,"nanos":8016020},{"secs":0,"nanos":7953489},{"secs":0,"nanos":7624840},{"secs":0,"nanos":7581321},{"secs":0,"nanos":7709040}]}},"py-evm.pypy":{"erc20.transfer":{"run_times":[{"secs":0,"nanos":666712250},{"secs":0,"nanos":113529972},{"secs":0,"nanos":115125600},{"secs":0,"nanos":97980862},{"secs":0,"nanos":97463883}]},"ten-thousand-hashes":{"run_times":[{"secs":0,"nanos":245686551},{"secs":0,"nanos":72411196},{"secs":0,"nanos":84104591},{"secs":0,"nanos":57956555},{"secs":0,"nanos":55027828}]},"erc20.mint":{"run_times":[{"secs":0,"nanos":602120593},{"secs":0,"nanos":68984960},{"secs":0,"nanos":66633063},{"secs":0,"nanos":92249950},{"secs":0,"nanos":67860521}]},"erc20.approval-transfer":{"run_times":[{"secs":0,"nanos":704793647},{"secs":0,"nanos":86125888},{"secs":0,"nanos":104097935},{"secs":0,"nanos":75341292},{"secs":0,"nanos":70642277}]},"snailtracer":{"run_times":[{"secs":2,"nanos":37957059}]}},"py-evm.cpython":{"erc20.transfer":{"run_times":[{"secs":0,"nanos":695363341},{"secs":0,"nanos":699074336},{"secs":0,"nanos":698104309},{"secs":0,"nanos":693895863},{"secs":0,"nanos":682253360}]},"snailtracer":{"run_times":[{"secs":7,"nanos":836818776}]},"ten-thousand-hashes":{"run_times":[{"secs":0,"nanos":449524650},{"secs":0,"nanos":443475368},{"secs":0,"nanos":441195272},{"secs":0,"nanos":443785247},{"secs":0,"nanos":442646500}]},"erc20.approval-transfer":{"run_times":[{"secs":0,"nanos":457147154},{"secs":0,"nanos":460718788},{"secs":0,"nanos":454535988},{"secs":0,"nanos":455645835},{"secs":0,"nanos":456859195}]},"erc20.mint":{"run_times":[{"secs":0,"nanos":496261353},{"secs":0,"nanos":500230198},{"secs":0,"nanos":489338882},{"secs":0,"nanos":490074021},{"secs":0,"nanos":491134510}]}},"geth":{"snailtracer":{"run_times":[{"secs":0,"nanos":148105000}]},"erc20.approval-transfer":{"run_times":[{"secs":0,"nanos":14793000},{"secs":0,"nanos":14032000},{"secs":0,"nanos":13466000},{"secs":0,"nanos":15026000},{"secs":0,"nanos":16541000}]},"erc20.mint":{"run_times":[{"secs":0,"nanos":14717000},{"secs":0,"nanos":13011000},{"secs":0,"nanos":13830000},{"secs":0,"nanos":14612000},{"secs":0,"nanos":15242000}]},"erc20.transfer":{"run_times":[{"secs":0,"nanos":24250000},{"secs":0,"nanos":20038000},{"secs":0,"nanos":19945000},{"secs":0,"nanos":19619000},{"secs":0,"nanos":19804000}]},"ten-thousand-hashes":{"run_times":[{"secs":0,"nanos":11379000},{"secs":0,"nanos":9792000},{"secs":0,"nanos":10106000},{"secs":0,"nanos":9665000},{"secs":0,"nanos":9481000}]}},"ethereumjs":{"erc20.approval-transfer":{"run_times":[{"secs":0,"nanos":410379118},{"secs":0,"nanos":387112689},{"secs":0,"nanos":373439418},{"secs":0,"nanos":375011307},{"secs":0,"nanos":382830976}]},"snailtracer":{"run_times":[{"secs":4,"nanos":341908667}]},"erc20.mint":{"run_times":[{"secs":0,"nanos":481914363},{"secs":0,"nanos":463607198},{"secs":0,"nanos":450915694},{"secs":0,"nanos":447671290},{"secs":0,"nanos":456229468}]},"erc20.transfer":{"run_times":[{"secs":0,"nanos":624806792},{"secs":0,"nanos":614366697},{"secs":0,"nanos":607304596},{"secs":0,"nanos":597486090},{"secs":0,"nanos":590809629}]},"ten-thousand-hashes":{"run_times":[{"secs":0,"nanos":272472237},{"secs":0,"nanos":254026711},{"secs":0,"nanos":251613454},{"secs":0,"nanos":252607793},{"secs":0,"nanos":251813734}]}},"evmone":{"ten-thousand-hashes":{"run_times":[{"secs":0,"nanos":2862040},{"secs":0,"nanos":2841480},{"secs":0,"nanos":2919490},{"secs":0,"nanos":2669210},{"secs":0,"nanos":2672860}]},"snailtracer":{"run_times":[{"secs":0,"nanos":25782900}]},"erc20.mint":{"run_times":[{"secs":0,"nanos":3120940},{"secs":0,"nanos":3040780},{"secs":0,"nanos":2993980},{"secs":0,"nanos":2972430},{"secs":0,"nanos":2870990}]},"erc20.approval-transfer":{"run_times":[{"secs":0,"nanos":4516020},{"secs":0,"nanos":4600520},{"secs":0,"nanos":4452840},{"secs":0,"nanos":4167770},{"secs":0,"nanos":4404270}]},"erc20.transfer":{"run_times":[{"secs":0,"nanos":5943050},{"secs":0,"nanos":5271690},{"secs":0,"nanos":5276930},{"secs":0,"nanos":5360110},{"secs":0,"nanos":5354380}]}},"revm":{"erc20.mint":{"run_times":[{"secs":0,"nanos":2915256},{"secs":0,"nanos":3021266},{"secs":0,"nanos":2622537},{"secs":0,"nanos":3050286},{"secs":0,"nanos":2648576}]},"erc20.approval-transfer":{"run_times":[{"secs":0,"nanos":4684525},{"secs":0,"nanos":4502454},{"secs":0,"nanos":4491694},{"secs":0,"nanos":4355524},{"secs":0,"nanos":4363034}]},"snailtracer":{"run_times":[{"secs":0,"nanos":32540318}]},"erc20.transfer":{"run_times":[{"secs":0,"nanos":5376304},{"secs":0,"nanos":4967194},{"secs":0,"nanos":5044953},{"secs":0,"nanos":4948674},{"secs":0,"nanos":4948763}]},"ten-thousand-hashes":{"run_times":[{"secs":0,"nanos":3410976},{"secs":0,"nanos":3741635},{"secs":0,"nanos":3281935},{"secs":0,"nanos":3165126},{"secs":0,"nanos":3180846}]}}}}"#;
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn test_serde() {
        let results = example_results();
        let s = serde_json::to_string_pretty(&results).unwrap();
        let results2 = serde_json::from_str::<ResultsFormatted>(&s).unwrap();
        assert_eq!(results, results2);
    }

    #[test]
    fn test_table() {
        let expect = expect![[r#"
            |                         |  evmone  |   revm   |  pyrevm  |    geth   | py-evm.pypy | ethereumjs | py-evm.cpython |
            |-------------------------|----------|----------|----------|-----------|-------------|------------|----------------|
            |         **sum**         | 41.445ms | 48.285ms | 60.785ms | 207.975ms |      2.747s |     6.051s |         9.925s |
            |       **relative**      |   1.000x |   1.165x |   1.467x |    5.018x |     66.278x |   146.004x |       239.474x |
            | erc20.approval-transfer |  4.428ms |  4.479ms |  6.015ms |  14.772ms |   208.200ms |  385.755ms |      456.981ms |
            |        erc20.mint       |  3.000ms |  2.852ms |  5.020ms |  14.282ms |   179.570ms |  460.068ms |      493.408ms |
            |      erc20.transfer     |  5.441ms |  5.057ms |  7.777ms |  20.731ms |   218.163ms |  606.955ms |      693.738ms |
            |       snailtracer       | 25.783ms | 32.540ms | 38.593ms | 148.105ms |      2.038s |     4.342s |         7.837s |
            |   ten-thousand-hashes   |  2.793ms |  3.356ms |  3.381ms |  10.085ms |   103.037ms |  256.507ms |      444.125ms |"#]];
        expect.assert_eq(&example_results().table().to_string());
    }
}
