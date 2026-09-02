# Cloclo — Le Proxy Magnifique

A multi-profile authentication proxy and launcher for Claude Code.

## Architecture

```
Claude Code  →  cloclo proxy (127.0.0.1:9393)  →  Anthropic API / Enterprise SSO / Copilot
                     ↕ control API (/_cloclo/*)
              cloclo CLI (switch/status/launch)
```

## Building

```bash
cargo build --release
```

## Quick Start

```bash
cloclo init              # Create config at ~/.config/cloclo/config.toml
cloclo start             # Start the proxy daemon
cloclo launch --profile work  # Launch Claude Code via the proxy
cloclo sw copilot        # Switch to copilot mid-session
cloclo stop              # Graceful shutdown
```

## Project Structure

- `src/main.rs` — CLI entry point
- `src/cli.rs` — clap command definitions
- `src/config.rs` — TOML config loading (ProfileConfig tagged enum)
- `src/auth.rs` — Token/key resolution with tilde expansion
- `src/proxy/` — axum-based proxy server (forward, control, SSE bridge)
- `src/launch.rs` — Claude Code launcher with auto-start
- `src/daemon.rs` — PID file management, daemonization
- `src/subprocess.rs` — copilot-api subprocess lifecycle
- `src/chanson.rs` — Claude François references

## Key Types

- `Alexandrie` (`Arc<RwLock<ProxyState>>`) — shared proxy state
- `ProfileConfig` — tagged enum: OAuth (personal), EnterpriseSso (enterprise), Proxy
- `ResolvedAuth` — BearerToken, Passthrough
- `SecretSource` — Literal, File, Env

## Testing

```bash
cargo test
```

## Dependencies

axum 0.8, tokio, reqwest (async with streaming), clap 4, serde, toml, tracing
