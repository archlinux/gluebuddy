CARGO := cargo
INSTALL := install
SED := sed
GIT := git
GPG := gpg

PROJECT=gluebuddy
TARBALLDIR ?= target/release/tarball

DEBUG := 0
ifeq ($(DEBUG), 0)
	CARGO_OPTIONS := --release --locked
else
	CARGO_OPTIONS :=
endif

.PHONY: all gluebuddy test lint release

all: gluebuddy test lint

gluebuddy:
	$(CARGO) build $(CARGO_OPTIONS)

test:
	$(CARGO) test $(CARGO_OPTIONS)

lint:
	$(CARGO) fmt -- --check
	$(CARGO) check
	$(CARGO) clippy --all -- -D warnings


release: all
	$(INSTALL) -d $(TARBALLDIR)
	@read -p 'version> ' VERSION && \
		$(SED) -E "s|^version = .*|version = \"$$VERSION\"|" -i Cargo.toml && \
		$(CARGO) build --release && \
		$(GIT) commit --gpg-sign --message "version: release v$$VERSION" Cargo.toml Cargo.lock && \
		$(GIT) tag --sign --message "version: release v$$VERSION" v$$VERSION && \
		$(GIT) archive --format tar --prefix=gluebuddy-v$$VERSION/ v$$VERSION | gzip -cn > $(TARBALLDIR)/gluebuddy-v$$VERSION.tar.gz && \
		$(GPG) --detach-sign $(TARBALLDIR)/gluebuddy-v$$VERSION.tar.gz && \
		$(GPG) --detach-sign --yes target/release/$(PROJECT) && \
		$(GIT) push --tags origin main && \
		glab release create v$$VERSION $(TARBALLDIR)/$(PROJECT)-v$$VERSION.tar.gz* target/release/$(PROJECT) target/release/$(PROJECT).sig
