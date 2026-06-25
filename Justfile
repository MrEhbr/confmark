APP := `basename $(pwd)`
profile := env_var_or_default('PROFILE', 'debug')

# Show available targets
help:
    @just --list

# Install development tools (cargo plugins)
setup:
    @echo "Installing development tools..."
    @cargo install --locked prek
    @prek install
    @cargo install cargo-llvm-cov --locked
    @echo "✓ Development tools installed"

# Rust
# Build application binary
build *opts="":
    @echo "Building {{APP}} ({{profile}} profile)"
    @cargo build {{ if profile == "release" { "--release" } else { "" } }} {{opts}}

# Install application into ~/.cargo/bin
install *opts="":
    @echo "Installing {{APP}}"
    @cargo install --path . {{opts}}

# Run tests
test *opts="--workspace":
    @cargo nextest run {{opts}}

# Run integration tests
test-integration filter="":
    @cargo nextest run --workspace -E 'binary(/integration_/){{ if filter != "" { " & test(/" + filter + "/)" } else { "" } }}' --run-ignored all

# Generate code coverage report (requires: cargo install cargo-llvm-cov)
test-coverage *opts="--workspace":
    @cargo llvm-cov nextest {{opts}}

# Run documentation tests
test-doc *opts="--workspace":
    @cargo test --doc {{opts}}

# Lint code
lint *opts="":
    cargo clippy --workspace --fix --allow-dirty --allow-staged --no-deps --all-targets --all-features {{opts}} -- -D warnings
    @cargo fmt --all -- --check

# Format code
fmt:
    @cargo fmt --all

# Check code for typos
typos:
    @typos --write-changes

# Tidy dependencies
tidy:
    @cargo update

# Download dependencies
deps:
    @cargo fetch

# Run application
run +args="--help":
    @cargo run {{ if profile == "release" { "--release" } else { "" } }} -- {{args}}
