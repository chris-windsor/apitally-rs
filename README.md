<p align="center">
  <a href="https://apitally.io" target="_blank">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://assets.apitally.io/logos/logo-vertical-dark.png">
      <source media="(prefers-color-scheme: light)" srcset="https://assets.apitally.io/logos/logo-vertical-light.png">
      <img alt="Apitally logo" src="https://assets.apitally.io/logos/logo-vertical-light.png" width="150">
    </picture>
  </a>
</p>

<p align="center"><b>Simple, privacy-focused API monitoring & analytics</b></p>

<p align="center"><i>Apitally helps you understand how your APIs are being used and alerts you when things go wrong.<br>Just add two lines of code to your project to get started.</i></p>
<br>

![Apitally screenshots](https://assets.apitally.io/screenshots/overview.png)

---

# Apitally client library for Rust

[![Tests](https://github.com/apitally/apitally-rs/actions/workflows/tests.yaml/badge.svg?event=push)](https://github.com/apitally/apitally-rs/actions)
[![Codecov](https://codecov.io/gh/apitally/apitally-rs/graph/badge.svg?token=sV0D4JeWG6)](https://codecov.io/gh/apitally/apitally-rs)

This client library for Apitally currently supports the following Rust web
frameworks:

- [Axum](https://docs.apitally.io/frameworks/axum) (≥ 0.7.9)

Learn more about Apitally on our 🌎 [website](https://apitally.io) or check out
the 📚 [documentation](https://docs.apitally.io).

## Key features

### API analytics

Track traffic, error and performance metrics for your API, each endpoint and
individual API consumers, allowing you to make informed, data-driven engineering
and product decisions.

### Error tracking

Understand which validation rules in your endpoints cause client errors. Capture
error details and stack traces for 500 error responses, and have them linked to
Sentry issues automatically.

### Request logging

Drill down from insights to individual requests or use powerful filtering to
understand how consumers have interacted with your API. Configure exactly what
is included in the logs to meet your requirements.

### API monitoring & alerting

Get notified immediately if something isn't right using custom alerts, synthetic
uptime checks and heartbeat monitoring. Notifications can be delivered via
email, Slack or Microsoft Teams.

## Install

`cargo add apitally`

## Usage

Add Apitally to your Axum application using the `TowerLayer` middleware.

```rust
use apitally::{ApitallyClient, ApitallyLayer, RequestLoggingConfig};
use axum::{Router};

#[tokio::main]
async fn main() {
    let mut api_tally_client = ApitallyClient::new("your-client-id", "dev" /* or "prod" etc. */);
    api_tally_client.set_request_logging_config(RequestLoggingConfig::blanket_enabled());

    let router = Router::new();
    // ...routes

    router.layer(ApitallyLayer(api_tally_client));
}
```

Then add the following properties to your env:

```
APITALLY_CLIENT_ID=11111111-4444-4444-bbbb-xxxxxxxxxxxx
APITALLY_ENVIRONMENT=dev # or "prod" etc.
```

For further instructions, see our
[setup guide for Axum](https://docs.apitally.io/frameworks/axum).

## Getting help

If you need help please
[create a new discussion](https://github.com/orgs/apitally/discussions/categories/q-a)
on GitHub or
[join our Slack workspace](https://join.slack.com/t/apitally-community/shared_invite/zt-2b3xxqhdu-9RMq2HyZbR79wtzNLoGHrg).

## License

This library is licensed under the terms of the MIT license.
