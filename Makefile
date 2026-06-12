BINARY  := $(shell . ./script/lib && echo $$BINARY)
VERSION := $(shell . ./script/lib && echo $$VERSION)

ifeq ($(DESTDIR),)
SUDO := $(shell [ -w /usr/bin ] || echo sudo)
endif

# Cross-compilation targets (requires `cross` or appropriate toolchains)
TARGETS = \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu

.PHONY: build install build-all release clean

build:
	cargo fetch --locked
	cargo build --release

install: build
	$(SUDO) install -Dm755 target/release/$(BINARY) $(DESTDIR)/usr/bin/$(BINARY)
	target/release/$(BINARY) completions bash > /tmp/$(BINARY).bash
	target/release/$(BINARY) completions zsh > /tmp/_$(BINARY)
	target/release/$(BINARY) completions fish > /tmp/$(BINARY).fish
	$(SUDO) install -Dm644 /tmp/$(BINARY).bash $(DESTDIR)/usr/share/bash-completion/completions/$(BINARY)
	$(SUDO) install -Dm644 /tmp/_$(BINARY) $(DESTDIR)/usr/share/zsh/site-functions/_$(BINARY)
	$(SUDO) install -Dm644 /tmp/$(BINARY).fish $(DESTDIR)/usr/share/fish/vendor_completions.d/$(BINARY).fish
	rm -f /tmp/$(BINARY).bash /tmp/_$(BINARY) /tmp/$(BINARY).fish

build-all:
	cargo fetch --locked
	@set -e; for target in $(TARGETS); do \
		echo "Building for $$target..."; \
		cross build --release --target $$target; \
		os=$$(echo $$target | sed 's/.*-\(linux\|darwin\|freebsd\)-.*/\1/'); \
		arch=$$(echo $$target | sed 's/x86_64/amd64/;s/aarch64/arm64/;s/-.*//' ); \
		mkdir -p dist; \
		cp target/$$target/release/$(BINARY) dist/$(BINARY)-$$os-$$arch; \
	done
	@echo "Artifacts in dist/"


release: build-all
	./script/release-github
	./script/release-aur
	@echo
	@echo "Released v$(VERSION)"
	@echo

clean:
	cargo clean
	rm -rf dist
	rm -rf target/aur
