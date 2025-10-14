CARGO ?= cargo
NIGHTLY ?= +nightly
LOCAL_BIN ?= mcp_google_calendar
SHUTTLE_BIN ?= shuttle
SHUTTLE ?= cargo shuttle

.PHONY: help run run-shuttle build build-release test fmt clippy clean shuttle-deploy shuttle-deploy-secrets shuttle-logs shuttle-status

help:
	@echo "Available targets:"
	@echo "  make run                  # Run the local MCP server binary ($(LOCAL_BIN))"
	@echo "  make run-shuttle          # Run the Shuttle binary ($(SHUTTLE_BIN)) locally"
	@echo "  make build                # Debug build with nightly toolchain"
	@echo "  make build-release        # Release build with nightly toolchain"
	@echo "  make test                 # Run tests with nightly toolchain"
	@echo "  make fmt                  # Format code with rustfmt"
	@echo "  make clippy               # Run clippy with warnings as errors"
	@echo "  make clean                # Clean cargo artifacts"
	@echo "  make shuttle-deploy       # Deploy to Shuttle using cargo shuttle deploy"
	@echo "  make shuttle-deploy-secrets # Deploy to Shuttle and sync Secrets.toml"
	@echo "  make shuttle-logs         # Tail latest Shuttle logs"
	@echo "  make shuttle-status       # Show Shuttle deployment status"

run:
	$(CARGO) $(NIGHTLY) run --bin $(LOCAL_BIN)

run-shuttle:
	$(SHUTTLE) run --secrets Secrets.dev.toml

build:
	$(CARGO) $(NIGHTLY) build

build-release:
	$(CARGO) $(NIGHTLY) build --release

test:
	$(CARGO) $(NIGHTLY) test

fmt:
	$(CARGO) fmt

clippy:
	$(CARGO) $(NIGHTLY) clippy --all-targets --all-features -D warnings

clean:
	$(CARGO) clean

shuttle-deploy:
	$(SHUTTLE) deploy

shuttle-deploy-secrets:
	$(SHUTTLE) deploy --secrets Secrets.toml

shuttle-logs:
	$(SHUTTLE) logs --latest

shuttle-status:
	$(SHUTTLE) status
