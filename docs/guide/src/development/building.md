# Building from Source

AWS Lambda Web Adapter is written in Rust and based on the [AWS Lambda Rust Runtime](https://github.com/aws/aws-lambda-rust-runtime).

## Prerequisites

- [Rust toolchain](https://rustup.rs/)
- [cargo-lambda](https://www.cargo-lambda.info/guide/installation.html)

## Clone the Repository

```bash
git clone https://github.com/aws/aws-lambda-web-adapter.git
cd aws-lambda-web-adapter
```

## Build with cargo-lambda

[cargo-lambda](https://www.cargo-lambda.info/) handles cross-compilation for Lambda targets automatically — no need to install cross-compiler toolchains manually.

Build for x86_64:

```bash
cargo lambda build --release --extension --target x86_64-unknown-linux-musl
```

Build for arm64:

```bash
cargo lambda build --release --extension --target aarch64-unknown-linux-musl
```

The compiled extension is placed under `target/lambda/extensions/`.

## Package as Docker Image

After building with cargo-lambda, you can package the binary into a minimal container:

```bash
# x86_64
printf 'FROM scratch\nADD target/lambda/extensions/. /\n' | docker build --platform=linux/amd64 -t aws-lambda-adapter:latest-x86_64 -f- .

# arm64
printf 'FROM scratch\nADD target/lambda/extensions/. /\n' | docker build --platform=linux/arm64 -t aws-lambda-adapter:latest-aarch64 -f- .
```

Or use the Makefile targets which combine both steps:

```bash
make build-image-x86
make build-image-arm64
```

## Running Tests

```bash
cargo fmt -- --check
cargo clippy -- -Dwarnings
cargo nextest run
```
