//! CLI verb implementations other than `build`. Each submodule exposes a
//! `run(...)` entrypoint that `lib.rs` wraps as the public API surface used
//! by `main.rs`.

pub mod clean;
pub mod new;
pub mod scaffold;
pub mod serve;
pub mod watch;
