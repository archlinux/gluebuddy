CARGO := cargo

DEBUG := 0
ifeq ($(DEBUG), 0)
	CARGO_OPTIONS := --release --locked
else
	CARGO_OPTIONS :=
endif

.PHONY: all gluebuddy lint

all: gluebuddy lint

gluebuddy:
	$(CARGO) build $(CARGO_OPTIONS)

lint:
	$(CARGO) check
	$(CARGO) clippy --all -- \
		-D clippy::all \
		-D clippy::pedantic \
		-D clippy::restriction \
		-D clippy::correctness \
		-D clippy::complexity \
		-D clippy::nursery \
		-D clippy::perf \
		-D clippy::cargo \
		-D warnings
