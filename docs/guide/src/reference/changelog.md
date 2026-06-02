# Changelog

For the full changelog with all releases and contributors, see the [GitHub Releases](https://github.com/aws/aws-lambda-web-adapter/releases) page.

## Highlights

### v0.7.1 (2023-08-18)
- Lambda Context support via `x-amzn-lambda-context` header
- Expanded to new AWS regions including China regions
- Customizable healthy status codes for readiness check

### v0.7.0 (2023-04-15)
- Lambda Response Streaming support
- Namespaced environment variables (`AWS_LWA_` prefix)
- Tightened HTTP readiness check

### v0.6.3 (2023-03-10)
- TLS/HTTPS support for web applications
- Proper URL encoding handling

### v0.6.2 (2023-02-17)
- Optional gzip compression of responses

### v0.6.0 (2022-12-18)
- Request Context forwarding via `x-amzn-request-context` header

### v0.5.0 (2022-10-13)
- Upgraded to `lambda_http` 0.7

### v0.4.0 (2022-09-12)
- Async initialization support
- Refactored into a library, published to crates.io

### v0.3.2 (2022-03-29)
- Base path removal support
- Published OCI images to ECR public repo

### v0.2.0 (2022-02-07)
- Rewritten as a Lambda Extension (breaking change)
- No longer requires changing ENTRYPOINT

### v0.1.0 (2021-09-15)
- Initial release
