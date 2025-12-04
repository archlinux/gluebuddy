CARGO := cargo
INSTALL := install
SED := sed
GIT := git
GPG := gpg

PROJECT=gluebuddy
TARBALLDIR ?= target/release/tarball
CARGO_TARGET_DIR ?= target

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
	$(CARGO) deny check

release: all
	$(INSTALL) -d $(TARBALLDIR)
	@glab --version &>/dev/null
	@glab auth status --hostname gitlab.archlinux.org
	@$(CARGO) pkgid | $(SED) 's/.*#/current version: /'
	@read -p 'version> ' VERSION && \
		$(SED) -E "s|^version = .*|version = \"$$VERSION\"|" -i Cargo.toml && \
		$(CARGO) build --release && \
		$(GIT) commit --gpg-sign --message "version: release v$$VERSION" Cargo.toml Cargo.lock && \
		$(GIT) tag --sign --message "version: release v$$VERSION" v$$VERSION && \
		$(GIT) archive --format tar --prefix=gluebuddy-v$$VERSION/ v$$VERSION | gzip -cn > $(TARBALLDIR)/gluebuddy-v$$VERSION.tar.gz && \
		$(GPG) --detach-sign $(TARBALLDIR)/gluebuddy-v$$VERSION.tar.gz && \
		$(GPG) --detach-sign --yes $(CARGO_TARGET_DIR)/release/$(PROJECT) && \
		$(GIT) push --tags origin main && \
		GITLAB_HOST=gitlab.archlinux.org glab release create v$$VERSION $(TARBALLDIR)/$(PROJECT)-v$$VERSION.tar.gz* $(CARGO_TARGET_DIR)/release/$(PROJECT) $(CARGO_TARGET_DIR)/release/$(PROJECT).sig
