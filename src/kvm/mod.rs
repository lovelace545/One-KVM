//! KVM Switch Module
//!
//! Controls an external 4-channel KVM switch over a serial (UART) connection.

mod controller;

pub use controller::{KvmController, KvmControllerConfig, KvmSerialHandle};
