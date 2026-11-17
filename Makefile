PASSPHRASE ?= demo-passphrase
VAULT      ?= default
PORT       ?= 8787
TEST_PORT  ?= 18787

SERVER_BIN := ./target/debug/server
CLIENT_BIN := ./target/debug/gpui-todo
CLIENT_ENV  = TODO_PASSPHRASE='$(PASSPHRASE)' TODO_VAULT='$(VAULT)' TODO_SYNC_URL=http://127.0.0.1:$(PORT)

.PHONY: help build dev server client local test test-sync fmt lint

help: ## Show available targets
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-10s %s\n", $$1, $$2}'

build: ## Build the app and the sync server
	cargo build --bins

dev: build ## Start the sync server and two synced clients; Ctrl-C or closing both windows stops everything
	@TODO_SERVER_ADDR=127.0.0.1:$(PORT) $(SERVER_BIN) & server=$$!; \
	trap 'kill $$server $$a $$b 2>/dev/null' EXIT INT TERM; \
	sleep 1; \
	$(CLIENT_ENV) $(CLIENT_BIN) & a=$$!; \
	$(CLIENT_ENV) $(CLIENT_BIN) & b=$$!; \
	wait $$a $$b

server: build ## Start only the sync server
	TODO_SERVER_ADDR=127.0.0.1:$(PORT) $(SERVER_BIN)

client: build ## Start one synced client (run in another terminal while `make server` is running)
	$(CLIENT_ENV) $(CLIENT_BIN)

local: build ## Start one client without sync (in-memory only)
	$(CLIENT_BIN)

test: ## Run unit tests
	cargo test

test-sync: build ## Run the two-client round trip against a temporary server
	@TODO_SERVER_ADDR=127.0.0.1:$(TEST_PORT) $(SERVER_BIN) & server=$$!; \
	trap 'kill $$server 2>/dev/null' EXIT INT TERM; \
	sleep 1; \
	TODO_SYNC_URL=http://127.0.0.1:$(TEST_PORT) cargo test --bin gpui-todo -- --ignored two_clients_converge

fmt: ## Format the code
	cargo fmt

lint: ## Run clippy
	cargo clippy --bins --tests
