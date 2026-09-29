# PolyGlid Development Makefile
#
# Thin wrapper over the scripts in scripts/dev/. All targets run from the
# repository root. Enter the pinned environment first:
#
#   nix develop
#
# Desktop development with live reload:
#   make serve            # dx serve with Rust hot-patching (Subsecond)
#   make serve-no-patch   # dx serve without Rust hot-patching
#   make run              # plain cargo run, no live reload
#
# Knowledge graph (graphify-rs):
#   make graph            # build the codebase knowledge graph
#   make graph-llm        # build, allowing LLM semantic extraction if configured
#   make graph-query Q="how does project deletion work?"
#   make graph-serve      # MCP server so AI agents can query the graph
#
# Verification (what CI runs):
#   make verify           # moon run :format :check :test :build website:bundle automation:validate
#   make check            # cargo check --workspace
#   make test             # cargo test --workspace
#   make fmt              # cargo fmt --all -- --check
#
# Utilities:
#   make help             # list targets
#   make clean-data       # remove the isolated dev data folder (target/dev-data)

.PHONY: help serve serve-no-patch run graph graph-llm graph-query graph-serve \
        verify check test fmt clean-data

help:
	@echo "PolyGlid development targets:"
	@echo "  make serve            desktop app with Dioxus live reload (hot-patch)"
	@echo "  make serve-no-patch   desktop app with live reload, no Rust hot-patching"
	@echo "  make run              desktop app via plain cargo run"
	@echo "  make graph            build the codebase knowledge graph (graphify-rs)"
	@echo "  make graph-llm        build the graph with LLM extraction enabled"
	@echo '  make graph-query Q="question"   query the knowledge graph'
	@echo "  make graph-serve      run the graphify-rs MCP server for agents"
	@echo "  make verify           full CI verification via moon"
	@echo "  make check            cargo check --workspace"
	@echo "  make test             cargo test --workspace"
	@echo "  make fmt              cargo fmt --all -- --check"
	@echo "  make clean-data       remove isolated dev data (target/dev-data)"

serve:
	@scripts/dev/serve-desktop.sh --hot-patch

serve-no-patch:
	@scripts/dev/serve-desktop.sh --no-hot-patch

run:
	@scripts/dev/run-desktop.sh

graph:
	@scripts/dev/graphify-build.sh --no-llm

graph-llm:
	@scripts/dev/graphify-build.sh

graph-query:
	@if [ -z "$(Q)" ]; then \
	  echo "usage: make graph-query Q=\"question\"" >&2; \
	  exit 2; \
	fi
	@scripts/dev/graphify-query.sh "$(Q)"

graph-serve:
	@scripts/dev/graphify-serve.sh

verify:
	@moon run :format :check :test :build website:bundle automation:validate

check:
	@cargo check --workspace

test:
	@cargo test --workspace

fmt:
	@cargo fmt --all -- --check

clean-data:
	@rm -rf target/dev-data
