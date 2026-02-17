BINARY  = voyager-disco
VERSION = $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')

# Cross-compilation targets (requires `cross` or appropriate toolchains)
TARGETS = \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu

.PHONY: build install build-all release clean

build:
	cargo build --release

install: build
	install -Dm755 target/release/$(BINARY) $(DESTDIR)/usr/bin/$(BINARY)

build-all:
	@for target in $(TARGETS); do \
		echo "Building for $$target..."; \
		cross build --release --target $$target; \
		os=$$(echo $$target | sed 's/.*-\(linux\|darwin\|freebsd\)-.*/\1/'); \
		arch=$$(echo $$target | sed 's/x86_64/amd64/;s/aarch64/arm64/;s/-.*//' ); \
		mkdir -p dist; \
		cp target/$$target/release/$(BINARY) dist/$(BINARY)-$$os-$$arch; \
	done
	@echo "Artifacts in dist/"

release: build-all
	@echo "Creating release v$(VERSION)..."
	gh release create v$(VERSION) dist/* \
		--title "v$(VERSION)" \
		--generate-notes
	@echo "Released v$(VERSION)"

clean:
	cargo clean
	rm -rf dist
