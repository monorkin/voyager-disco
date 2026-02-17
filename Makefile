BINARY  = voyager-disco
VERSION ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")

# Cross-compilation targets (requires `cross` or appropriate toolchains)
TARGETS = \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu \
	x86_64-unknown-freebsd

# Map Rust target triples to release asset names
asset_name = $(BINARY)-$(call target_os,$1)-$(call target_arch,$1)
target_os  = $(if $(findstring linux,$1),linux,$(if $(findstring darwin,$1),darwin,$(if $(findstring freebsd,$1),freebsd,unknown)))
target_arch = $(if $(findstring x86_64,$1),amd64,$(if $(findstring aarch64,$1),arm64,unknown))

.PHONY: build build-all release clean

build:
	cargo build --release

build-all:
	@for target in $(TARGETS); do \
		echo "Building for $$target..."; \
		cross build --release --target $$target; \
		mkdir -p dist; \
		cp target/$$target/release/$(BINARY) dist/$(call asset_name,$$target); \
	done
	@echo "Artifacts in dist/"

release: build-all
	@if [ -z "$(TAG)" ]; then echo "Error: TAG is required. Usage: make release TAG=v0.1.0" >&2; exit 1; fi
	@echo "Creating release $(TAG)..."
	gh release create $(TAG) dist/* \
		--title "$(TAG)" \
		--generate-notes
	@echo "Released $(TAG)"

clean:
	cargo clean
	rm -rf dist
