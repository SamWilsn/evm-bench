import argparse
import pathlib
import time
from typing import Final

import pyrevm

GAS_LIMIT: Final[int] = 1_000_000_000
ZERO_ADDRESS: Final[str] = "0x0000000000000000000000000000000000000000"

CALLER_ADDRESS: Final[str] = "0x1000000000000000000000000000000000000001"


def _load_contract_data(data_file_path: pathlib.Path) -> bytes:
    assert data_file_path is not None, "Contract code path is required"
    with open(data_file_path, mode="r") as file:
        return bytes.fromhex(file.read())


def _construct_evm() -> pyrevm.EVM:
    evm = pyrevm.EVM()
    return evm


def _benchmark(
    evm: pyrevm.EVM,
    caller_address: str,
    contract_data: bytes,
    call_data: bytes,
    num_runs: int,
) -> None:
    contract_address = evm.deploy(
        deployer=caller_address,
        code=contract_data,
        gas=GAS_LIMIT,
    )
    assert evm.result.is_success, evm.result

    def bench() -> None:
        evm.message_call(
            caller=caller_address,
            to=contract_address,
            calldata=call_data,
            gas=GAS_LIMIT,
        )

    for _ in range(num_runs):
        start = time.perf_counter_ns()
        bench()
        end = time.perf_counter_ns()
        print((end - start) / 1e6)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract-code-path", type=pathlib.Path)
    parser.add_argument("--calldata", type=str)
    parser.add_argument("--num-runs", type=int)
    return parser.parse_args()


def main() -> None:
    args = parse_args()

    contract_data = _load_contract_data(args.contract_code_path)
    evm = _construct_evm()

    _benchmark(
        evm,
        caller_address=CALLER_ADDRESS,
        contract_data=contract_data,
        call_data=bytes.fromhex(args.calldata),
        num_runs=args.num_runs,
    )


if __name__ == "__main__":
    main()
