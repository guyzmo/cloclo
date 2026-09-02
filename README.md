# cloclo — le proxy magnifique

A multi-profile authentication proxy and launcher for [Claude Code](https://claude.com/claude-code).

Switch between your personal and enterprise `claude.ai` accounts (or a
GitHub Copilot backend) without logging out and back in — `cloclo` sits
between Claude Code and Anthropic, injecting whichever account's
credentials are currently active.

```
Claude Code  →  cloclo proxy (127.0.0.1:9393)  →  Anthropic API / Enterprise SSO / Copilot
                     ↕ control API (/_cloclo/*)
              cloclo CLI (login/switch/status/launch)
```

## Install

```bash
cargo build --release
cp target/release/cloclo ~/bin/   # or anywhere on your PATH
```

## Quick start

```bash
cloclo init                     # create ~/.config/cloclo/config.toml (or platform equivalent)
cloclo login personal           # browser OAuth login, stores a token for the "personal" profile
cloclo login enterprise         # same, for your enterprise claude.ai account
cloclo launch --profile personal  # launch Claude Code through a per-session proxy
```

Mid-session, switch accounts without restarting Claude Code:

```bash
cloclo sw enterprise    # switches the proxy for *this* session only
cloclo m claude-opus-4-6  # override the model on the fly
```

## Profiles

Each profile in `config.toml` is one of:

- **`oauth`** — a personal `claude.ai` account. Token comes from `cloclo login <profile>`
  (which runs `claude setup-token`) and is billed against that account's subscription,
  same as using `claude` directly.
- **`enterprise_sso`** — same mechanics as `oauth`, named separately so you can tell your
  accounts apart in `cloclo profiles` / `cloclo status`.
- **`proxy`** — forwards to an arbitrary upstream (e.g. [`copilot-api`](https://github.com/ericc-ch/copilot-api))
  instead of Anthropic directly, optionally managing that upstream as a subprocess.

There is no `api_key` profile type — `cloclo` only manages OAuth-authenticated
`claude.ai` accounts. If you need a static API key, set `ANTHROPIC_API_KEY`
yourself and skip `cloclo` for that account.

## How switching scopes to a session

`cloclo launch` starts a fresh proxy on a random port for that one Claude Code
process — parallel `cloclo launch` sessions never share state. `cloclo sw` /
`cloclo m` talk to whichever proxy is reachable on `$CLOCLO_PORT` (set
automatically inside a `cloclo launch` session) or the long-running daemon
from `cloclo start` if that variable isn't set. Run them from inside the
session you want to affect.

## Desktop app (Claude.app)

Claude.app manages its own OAuth session directly — it doesn't read
`ANTHROPIC_BASE_URL`/`ANTHROPIC_API_KEY`, so it can't be proxied like Claude
Code. Instead, `cloclo desktop` gives each account its own isolated
`--user-data-dir`:

```bash
cloclo desktop launch personal     # first run: opens the app logged out — log in normally
cloclo desktop launch enterprise   # a second, independent, already-logged-in window
cloclo desktop list                # shows each profile's login status
```

Because each profile is a separate data directory, both can be logged in and
open **at the same time** as distinct windows — there's no "switching" step
to run mid-session like with `cloclo sw`.

## Commands

```
cloclo init                        Create a default config file
cloclo start [--profile P]         Start the long-running proxy daemon
cloclo stop                        Stop the daemon
cloclo login <profile> [--token T] Log in and store an OAuth token for a profile
cloclo switch <profile>            Alias: sw — switch the active profile
cloclo status                      Show the active profile, model, and stats
cloclo profiles                    Alias: ls — list configured profiles
cloclo launch [--profile P] [args] Alias: go — launch Claude Code via a per-session proxy
cloclo model [name]                Alias: m — set/clear a model override
cloclo desktop launch <profile>    Launch Claude.app against a desktop profile
cloclo desktop list                Alias: ls — list desktop profiles and login status
```

## Project structure

- `src/main.rs` — CLI entry point
- `src/cli.rs` — clap command definitions
- `src/config.rs` — TOML config loading (`ProfileConfig` tagged enum)
- `src/auth.rs` — token/key resolution with tilde expansion
- `src/login.rs` — `cloclo login`, wraps `claude setup-token`
- `src/desktop.rs` — `cloclo desktop`, launches Claude.app per isolated `--user-data-dir`
- `src/proxy/` — axum-based proxy server (forward, control API, SSE bridge)
- `src/launch.rs` — Claude Code launcher with auto-start
- `src/daemon.rs` — PID file management, daemonization
- `src/subprocess.rs` — managed subprocess lifecycle (e.g. `copilot-api`)
- `src/chanson.rs` — Claude François references

## Testing

```bash
cargo test
```

## License

WTFPL — see [LICENSE](LICENSE).
