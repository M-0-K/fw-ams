# framework_lib 0.6.6, patched for fw-ams

Unmodified sources from crates.io (`framework_lib` 0.6.6, BSD-3-Clause, see `LICENSE.md`),
used through `[patch.crates-io]` in the workspace `Cargo.toml`. The crate's `test_bins/` are
left out; only its own tests use them. Every change is marked with `fw-ams patch` in a comment.

Upstream: https://github.com/FrameworkComputer/framework-system. When a release contains
these fixes, delete this directory and the `[patch.crates-io]` entry.

## Changes

1. `src/chromium_ec/windows.rs`: `DeviceIoControl(...).unwrap()` panicked while the global
   `DEVICE` mutex was held. One failed ioctl (seen around standby/resume) poisoned the mutex,
   and every later EC call panicked until the process restarted (fw-ams issue #4). Now the
   lock tolerates poisoning, a failed ioctl returns `EcError::DeviceError`, and the handle is
   closed so the next call reopens the driver. `read_memory` also passed a null pointer as
   `lpBytesReturned`; it now gets a real one.
2. `src/chromium_ec/cros_ec.rs`: same pattern on Linux (`unwrap()` on the file descriptor
   and on the ioctl result under the lock). Errors are returned, the file is reopened after a
   failed ioctl, and a missing `/dev/cros_ec` is logged at debug level instead of printed.
3. `src/smbios.rs`: on Linux the raw SMBIOS table (`/sys/firmware/dmi/tables`) can only be
   read as root. Without it `is_framework()` returned `false` and `CrosEc` refused every
   command, even with access to `/dev/cros_ec` (fw-ams issue #3). `is_framework()` and
   `get_product_name()` now read `/sys/class/dmi/id/{sys_vendor,product_name}` first, which
   is world-readable. New helper `sysfs_dmi()`.
