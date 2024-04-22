# Ore CLI

A command line interface for the Ore program.

## NOTE

Ore mining has been disabled until V2 is released, this client is mostly for testing purposes (or mining ORZ).

## Building

To build the Ore CLI, you will need to have the Rust programming language installed. You can install Rust by following the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

Once you have Rust installed, you can build the Ore CLI by running the following command:

```sh
cargo build --release
```

Once you have built the executable, you can run it in CPU mode using the following command:

```sh
.\target\release\ore --rpc "your_rpc_node_url" --keypair1 "keypair1.json" --priority-fee 12345 mine
```

You may also specify up to 4 additional wallets to mine with which will get bundled into the same transaction as follows:

```sh
.\target\release\ore --rpc "your_rpc_node_url" --keypair1 "keypair1.json" --keypair2 "keypair2.json" --keypair3 "keypair3.json" --keypair4 "keypair4.json" --keypair5 "keypair5.json" --priority-fee 12345 mine
```

To run the GPU mining version, you will first need to install the CUDA Toolkit from the NVIDIA site here:

https://developer.nvidia.com/cuda-downloads

and then build either the ore-linux.cu or ore-win.cu file using the following command:

```sh
nvcc ore-win.cu -o .\target\release\gpu-worker
```

Also before running the GPU version, set the following environment variable: (replace "export" with "set" in Windows environment)

```sh
export CUDA_VISIBLE_DEVICES=<GPU_INDEX>
```

And lastly, if you want to mine ORZ instead of ORE, you can compile this project using the "orz" feature as follows

```sh
cargo build --release --no-default-features --features orz
```