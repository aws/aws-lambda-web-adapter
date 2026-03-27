## v1.0.0 - 2026-03-27

### Major Updates

This is the first GA release of AWS Lambda Web Adapter. It includes multi-tenancy support, Lambda Managed Instances, improved error handling, zero-copy body conversion, comprehensive documentation, and a migration path from 0.x.

### Breaking Changes

- Removed `AWS_LWA_READINESS_CHECK_MIN_UNHEALTHY_STATUS` and the `readiness_check_min_unhealthy_status` field. Use `AWS_LWA_READINESS_CHECK_HEALTHY_STATUS` / `readiness_check_healthy_status` instead.
- Non-prefixed environment variables (`HOST`, `READINESS_CHECK_PORT`, etc.) are deprecated and will be removed in 2.0. Use `AWS_LWA_` prefixed versions.

### Features

- **Multi-tenancy support**: Propagate `tenant_id` from Lambda context as `x-amz-tenant-id` header (#631)
- **Lambda Managed Instances**: Support concurrent request handling in a single execution environment (#625)
- **Error status codes**: New `AWS_LWA_ERROR_STATUS_CODES` env var to treat specific HTTP status codes as Lambda failures (#558)
- **Runtime API proxy**: New `AWS_LWA_LAMBDA_RUNTIME_API_PROXY` env var to proxy Lambda runtime API calls (#588)
- **Healthy status ranges**: New `AWS_LWA_READINESS_CHECK_HEALTHY_STATUS` supporting comma-separated codes and ranges (e.g., `200-399,404`) (#638)
- **GA readiness improvements**: Deprecation warnings for non-prefixed env vars, improved configuration validation (#667)

### Performance

- Zero-copy body conversion — avoid unnecessary allocations when forwarding request bodies (#627)
- Add `panic=abort` and `opt-level=s` to release profile for smaller binary size (#646)

### Bug Fixes

- Update aws-lc-rs to resolve 5 security advisories (#687)
- Disable connection pooling to prevent stale connections after SnapStart (#671)
- Preserve response body when `error_status_codes` triggers (#668)
- Don't append trailing `?` when query string is empty (#657)
- Fix `path_through_path` renamed to `pass_through_path` (#619)
- Override user-set `x-amzn-{lambda,request}-context` headers to prevent spoofing (#286)
- Remove `transfer-encoding` header from responses for SAM local compatibility (#442)
- Warn when configured `authorization_source` header is not found in request (#479)

### Documentation

- Comprehensive rustdoc documentation for all public types and methods
- Added migration guide for 0.x to 1.0 upgrade
- Added SECURITY.md
- Added user guide with GitHub Pages deployment (#639)
- Simplify README and link to user guide (#644)

### CI/CD

- Split pipeline into separate PR, Merge, and Release workflows (#642)
- Add conventional commits changelog generation and commit linting (#641)
- Add workflow to verify examples without deployment (#643)
- Daily security audit via `rustsec/audit-check` (#391)
- Benchmark tracking with PR comments

### Dependencies

- Upgrade lambda runtime from 1.1.0-rc1 to 1.1.1 (#661)
- Bump rustls-webpki from 0.103.9 to 0.103.10 (#679)

### Examples

- Add FastMCP examples (#645)
- Add Datadog examples for streaming mode (#608)
- Upgrade deprecated Lambda runtimes in examples (#649)

---

## v1.0.0-rc1 - 2026-02-16

### Features

- **Multi-tenancy support**: Propagate `tenant_id` from Lambda context as `x-amz-tenant-id` header (#631)
- **Lambda Managed Instances**: Support concurrent request handling in a single execution environment (#625)
- Improve error handling, comprehensive documentation, and API improvements (#628)

### Performance

- Zero-copy body conversion and performance benchmarks (#627)

### Bug Fixes

- Rename `path_through_path` to `pass_through_path` (#619)
- Use native SAM CLI installer for all pipeline jobs (#632)

### Examples

- Add Datadog examples (#602, #608)

### Maintenance

- Update dependencies to latest versions (#624)
- Update arm64 supported regions (#621)
- Update `actions/checkout` GitHub Actions (#594)

---

## v0.9.1 - 2025-04-23

### Features

- Add `AWS_LWA_LAMBDA_RUNTIME_API_PROXY` to overwrite Lambda Runtime API endpoint (#588)

### Examples

- Bun server with GraphQL examples (#584)

---

## v0.9.0 - 2025-01-10

### Features

- Force Lambda failure on configurable HTTP error status codes (#558)
- Streaming API backend-only example (#543)

### Examples

- SvelteKit SSR zip example (#561)
- Remix examples (#524)
- FastHTML examples (#496)

### Improvements

- Simplify debug logging for app readiness checks (#520)
- Read AWS region from env var (#518)

---

## v0.8.4 - 2024-07-27

### Features

- Add `authorization_source` feature to copy custom header to `Authorization` (#478)
- Warn when configured authorization source header not found (#479)

---

## v0.8.3 - 2024-04-21

### Bug Fixes

- Remove `transfer-encoding` header from responses (#442)

### Examples

- Javalin 6 example using Arm64 image (#425)
- NestJS example (#394)
- FastAPI Background Tasks example (#408)
- FastAPI response streaming with Claude3 refactor (#416)

---

## v0.8.2 - 2024-04-10

### Improvements

- Add daily security audit (#391)
- Upgrade lambda-http crate to 0.9.3 (#390)

### Examples

- Go HTTP example (#383)
- Sinatra example (#365)

---

## v0.8.1 - 2024-01-24

### Bug Fixes

- Minor fixes and dependency updates

---

## v0.8.0 - 2024-01-22

### Features

- Pass-through events support for non-HTTP triggers (#342)
- Adopt `cargo-lambda` for builds (#337)

### Examples

- SQS + Express.js example (#347, #348)

---

## v0.7.1 - 2023-08-18
# Major Updates

This minior release add Lambda Context support, expend to new regions, additional examples and bug fixes. 


## What's Changed
* Update examples and doc with v0.7.0 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/215
* Handle readiness check by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/217
* Fix FastAPI description in the sam template. by @joe-king-sh in https://github.com/awslabs/aws-lambda-web-adapter/pull/219
* nextjs example for lambda streaming response by @xjiaqing in https://github.com/awslabs/aws-lambda-web-adapter/pull/218
* Update Nextjs Response Streaming Example by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/220
* fix nextjs-response-streaming example title typo by @xjiaqing in https://github.com/awslabs/aws-lambda-web-adapter/pull/221
* Bump yaml from 2.2.1 to 2.2.2 in /examples/nextjs-response-streaming by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/222
* Upgrade Flask to 2.3.2 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/225
* Update nextjs response streaming example readme by @xjiaqing in https://github.com/awslabs/aws-lambda-web-adapter/pull/227
* Laravel link by @elonniu in https://github.com/awslabs/aws-lambda-web-adapter/pull/229
* Nextjs streaming response by @xjiaqing in https://github.com/awslabs/aws-lambda-web-adapter/pull/230
* Upgrade FastAPI in examples by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/235
* delete public in nextjs response streaming example by @xjiaqing in https://github.com/awslabs/aws-lambda-web-adapter/pull/238
* SpringBoot Response Streaming using function url by @soyer-dev in https://github.com/awslabs/aws-lambda-web-adapter/pull/237
* Bump github.com/gin-gonic/gin from 1.9.0 to 1.9.1 in /examples/gin/app by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/240
* Bump github.com/gin-gonic/gin from 1.9.0 to 1.9.1 in /examples/gin-zip/app by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/241
* Fix: nextjs-response-streaming build docker image fail by @xjiaqing in https://github.com/awslabs/aws-lambda-web-adapter/pull/243
* Add AWS_LWA_PORT as a fallback for readiness_check_port. by @sprvgt in https://github.com/awslabs/aws-lambda-web-adapter/pull/246
* Pass Lambda Context to web app in a new header by @mbfreder in https://github.com/awslabs/aws-lambda-web-adapter/pull/248
* Add option to override unhealthy status so 4xx can be unhealthy by @jeffmercerVG in https://github.com/awslabs/aws-lambda-web-adapter/pull/252
* Bump semver from 6.3.0 to 6.3.1 in /examples/nextjs-response-streaming by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/256
* Bump semver from 6.3.0 to 6.3.1 in /examples/nextjs-zip/app by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/255
* Bump semver from 6.3.0 to 6.3.1 in /examples/nextjs/app by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/254
* Update github actions to publish lambda web adapter in all new regions by @mbfreder in https://github.com/awslabs/aws-lambda-web-adapter/pull/264
* fix: set arm64_supported to false on eu-central-2 by @mbfreder in https://github.com/awslabs/aws-lambda-web-adapter/pull/269
* Update github actions to deploy LWA layer in china regions by @mbfreder in https://github.com/awslabs/aws-lambda-web-adapter/pull/266
* fix: update pipeline file to run package-china-gamma before load-china-gamma-matrix2 by @mbfreder in https://github.com/awslabs/aws-lambda-web-adapter/pull/270
* Updated Readme with new layer ARNs for China regions by @mbfreder in https://github.com/awslabs/aws-lambda-web-adapter/pull/271
* fix: deploy to china gamma accounts when PR is merged to main branch by @mbfreder in https://github.com/awslabs/aws-lambda-web-adapter/pull/272

## New Contributors
* @joe-king-sh made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/219
* @xjiaqing made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/218
* @soyer-dev made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/237
* @sprvgt made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/246
* @mbfreder made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/248
* @jeffmercerVG made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/252

**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.7.0...v0.7.1

## v0.7.0 - 2023-04-15
## Major updates

This release adds support for Lambda Response Streaming, name spaces environment variables, tighten readiness check for HTTP. 

## What's Changed
* Update examples to v0.6.4 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/198
* Improved PHP example by @elonniu in https://github.com/awslabs/aws-lambda-web-adapter/pull/195
* Supported PHP ZIP Example by @elonniu in https://github.com/awslabs/aws-lambda-web-adapter/pull/165
* Added bootstrap script by @elonniu in https://github.com/awslabs/aws-lambda-web-adapter/pull/200
* Added bootstrap script by @elonniu in https://github.com/awslabs/aws-lambda-web-adapter/pull/201
* Add streaming response support by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/204
* Use Ubuntu 20.04 for builds by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/205
* Tighten HTTP readiness check by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/206
* Name spaced all environment variables by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/207
* Bump h2 from 0.3.15 to 0.3.17 in /examples/rust-actix-web-zip/rust_app by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/208
* Bump h2 from 0.3.16 to 0.3.17 in /examples/rust-axum-https-zip/rust_app by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/209
* Update pipeline to remove foresight integration by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/212
* Add FastAPI response streaming example by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/211
* Remove remaining Foresight integration steps by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/213
* Update project README by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/210
* Bump version to v0.7.0 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/214


**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.6.4...v0.7.0

## v0.6.4 - 2023-03-15
## Main Updates

This is a minor bug fix release. 

- **[Bug Fix]** Fix 'ca certs not found' issue when TLS is not enabled
- **[Example]** New Nginx Zip example
- **[Example]** New PHP Zip example


## What's Changed
* Update examples to v0.6.3 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/191
* Improved Nginx Example by @elonniu in https://github.com/awslabs/aws-lambda-web-adapter/pull/192
* Added Nginx ZIP Example by @elonniu in https://github.com/awslabs/aws-lambda-web-adapter/pull/171
* Improved PHP example by @elonniu in https://github.com/awslabs/aws-lambda-web-adapter/pull/164
* Separate HTTPS and HTTP adapters by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/196
* Prepare to release v0.6.4 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/197


**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.6.3...v0.6.4

## v0.6.3 - 2023-03-10
## Main Updates

- **[Feature]**: support web applications over TLS/HTTPS 
- **[Bug Fix]**: handle URL encoding properly with the URL crate
- **[Example]**: add FastAPI HTTPS example
- **[Example]**: add Axum HTTPS example
- **[Example]**: add Actix Web example
- **[Example]**: update Next.js example to enable cache
- **[Doc]**: ports should be avoided
- **[Doc]**: local debugging with sam local
- **[Chore]**: pipeline update


## What's Changed
* Upgrade to Nextjs 13 and enable cache by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/163
* Update README and examples for v0.6.2 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/172
* Use url crate to handle app url encoding by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/176
* Upgrade gin to v1.9.0 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/180
* Update Axum example to use new SAM CLI rust build by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/181
* Add Actix Web example by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/182
* Add HTTPS support by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/183
* Add rust axum https example by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/185
* Update Axum https example by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/186
* Document ports should be avoided by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/187
* Document Lambda Function URL support by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/188
* Document local debugging with aws sam local by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/189
* Release v0.6.3 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/190


**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.6.2...v0.6.3

## v0.6.2 - 2023-02-17
Main updates: 

1. Optional gzip compression of responses
2. Add e2e tests to the pipeline
3. Integrate Foresight in the pipeline
4. Add Deno Oak in Zip example


## What's Changed
* Update README and examples to v0.6.1 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/123
* Add e2e tests by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/126
* Update pipeline to fix matrix variables by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/127
* Update pipeline to test the latest layers and images by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/134
* Add Foresight workflow-kit action to collect metrics and traces by @serkan-ozal in https://github.com/awslabs/aws-lambda-web-adapter/pull/137
* Add Deno Oak in Zip example by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/139
* Use nextest as test runner to get junit test reports by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/142
* Bump bumpalo from 3.10.0 to 3.12.0 by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/140
* Add Foresight Test Kit Action by @rwxdash in https://github.com/awslabs/aws-lambda-web-adapter/pull/143
* Return app response directly to lambda-http runtime by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/146
* Refactor Tower.Service call method by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/147
* Bump tokio from 1.20.3 to 1.20.4 in /examples/rust-axum-zip by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/149
* Bump tokio from 1.24.1 to 1.24.2 by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/150
* Upgrade Flask by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/158
* upgrade fastapi by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/159
* Optional gzip compression of responses by @huntharo in https://github.com/awslabs/aws-lambda-web-adapter/pull/157
* release v0.6.2 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/161

## New Contributors
* @serkan-ozal made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/137
* @rwxdash made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/143
* @huntharo made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/157

**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.6.1...v0.6.2


## v0.6.1 - 2023-01-13
## What's Changed
* Added integration tests for http adapter by @ferdingler in https://github.com/awslabs/aws-lambda-web-adapter/pull/113
* Removed redundant closing tag & Format code by PSR by @elonniu in https://github.com/awslabs/aws-lambda-web-adapter/pull/115
* Replace reqwest with hyper client by @calavera in https://github.com/awslabs/aws-lambda-web-adapter/pull/114
* Bump json5 from 1.0.1 to 1.0.2 in /examples/nextjs/app by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/119
* Bump json5 from 1.0.1 to 1.0.2 in /examples/nextjs-zip/app by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/118
* Bump tokio from 1.20.1 to 1.20.3 in /examples/rust-axum-zip by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/117
* rewrite extension client and upgrade tokio by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/120
* Release v0.6.1 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/122

## New Contributors
* @ferdingler made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/113
* @elonniu made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/115

**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.6.0...v0.6.1

## v0.6.0 - 2022-12-18
## What's Changed
* relax readiness check for HTTP by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/106
* forward RequestContext in a http header by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/108
* Bump qs and express in /examples/expressjs/app/src by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/110
* update README and examples for v0.6.0 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/111
* bump release version to 0.6.0 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/112


**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.5.1...v0.6.0

## v0.5.1 - 2022-10-30
## What's Changed
* upgrade to lambda_http v0.7.1 to pass correct x-ray trace id header by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/103
* release v0.5.1 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/105


**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.5.0...v0.5.1

## v0.5.0 - 2022-10-13
## What's Changed
* update README for v0.4.1 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/89
* add an example for flask in zip by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/91
* add golang gin examples by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/93
* add fastapi examples by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/98
* Upgrade to lambda_http 0.7 by @calavera in https://github.com/awslabs/aws-lambda-web-adapter/pull/100
* Release 0.5.0 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/101


**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.4.1...v0.5.0

## v0.4.1 - 2022-09-21
This release contains two notable changes: 

- add TCP readiness check option
- minor change to the library public interface

## What's Changed
* update README and examples to v0.4.0 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/77
* Hide run instruction by @calavera in https://github.com/awslabs/aws-lambda-web-adapter/pull/78
* change register_default_extension() as a method on Adapter by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/79
* update axum version to 0.5.16 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/81
* Add more debug information for req/res transformations by @calavera in https://github.com/awslabs/aws-lambda-web-adapter/pull/83
* update demo for sam local debug by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/84
* Add tcp  readiness check by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/86
* Release v0.4.1 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/88


**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.4.0...v0.4.1

## v0.4.0 - 2022-09-12
## Major updates
- Support async init for long initialization lambda functions
- Add more examples: Rust Axum (Zip) and Next.js (both Zip and Docker)
- Refactor main logic into a library
- Publish the library to crates.io


## What's Changed
* Update examples to version 0.3.3 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/52
* support async init for long initialization lambda functions by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/53
* Add Rust Axum Example by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/56
* add next.js example by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/59
* fix github build issue by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/60
* Fix build command by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/61
* fix Adapter Layer Version Permission by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/62
* remove python3.6 from compatible runtimes for x86 layer by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/63
* Bump next from 12.2.3 to 12.2.4 in /examples/nextjs/app by @dependabot in https://github.com/awslabs/aws-lambda-web-adapter/pull/65
* Add Next.js Zip example by @julianbonilla in https://github.com/awslabs/aws-lambda-web-adapter/pull/66
* Extract logic into a library. by @calavera in https://github.com/awslabs/aws-lambda-web-adapter/pull/68
* Update lambda_http by @calavera in https://github.com/awslabs/aws-lambda-web-adapter/pull/69
* add metadata for crates.io by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/70
* Add Related projects in README.md by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/71
* fix readiness check function by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/72
* Fix casing in README by @mnapoli in https://github.com/awslabs/aws-lambda-web-adapter/pull/73
* Remove blocking calls by @calavera in https://github.com/awslabs/aws-lambda-web-adapter/pull/74
* configure log level with RUST_LOG environment variable by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/75
* release v0.4.0 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/76

## New Contributors
* @dependabot made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/65
* @julianbonilla made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/66
* @mnapoli made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/73

**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.3.3...v0.4.0

## v0.3.3 - 2022-07-19
## What's Changed
* Preserve aws-lambda-rust in the user-agent by @calavera in https://github.com/awslabs/aws-lambda-web-adapter/pull/33
* Added Spring Boot Zip example by @maschnetwork in https://github.com/awslabs/aws-lambda-web-adapter/pull/34
* Update project name in README file by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/37
* readness_check_port defaults to port by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/45
* readiness check verify the http status code is successful (2xx) by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/46
* reduce idle connection pool time to 4 seconds by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/47
* treat response body as TEXT when both CONTENT_ENCODING and CONTENT_TY… by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/48
* upgrade to Rust 2021 edition by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/49
* upgrade to the latest lambda-http crate by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/50
* Bump crate version to 0.3.3 by @bnusunny in https://github.com/awslabs/aws-lambda-web-adapter/pull/51

## New Contributors
* @calavera made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/33
* @maschnetwork made their first contribution in https://github.com/awslabs/aws-lambda-web-adapter/pull/34

**Full Changelog**: https://github.com/awslabs/aws-lambda-web-adapter/compare/v0.3.2...v0.3.3

## v0.3.2 - 2022-03-29
## What's Changed
* support to remove base path from http request path  by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/23
* update pipeline to deploy adapter layer in beta and gamma accounts by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/24
* update README file by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/25
* update expressjs-zip example using the new adapter layer by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/26
* make layers public by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/27
* update pipeline to publish OCI images to ECR public repo by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/28
* prepare to release v0.3.2 by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/29
* update Makefile by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/30
* update pipeline by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/31
* update pipeline by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/32


**Full Changelog**: https://github.com/aws-samples/aws-lambda-adapter/compare/v0.3.1...v0.3.2

## v0.3.1 - 2022-03-22
This is a release for bug fix and minor improvement. 

- [bugfix] remove stage from URL send to app server 
- [improvement] Custom User-Agent as aws-lambda-adapter/CARGO-PACKAGE-VERSION
- [improvement] Strip the binary to reduce size
- Update examples

## What's Changed
* remove stage from URL send to app server + custom User-Agent by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/22


**Full Changelog**: https://github.com/aws-samples/aws-lambda-adapter/compare/v0.3.0...v0.3.1

## v0.3.0 - 2022-03-20
Upgrade Rust Runtime lambda_http crate to v0.5.1 

## What's Changed
* upgrade to lambda_http v0.5.1 by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/21
* [SpringBoot Example] use customized metrics to scale Provisioned Concurrency by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/17

**Full Changelog**: https://github.com/aws-samples/aws-lambda-adapter/compare/v0.2.0...v0.3.0

## v0.2.0 - 2022-02-07
Lambda Adapter as an Extension. Run web app containers on Lambda without changing ENTRYPOINT. 

This is a breaking change. Please refer to [README](https://github.com/aws-samples/aws-lambda-adapter/blob/main/README.md) for updated usage. 


## What's Changed
* run Lambda Adapter as an extension by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/16


**Full Changelog**: https://github.com/aws-samples/aws-lambda-adapter/compare/v0.1.2...v0.2.0

## v0.1.2 - 2022-01-31
support HTTP compression

## What's Changed
* Add an example to show how to use Lambda Adapter with managed runtime  by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/13
* add support for HTTP compression by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/14


**Full Changelog**: https://github.com/aws-samples/aws-lambda-adapter/compare/v0.1.1...v0.1.2

## v0.1.1 - 2021-10-24
New: instruction for compiling the adapter for Gravition2. 
Bug fix: forward query paramters to application process.

## What's Changed
* update README by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/6
* add instructions for ARM support by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/8
* update README for ARM support by @bnusunny in https://github.com/aws-samples/aws-lambda-adapter/pull/9


**Full Changelog**: https://github.com/aws-samples/aws-lambda-adapter/compare/v0.1.0...v0.1.1

## v0.1.0 - 2021-09-15
Initial release

