// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8.17;

contract TenThousandHashes {
    function Benchmark() external pure returns (bytes32 acc) {
        for (uint256 i = 0; i < 10000; i++) {
            acc = keccak256(abi.encodePacked(acc));
        }
    }
}
