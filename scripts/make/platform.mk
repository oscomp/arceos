# Architecture and platform resolving

ifeq ($(PLATFORM),)
  # `PLATFORM` is not specified, use the default platform for each architecture
  ifeq ($(ARCH), x86_64)
    PLAT_NAME := x86_64-qemu-q35
    PLAT_FAMILY := x86-pc
  else ifeq ($(ARCH), aarch64)
    PLAT_NAME := aarch64-qemu-virt
    PLAT_FAMILY := aarch64-qemu-virt
  else ifeq ($(ARCH), riscv64)
    PLAT_NAME := riscv64-qemu-virt
    PLAT_FAMILY := riscv64-qemu-virt
  else ifeq ($(ARCH), loongarch64)
    PLAT_NAME := loongarch64-qemu-virt
    PLAT_FAMILY := loongarch64-qemu-virt
  else
    $(error "ARCH" must be one of "x86_64", "riscv64", "aarch64" or "loongarch64")
  endif
else
  # `PLATFORM` is specified, override the `ARCH` variables
  builtin_platforms_map := \
    x86_64-qemu-q35:x86-pc \
    x86_64-pc-oslab:x86-pc \
    riscv64-qemu-virt:riscv64-qemu-virt \
    aarch64-qemu-virt:aarch64-qemu-virt \
    aarch64-raspi4:aarch64-raspi \
    aarch64-bsta1000b:aarch64-bsta1000b \
    aarch64-phytium-pi:aarch64-phytium-pi \
    loongarch64-qemu-virt:loongarch64-qemu-virt
  platform_map = $(filter $(PLATFORM):%, $(builtin_platforms_map))
  ifneq ($(platform_map),)
    # builtin platform
    _arch := $(word 1,$(subst -, ,$(PLATFORM)))
    PLAT_NAME := $(PLATFORM)
    PLAT_FAMILY := $(word 2, $(subst :, ,$(platform_map)))
  else ifneq ($(wildcard $(PLATFORM)),)
    # custom platform, read the "arch" and "plat-name" fields from the toml file
    _arch :=  $(patsubst "%",%,$(shell axconfig-gen $(PLATFORM) -r arch))
    PLAT_NAME := $(patsubst "%",%,$(shell axconfig-gen $(PLATFORM) -r platform))
    PLAT_FAMILY := $(patsubst "%",%,$(shell axconfig-gen $(PLATFORM) -r family))
  else
    builtin_platforms := $(foreach pair,$(builtin_platforms_map),$(firstword $(subst :, ,$(pair))))
    $(error "PLATFORM" must be one of "$(builtin_platforms)" or a valid path to a toml file)
  endif
  ifeq ($(origin ARCH),command line)
    ifneq ($(ARCH),$(_arch))
      $(error "ARCH=$(ARCH)" is not compatible with "PLATFORM=$(PLATFORM)")
    endif
  endif
  ARCH := $(_arch)
endif

