use serde::{Deserialize, Serialize};
use typeshare::typeshare;

/// Configuration for the external 4-channel KVM switch connected over serial.
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KvmConfig {
    /// Whether the KVM switch control is enabled.
    pub enabled: bool,
    /// Serial device path (e.g. `/dev/ttyUSB0`, `/dev/ttyAMA0`, `COM3`).
    pub device: String,
    /// Serial baud rate (default 19200).
    pub baud_rate: u32,
}

impl Default for KvmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: String::new(),
            baud_rate: 19200,
        }
    }
}

impl KvmConfig {
    pub fn normalize(&mut self) {
        if self.device.trim().is_empty() {
            self.enabled = false;
        }
    }

    pub fn to_controller_config(&self) -> crate::kvm::KvmControllerConfig {
        crate::kvm::KvmControllerConfig {
            device: self.device.clone(),
            baud_rate: self.baud_rate,
        }
    }
}
