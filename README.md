# ddns-a

[![Crates.io](https://img.shields.io/crates/v/ddns-a.svg)](https://crates.io/crates/ddns-a)
[![Downloads](https://img.shields.io/crates/d/ddns-a.svg)](https://crates.io/crates/ddns-a)
[![License](https://img.shields.io/crates/l/ddns-a.svg)](LICENSE)
<!-- [![CI](https://github.com/doraemonkeys/ddns-a/actions/workflows/ci.yml/badge.svg)](https://github.com/doraemonkeys/ddns-a/actions/workflows/ci.yml) -->
[![Test Coverage](https://img.shields.io/badge/coverage-90%25%2B-brightgreen.svg)](.github/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/doraemonkeys/ddns-a/pulls)

A lightweight Dynamic DNS client for Windows that monitors IP address changes and notifies external services via webhooks.

## Features

- **Real-time monitoring** – Uses Windows API events with polling fallback
- **State persistence** – Detects IP changes that occurred during program downtime
- **Flexible filtering** – Include/exclude adapters by regex, skip virtual adapters
- **Customizable webhooks** – Any HTTP method, headers, bearer auth, Handlebars templates
- **Robust retry** – Exponential backoff with configurable limits
- **Graceful shutdown** – Handles Ctrl+C cleanly

## Installation

```bash
cargo install --git https://github.com/doraemonkeys/ddns-a.git
```

```bash
cargo install ddns-a
```

## Quick Start

```bash
# Monitor IPv6 changes and send to webhook
ddns-a --url https://example.com/webhook --ip-version ipv6 --exclude-virtual

# Monitor both IPv4 and IPv6
ddns-a --url https://api.example.com/ddns --ip-version both --exclude-virtual

# With bearer token and custom template
ddns-a --url https://api.example.com/ddns \
       --ip-version ipv4 \
       --bearer YOUR_TOKEN \
       --body-template '{"ip": "{{address}}", "adapter": "{{adapter}}"}'

# Test mode - log changes without sending webhooks
ddns-a --url https://example.com/webhook --ip-version ipv6 --dry-run --verbose

# With state persistence (detect changes across restarts)
ddns-a --url https://example.com/webhook --ip-version ipv6 --state-file ~/.ddns-a/state.json

# Generate config file
ddns-a init
```

## CLI Options

```
ddns-a [OPTIONS] --url <URL> --ip-version <VERSION>
ddns-a init [--output <FILE>]

Required:
    --url <URL>                  Webhook URL
    --ip-version <VERSION>       ipv4 | ipv6 | both

Request:
    --method <METHOD>            HTTP method (default: POST)
    --header <K=V>               HTTP header (repeatable)
    --bearer <TOKEN>             Bearer token
    --body-template <TEMPLATE>   Handlebars template

Filter:
    --include-adapter <PATTERN>  Include adapters matching regex
    --exclude-adapter <PATTERN>  Exclude adapters matching regex
    --exclude-virtual            Exclude virtual adapters

Monitor:
    --poll-interval <SEC>        Polling interval (default: 60)
    --poll-only                  Disable API events, polling only
    --state-file <PATH>          State file for detecting changes across restarts

Retry:
    --retry-max <N>              Max attempts (default: 3)
    --retry-delay <SEC>          Initial delay (default: 5)

Other:
    --config <FILE>              Config file path
    --dry-run                    Log changes without sending webhooks
    --verbose                    Enable debug logging
```

## Default Filtering Behavior

By default (without any filter options):

| Adapter Type | Monitored |
|--------------|-----------|
| Ethernet | ✅ Yes |
| Wi-Fi | ✅ Yes |
| VMware / VirtualBox / Hyper-V | ✅ Yes |
| Loopback (127.0.0.1 / ::1) | ❌ No (always excluded) |

**Recommendation**: Use `--exclude-virtual` to skip virtual adapters in most cases.

### Filter Examples

```bash
# Exclude virtual adapters (recommended)
ddns-a --url ... --ip-version ipv6 --exclude-virtual

# Monitor only Ethernet adapter
ddns-a --url ... --ip-version ipv6 --include-adapter "^Ethernet$"

# Monitor Ethernet and Wi-Fi, exclude Docker
ddns-a --url ... --ip-version both \
       --include-adapter "^Ethernet" \
       --include-adapter "^Wi-Fi" \
       --exclude-adapter "^Docker"
```

## Configuration File

Generate a template:

```bash
ddns-a init --output ddns-a.toml
```

Example `ddns-a.toml`:

```toml
[webhook]
url = "https://api.example.com/ddns"
ip_version = "ipv6"
method = "POST"
body_template = '{"ip": "{{address}}", "adapter": "{{adapter}}", "event": "{{kind}}"}'

# Optional: bearer token or custom headers
# bearer = "your-token"
# [webhook.headers]
# X-Custom-Header = "value"

[filter]
# include = ["^Ethernet", "^Wi-Fi"]
# exclude = ["^Docker"]
exclude_virtual = true

[monitor]
poll_interval = 60
poll_only = false
# state_file = "ddns-a-state.json"

[retry]
max_attempts = 3
initial_delay = 5
max_delay = 60
multiplier = 2.0
```

**Priority**: CLI arguments > Config file > Built-in defaults

## Body Template Variables

Use [Handlebars](https://handlebarsjs.com/) syntax:

| Variable | Description |
|----------|-------------|
| `{{adapter}}` | Adapter name |
| `{{address}}` | IP address |
| `{{kind}}` | `added` or `removed` |
| `{{timestamp}}` | Unix timestamp |

Example:

```json
{"ip": "{{address}}", "adapter": "{{adapter}}", "event": "{{kind}}"}
```

## How It Works

1. On startup, fetches current IP addresses from all (filtered) adapters
2. If `--state-file` is set, compares with saved state and triggers webhooks for changes during downtime
3. Listens for Windows network change events via `NotifyIpInterfaceChange` API
4. Falls back to pure polling if API events fail
5. On IP change, sends webhook with retry on failure
6. Uses debouncing to merge rapid changes (2s window)

## Platform Support

Currently Windows-only. The architecture supports adding Linux/macOS via platform-specific `AddressFetcher` and `ApiListener` implementations.

## License

Apache License 2.0
