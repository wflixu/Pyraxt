# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Axon is a high-performance async Python web framework with a Rust runtime. It uses PyO3 for Python-Rust bindings and Axum under the hood.

## Development Setup

### Prerequisites
- Python >= 3.9, <= 3.12
- Rust (latest stable)
- C compiler (gcc/clang)

### Setup Commands
```bash
# Create virtual environment
python3 -m venv .venv
source .venv/bin/activate

# Install dependencies
pip install pre-commit poetry maturin
poetry install --with dev --with test

# Install pre-commit hooks
pre-commit install

# Build and install the Rust package
maturin develop

# Run the test server
poetry run test_server

# Run all tests
pytest

# Run integration tests only
pytest integration_tests

# Run unit tests only
pytest unit_tests
```

### Quick Iteration
```bash
# Rebuild and restart server
maturin develop && poetry run test_server

# Rebuild and run tests
maturin develop && pytest
```

## Architecture

### Core Structure
- **`axon/`** - Python source code (note: directory is named `axon` but package is `axon`)
- **`src/`** - Rust source code (PyO3 bindings)
- **`integration_tests/`** - Integration tests
- **`unit_tests/`** - Unit tests

### Python Layer (`axon/`)
- **`__init__.py`** - Main `Axon` and `BaseAxon` classes, decorators for HTTP methods
- **`router.py`** - Route handling, middleware routing, websocket routing
- **`processpool.py`** - Multi-process server execution
- **`openapi.py`** - Automatic OpenAPI documentation generation
- **`ws.py`** - WebSocket support
- **`authentication.py`** - Authentication handler
- **`dependency_injection.py`** - DI container

### Rust Layer (`src/`)
- **`lib.rs`** - PyO3 module entry point
- **`server.rs`** - Core server implementation
- **`routers/`** - HTTP router, middleware router, websocket router
- **`types/`** - Request, Response, Headers, QueryParams, HttpMethod
- **`executors/`** - Function execution handlers
- **`websockets/`** - WebSocket registry and connector

### Key Patterns
1. **Decorator-based routing**: `@app.get("/")`, `@app.post("/user")`
2. **Middleware**: `@app.before_request()` and `@app.after_request()`
3. **Async-first**: Handlers can be async or sync
4. **Multi-process**: Uses `multiprocess` library for parallel workers

## Testing

- Tests use `pytest` with `requests` and `websocket-client` for integration testing
- Integration tests run against a live server (`test_server`)
- Test helpers in `integration_tests/helpers/` provide HTTP method utilities

## Linting

```bash
# Using pre-commit (installed hooks)
pre-commit run

# Manual linting
black axon/ integration_tests/
ruff check axon/ integration_tests/
```

## Notes

- The package name in `pyproject.toml` is `axon`
- The Python module directory is `axon/`
- The Rust crate is named `axon` in `Cargo.toml`
- Python 3.13 support is in development
- IO-uring support available via `maturin develop --cargo-extra-args="--features=io-uring"`
