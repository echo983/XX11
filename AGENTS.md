# Repository Guidelines

## Project Overview
This repository is currently a requirements/specification drop for the "X11 GUI Bridge for LLM" concept. The only tracked artifact is `????.txt`, which captures goals, DSL shapes, and a proposed Rust module layout. Treat it as the source of truth until code lands.

## Project Structure & Module Organization
- `????.txt`: product and architecture specification, including the tentative Rust crate layout under `src/`.
- Source, tests, and assets directories are not present yet; create them once implementation starts (use the layout proposed in `????.txt`).

## Build, Test, and Development Commands
- No build scripts or package manifests are checked in yet.
- When scaffolding is added, document canonical commands here (e.g., `cargo build`, `cargo test`, `cargo run`).

## Coding Style & Naming Conventions
- Until a formatter/linter is configured, follow standard Rust conventions.
- Use snake_case for module and function names, and SCREAMING_SNAKE_CASE for constants.
- Keep module boundaries aligned with the planned folders (`orchestrator`, `dsl`, `x11`, `state`, `llm`).

## Testing Guidelines
- Tests are not defined yet. When added, prefer `tests/` for integration tests and `mod tests` for unit tests.
- Name tests after behavior (e.g., `validates_event_envelope`).

## Commit & Pull Request Guidelines
- No git history is available in this directory, so there is no established commit style.
- Use short, imperative commit summaries (e.g., "Add DSL validator") and include a brief rationale in the body when behavior changes.
- PRs should link to the relevant requirement section in `????.txt` and describe test coverage.

## Security & Configuration Tips
- The spec assumes Windows + VcXsrv and a local `DISPLAY` such as `127.0.0.1:0.0`.
- Do not hardcode API keys or secrets in source; document required env vars once the client is implemented.