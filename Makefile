.PHONY: sim check check-ipk ipk smoke setup-env setup-rust

export PATH := $(HOME)/.cargo/bin:$(PATH)
CARGO ?= cargo

WEBOS_NDK ?= $(HOME)/webos-ndk/arm-webos-linux-gnueabi_sdk-buildroot
WEBOS_SDK  = $(WEBOS_NDK)
RUST_TARGET = armv7-unknown-linux-gnueabi
TOOLPREFIX  = $(WEBOS_SDK)/bin/arm-webos-linux-gnueabi-
CC          = $(TOOLPREFIX)gcc
AR          = $(TOOLPREFIX)ar
STRIP       = $(TOOLPREFIX)strip
SYSROOT     = $(WEBOS_SDK)/arm-webos-linux-gnueabi/sysroot
APPID       = com.harbor.player
STAGE       = ipkroot/data/usr/palm/applications/$(APPID)
RUST_TDIR   = target-webos
RUST_LIB    = $(RUST_TDIR)/$(RUST_TARGET)/release/libharbor_app.a
OBJS        = src/main.o src/starfish.o
CFLAGS      = --sysroot=$(SYSROOT) -O2 -fno-omit-frame-pointer -funwind-tables -Wall -Wextra -Isrc -D_GNU_SOURCE
LIBS        = -lSDL2 -lGLESv2 -ldl -lpthread -lm -lrt
CARGO_HOME  ?= $(HOME)/.cargo
RUSTUP_HOME ?= $(HOME)/.rustup
RUST_CODEGEN = -C target-cpu=cortex-a9 -C target-feature=-neon,-soft-float -C force-frame-pointers=yes
RUST_REMAP   = --remap-path-prefix=$(HOME)=/build --remap-path-prefix=$(CARGO_HOME)=/cargo --remap-path-prefix=$(RUSTUP_HOME)=/rustup
RUST_ENV     = RUSTFLAGS="$(RUST_CODEGEN) $(RUST_REMAP)"
NDK_OS      := $(shell uname -s | tr 'A-Z' 'a-z')
NDK_HOST    := $(shell uname -m)
NDK_PLAT    := $(strip $(if $(filter darwin,$(NDK_OS)),$(if $(filter arm64,$(NDK_HOST)),darwin-arm64,$(if $(filter x86_64,$(NDK_HOST)),darwin-x86_64,UNSUPPORTED)),$(if $(and $(filter linux,$(NDK_OS)),$(filter aarch64 arm64,$(NDK_HOST))),linux-aarch64,UNSUPPORTED)))
NDK_DL      := $(HOME)/webos-ndk/downloads
CROSS_ENV   = PATH="$(WEBOS_SDK)/bin:$$PATH" \
	CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABI_LINKER="$(CC)" \
	CC_armv7_unknown_linux_gnueabi="$(CC)" \
	AR_armv7_unknown_linux_gnueabi="$(AR)" \
	PKG_CONFIG_ALLOW_CROSS=1 \
	PKG_CONFIG_SYSROOT_DIR="$(SYSROOT)" \
	PKG_CONFIG_LIBDIR="$(SYSROOT)/usr/lib/pkgconfig" \
	PKG_CONFIG_PATH="$(SYSROOT)/usr/lib/pkgconfig"

define NDK_MISSING
	echo "make ipk: WEBOS_NDK gcc missing at $(CC)"; \
	echo "Official webOS NDK has no x86_64 Linux toolchain."; \
	echo "Run this on macOS or linux-aarch64, or in GitHub Actions ubuntu-24.04-arm."; \
	echo "  make setup-env && make ipk"
endef

sim:
	$(CARGO) build -p harbor-sim
	@echo "built target/debug/harbor-sim"

check:
	$(CARGO) test --workspace
	$(MAKE) check-ipk

check-ipk:
	python3 ci/test_mkipk.py

smoke: sim
	HARBOR_SMOKE_FRAMES=3 SDL_VIDEODRIVER=dummy ./target/debug/harbor-sim

setup-rust:
	rustup target add $(RUST_TARGET)

setup-env:
	@test "$(NDK_PLAT)" != UNSUPPORTED || { \
	  echo "no webOS NDK published for $(NDK_OS)-$(NDK_HOST)."; \
	  echo "Available: darwin-arm64, darwin-x86_64, linux-aarch64."; \
	  echo "This host cannot cross-compile the ARM IPK."; exit 1; }
	$(MAKE) setup-rust
	@if [ -x "$(CC)" ]; then echo "NDK already present at $(WEBOS_SDK)"; exit 0; fi
	mkdir -p $(dir $(WEBOS_SDK)) $(NDK_DL)
	tarball=$$(python3 ci/deps.py fetch webos-ndk --asset $(NDK_PLAT) --into $(NDK_DL)) && \
	  tar xjf "$$tarball" -C $(dir $(WEBOS_SDK))
	cd $(WEBOS_SDK) && ./relocate-sdk.sh
	@echo "NDK ready: $$($(CC) --version | head -1)"

src/%.o: src/%.c src/starfish.h
	@test -x "$(CC)" || { $(NDK_MISSING); exit 1; }
	$(CC) $(CFLAGS) -c $< -o $@

$(RUST_LIB): Cargo.toml crates/harbor-app/Cargo.toml
	@test -x "$(CC)" || { $(NDK_MISSING); exit 1; }
	$(CROSS_ENV) $(RUST_ENV) $(CARGO) rustc -p harbor-app --release --lib --crate-type staticlib \
	  --target $(RUST_TARGET) --target-dir $(RUST_TDIR)

pkg/harbor: $(OBJS) $(RUST_LIB)
	@test -x "$(CC)" || { $(NDK_MISSING); exit 1; }
	$(CROSS_ENV) $(CC) $(CFLAGS) -Wl,--build-id=sha1 -Wl,--gc-sections \
	  $(OBJS) -lm $(RUST_LIB) $(LIBS) -o $@

ipk:
	@test -x "$(CC)" || { $(NDK_MISSING); exit 1; }
	$(MAKE) pkg/harbor
	rm -rf ipkroot/data
	mkdir -p $(STAGE)
	cp pkg/appinfo.json pkg/icon.png pkg/largeIcon.png pkg/splash.png $(STAGE)/
	cp pkg/harbor $(STAGE)/harbor
	$(STRIP) --strip-unneeded $(STAGE)/harbor
	python3 ci/mkipk.py
