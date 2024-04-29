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

        let mut table = Table::new();
        table.load_preset(presets::ASCII_MARKDOWN);

        // Header.
        {
            let header = runner_names.iter().map(|s| s.as_str());
            let mut cells = Cells::from(iter::once("").chain(header));
            for cell in &mut cells.0 {
                *cell = std::mem::replace(cell, Cell::new("")).set_alignment(CellAlignment::Center);
            }
            table.set_header(cells);
        }

        let average_runner_times = runner_times
            .iter()
            .map(|(name, times)| (name, times.iter().sum::<Duration>()))
            .collect::<HashMap<_, _>>();
        // Sum of all times.
        {
            let row = runner_names
                .iter()
                .map(|&runner_name| average_runner_times.get(runner_name))
                .map(|val: Option<&Duration>| Some(format!("{:.3?}", val?)))
                .map(|s| s.unwrap_or_default());
            table.add_row(iter::once("**sum**".to_string()).chain(row));
        }

        // Relative times.
        {
            let min_runner_time =
                average_runner_times.values().min().copied().unwrap_or(Duration::from_secs(1));
            let row = runner_names
                .iter()
                .map(|&name| {
                    average_runner_times.get(name).map(|time| {
                        format!("{:.3?}x", time.as_secs_f64() / min_runner_time.as_secs_f64())
                    })
                })
                .map(Option::unwrap_or_default);
            table.add_row(iter::once("**relative**".to_string()).chain(row));
        }

        // Individual runs.
        for &(benchmark_name, benchmark_runs) in runs.iter() {
            let vals = runner_names.iter().map(|&runner_name| {
                let run = benchmark_runs.get(runner_name)?;
                let avg_run_time =
                    run.run_times.iter().sum::<Duration>().div_f64(run.run_times.len() as f64);
                runner_times.entry(runner_name.clone()).or_default().push(avg_run_time);
                Some(avg_run_time)
            });

            let row = vals.map(|val| val.map(|time| format!("{time:.3?}")).unwrap_or_default());
            table.add_row(iter::once(benchmark_name.clone()).chain(row));
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
        let s = r#"{"benchmarks":{"erc20.mint":{"name":"erc20.mint","solc_version":"stable","num_runs":5,"contract":"$ROOT/benchmarks/erc20/mint/ERC20Mint.sol","build_context":"$ROOT/benchmarks/erc20","calldata":"0x30627b7c"},"snailtracer":{"name":"snailtracer","solc_version":"0.4.26","num_runs":1,"contract":"$ROOT/benchmarks/snailtracer/SnailTracer.sol","build_context":"$ROOT/benchmarks/snailtracer","calldata":"0x30627b7c"},"ten-thousand-hashes":{"name":"ten-thousand-hashes","solc_version":"stable","num_runs":5,"contract":"$ROOT/benchmarks/ten-thousand-hashes/TenThousandHashes.sol","build_context":"$ROOT/benchmarks/ten-thousand-hashes","calldata":"0x30627b7c"},"erc20.transfer":{"name":"erc20.transfer","solc_version":"stable","num_runs":5,"contract":"$ROOT/benchmarks/erc20/transfer/ERC20Transfer.sol","build_context":"$ROOT/benchmarks/erc20","calldata":"0x30627b7c"},"erc20.approval-transfer":{"name":"erc20.approval-transfer","solc_version":"stable","num_runs":5,"contract":"$ROOT/benchmarks/erc20/approval-transfer/ERC20ApprovalTransfer.sol","build_context":"$ROOT/benchmarks/erc20","calldata":"0x30627b7c"}},"runners":{"revm":{"name":"revm","entry":"$ROOT/runners/revm/entry.sh"},"pyrevm":{"name":"pyrevm","entry":"$ROOT/runners/pyrevm/entry.sh"},"py-evm.pypy":{"name":"py-evm.pypy","entry":"$ROOT/runners/py-evm/pypy/entry.sh"},"py-evm.cpython":{"name":"py-evm.cpython","entry":"$ROOT/runners/py-evm/cpython/entry.sh"},"ethereumjs":{"name":"ethereumjs","entry":"$ROOT/runners/ethereumjs/entry.sh"},"evmone":{"name":"evmone","entry":"$ROOT/runners/evmone/entry.sh"},"geth":{"name":"geth","entry":"$ROOT/runners/geth/entry.sh"}},"runs":{"erc20.transfer":{"ethereumjs":{"run_times":[{"secs":0,"nanos":595451391},{"secs":0,"nanos":576946801},{"secs":0,"nanos":570141738},{"secs":0,"nanos":569878842},{"secs":0,"nanos":562415831}]},"evmone":{"run_times":[{"secs":0,"nanos":5327950},{"secs":0,"nanos":5102470},{"secs":0,"nanos":5184160},{"secs":0,"nanos":5071660},{"secs":0,"nanos":5093910}]},"geth":{"run_times":[{"secs":0,"nanos":21826000},{"secs":0,"nanos":20110000},{"secs":0,"nanos":19655000},{"secs":0,"nanos":20432000},{"secs":0,"nanos":20542000}]},"pyrevm":{"run_times":[{"secs":0,"nanos":8328796},{"secs":0,"nanos":8165877},{"secs":0,"nanos":7837010},{"secs":0,"nanos":7751586},{"secs":0,"nanos":10080772}]},"py-evm.pypy":{"run_times":[{"secs":0,"nanos":663633186},{"secs":0,"nanos":100202777},{"secs":0,"nanos":129912799},{"secs":0,"nanos":97360300},{"secs":0,"nanos":100479148}]},"revm":{"run_times":[{"secs":0,"nanos":5492564},{"secs":0,"nanos":5126171},{"secs":0,"nanos":5263780},{"secs":0,"nanos":5084653},{"secs":0,"nanos":5096012}]},"py-evm.cpython":{"run_times":[{"secs":0,"nanos":670503052},{"secs":0,"nanos":685093042},{"secs":0,"nanos":680752008},{"secs":0,"nanos":684123476},{"secs":0,"nanos":693111643}]}},"ten-thousand-hashes":{"py-evm.pypy":{"run_times":[{"secs":0,"nanos":310507406},{"secs":0,"nanos":60379774},{"secs":0,"nanos":57932332},{"secs":0,"nanos":80748299},{"secs":0,"nanos":54078439}]},"py-evm.cpython":{"run_times":[{"secs":0,"nanos":448843086},{"secs":0,"nanos":444881060},{"secs":0,"nanos":441763728},{"secs":0,"nanos":442264499},{"secs":0,"nanos":443060202}]},"pyrevm":{"run_times":[{"secs":0,"nanos":3525772},{"secs":0,"nanos":3450557},{"secs":0,"nanos":3348263},{"secs":0,"nanos":3291325},{"secs":0,"nanos":3280306}]},"ethereumjs":{"run_times":[{"secs":0,"nanos":259935437},{"secs":0,"nanos":248871553},{"secs":0,"nanos":243637159},{"secs":0,"nanos":246385007},{"secs":0,"nanos":246527254}]},"geth":{"run_times":[{"secs":0,"nanos":10305000},{"secs":0,"nanos":9822000},{"secs":0,"nanos":9849000},{"secs":0,"nanos":10478000},{"secs":0,"nanos":11173000}]},"evmone":{"run_times":[{"secs":0,"nanos":2965440},{"secs":0,"nanos":2873140},{"secs":0,"nanos":2662000},{"secs":0,"nanos":2743540},{"secs":0,"nanos":3663830}]},"revm":{"run_times":[{"secs":0,"nanos":3358962},{"secs":0,"nanos":3453596},{"secs":0,"nanos":3194841},{"secs":0,"nanos":3203581},{"secs":0,"nanos":3163063}]}},"erc20.mint":{"py-evm.cpython":{"run_times":[{"secs":0,"nanos":474708248},{"secs":0,"nanos":479187710},{"secs":0,"nanos":473000755},{"secs":0,"nanos":470173888},{"secs":0,"nanos":469321850}]},"py-evm.pypy":{"run_times":[{"secs":0,"nanos":608120606},{"secs":0,"nanos":68161369},{"secs":0,"nanos":69981356},{"secs":0,"nanos":95211630},{"secs":0,"nanos":69322679}]},"pyrevm":{"run_times":[{"secs":0,"nanos":5328567},{"secs":0,"nanos":5261634},{"secs":0,"nanos":5083386},{"secs":0,"nanos":4969432},{"secs":0,"nanos":4948411}]},"revm":{"run_times":[{"secs":0,"nanos":3116352},{"secs":0,"nanos":2735126},{"secs":0,"nanos":2713626},{"secs":0,"nanos":2682024},{"secs":0,"nanos":2661823}]},"evmone":{"run_times":[{"secs":0,"nanos":3679190},{"secs":0,"nanos":2801370},{"secs":0,"nanos":2838230},{"secs":0,"nanos":3191520},{"secs":0,"nanos":2749040}]},"geth":{"run_times":[{"secs":0,"nanos":15492000},{"secs":0,"nanos":13582000},{"secs":0,"nanos":14501000},{"secs":0,"nanos":14682000},{"secs":0,"nanos":14671000}]},"ethereumjs":{"run_times":[{"secs":0,"nanos":470614560},{"secs":0,"nanos":459002884},{"secs":0,"nanos":443231753},{"secs":0,"nanos":444097641},{"secs":0,"nanos":438044964}]}},"snailtracer":{"geth":{"run_times":[{"secs":0,"nanos":148981000}]},"revm":{"run_times":[{"secs":0,"nanos":34953920}]},"py-evm.cpython":{"run_times":[{"secs":8,"nanos":486977188}]},"py-evm.pypy":{"run_times":[{"secs":1,"nanos":953374715}]},"ethereumjs":{"run_times":[{"secs":4,"nanos":186305557}]},"evmone":{"run_times":[{"secs":0,"nanos":25656300}]},"pyrevm":{"run_times":[{"secs":0,"nanos":37122468}]}},"erc20.approval-transfer":{"py-evm.pypy":{"run_times":[{"secs":0,"nanos":655602340},{"secs":0,"nanos":134665186},{"secs":0,"nanos":80479541},{"secs":0,"nanos":82446442},{"secs":0,"nanos":83133801}]},"revm":{"run_times":[{"secs":0,"nanos":4539161},{"secs":0,"nanos":5307885},{"secs":0,"nanos":4506030},{"secs":0,"nanos":6185761},{"secs":0,"nanos":4463649}]},"pyrevm":{"run_times":[{"secs":0,"nanos":6497804},{"secs":0,"nanos":6183122},{"secs":0,"nanos":6129618},{"secs":0,"nanos":6086518},{"secs":0,"nanos":6054345}]},"ethereumjs":{"run_times":[{"secs":0,"nanos":407679168},{"secs":0,"nanos":365384613},{"secs":0,"nanos":363423684},{"secs":0,"nanos":365881312},{"secs":0,"nanos":359681715}]},"evmone":{"run_times":[{"secs":0,"nanos":4482580},{"secs":0,"nanos":4451460},{"secs":0,"nanos":4296120},{"secs":0,"nanos":4291810},{"secs":0,"nanos":4322720}]},"py-evm.cpython":{"run_times":[{"secs":0,"nanos":470308903},{"secs":0,"nanos":467006699},{"secs":0,"nanos":477113127},{"secs":0,"nanos":457630123},{"secs":0,"nanos":451851282}]},"geth":{"run_times":[{"secs":0,"nanos":14790000},{"secs":0,"nanos":13953000},{"secs":0,"nanos":13494000},{"secs":0,"nanos":22779000},{"secs":0,"nanos":16174000}]}}}}"#;
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
            |         **sum**         | 41.215ms | 51.224ms | 60.243ms | 210.643ms |      2.674s |     5.834s |        10.552s |
            |       **relative**      |   1.000x |   1.243x |   1.462x |    5.111x |     64.876x |   141.545x |       256.023x |
            | erc20.approval-transfer |  4.369ms |  5.000ms |  6.190ms |  16.238ms |   207.265ms |  372.410ms |      464.782ms |
            |        erc20.mint       |  3.052ms |  2.782ms |  5.118ms |  14.586ms |   182.160ms |  450.998ms |      473.278ms |
            |      erc20.transfer     |  5.156ms |  5.213ms |  8.433ms |  20.513ms |   218.318ms |  574.967ms |      682.717ms |
            |       snailtracer       | 25.656ms | 34.954ms | 37.122ms | 148.981ms |      1.953s |     4.186s |         8.487s |
            |   ten-thousand-hashes   |  2.982ms |  3.275ms |  3.379ms |  10.325ms |   112.729ms |  249.071ms |      444.163ms |"#]];
        expect.assert_eq(&example_results().table().to_string());
    }
}
