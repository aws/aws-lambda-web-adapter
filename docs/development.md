## How to build it?

AWS Lambda Web Adapter is written in Rust and based on [AWS Lambda Rust Runtime](https://github.com/aws/aws-lambda-rust-runtime).
AWS Lambda executes functions in x86_64 Amazon Linux Environment. We need to compile the adapter to that environment.

### Clone the repo

First, clone this repo to your local computer.

```shell
$ git clone https://github.com/aws/aws-lambda-web-adapter.git
$ cd aws-lambda-web-adapter
```

### Compiling with Docker
On x86_64 Windows, Linux and macOS, you can run one command to compile Lambda Web Adapter with docker.
The Dockerfile is [here](../Dockerfile). [AWS CLI v2](https://docs.aws.amazon.com/cli/latest/userguide/install-cliv2.html) should have been installed and configured.

```shell
$ make build
```

Once the build completes, it creates two docker images:
- "aws-lambda-adapter:latest-x86_64" for x86_64.
- "aws-lambda-adapter:latest-aarch64" for arm64.

AWS Lambda Web Adapter binary is packaged as '/opt/bootstrap' inside each docker image. "aws-lambda-adapter:latest" is tagged to the same image as "aws-lambda-adapter:latest-x86_64".

### Compiling on macOS

If you want to install rust toolchain in your Macbook and play with the source code, you can follow the steps below.
First, install [rustup](https://rustup.rs/) if you haven't done it already. Then, add targets for x86_86 and arm64:

```shell
$ rustup target add x86_64-unknown-linux-musl
$ rustup target add aarch64-unknown-linux-musl
```

And we have to install macOS cross-compiler toolchains. `messense/homebrew-macos-cross-toolchains` can be used on both Intel chip and Apple M1 chip.

```shell
$ brew tap messense/macos-cross-toolchains
$ brew install x86_64-unknown-linux-musl
$ brew install aarch64-unknown-linux-musl
```

And we need to inform Cargo that our project uses the newly-installed linker when building for the `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` platforms.
Create a new directory called `.cargo` in your project folder and a new file called `config` inside the new folder.

```shell
$ mkdir .cargo
$ echo '[target.x86_64-unknown-linux-musl]
linker = "x86_64-unknown-linux-musl-gcc"

[target.aarch64-unknown-linux-musl] 
linker = "aarch64-unknown-linux-musl-gcc"'> .cargo/config
```

Now we can cross compile AWS Lambda Web Adapter.

```shell
$ CC=x86_64-unknown-linux-musl-gcc cargo build --release --target=x86_64-unknown-linux-musl --features vendored
$ CC=aarch64-unknown-linux-musl-gcc cargo build --release --target=aarch64-unknown-linux-musl --features vendored
```

Lambda Web Adapter binary for x86_64 will be placed at `target/x86_64-unknown-linux-musl/release/bootstrap`.
Lambda Web Adapter binary for arm64 will be placed at `target/aarch64-unknown-linux-musl/release/bootstrap`.

Finally, run the following command to package lambda adapter into two container images for x86_64 and aarch64.

```shell
$ aws ecr-public get-login-password --region us-east-1 | docker login --username AWS --password-stdin public.ecr.aws
$ docker build -f Dockerfile.mac --build-arg ARCH=x86_64 -t aws-lambda-adapter:latest-x86_64 .
$ docker build -f Dockerfile.mac --build-arg ARCH=aarch64 -t aws-lambda-adapter:latest-aarch64 .
$ docker tag aws-lambda-adapter:latest-x86_64 aws-lambda-adapter:latest
```

When these commands complete successfully, you will have the following container images.

- "aws-lambda-adapter:latest-x86_64" for x86_64.
- "aws-lambda-adapter:latest-aarch64" for arm64.
- "aws-lambda-adapter:latest" is the same as "aws-lambda-adapter:latest-x86_64".

### Testing

Please test with the following [commands](https://github.com/aws/aws-lambda-web-adapter/blob/ff2dc8bddd968e74d7dc2ec56a249c56e5a3c5a7/.github/workflows/pipeline.yaml#L46-L49) before submitting a pull request:
1. `cargo fmt -- --check`
1. `cargo clippy -- -Dwarnings`
1. `cargo nextest run`
