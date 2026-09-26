use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KvmConfig {
    pub enabled: bool,
    pub device: String,
    pub baud_rate: u32,
    pub send_template: String,
    pub recv_template: String,
}

impl Default for KvmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: String::new(),
            baud_rate: 19200,
            send_template: "SW{ch}\\r\\nG{ch2}gA".to_string(),
            recv_template: "G{ch2}gA".to_string(),
        }
    }
}

impl KvmConfig {
    pub fn normalize(&mut self) {
        if self.device.trim().is_empty() {
            self.enabled = false;
        }
        if self.send_template.trim().is_empty() {
            self.send_template = "SW{ch}\\r\\nG{ch2}gA".to_string();
        }
        if self.recv_template.trim().is_empty() {
            self.recv_template = "G{ch2}gA".to_string();
        }
    }

    pub fn to_controller_config(&self) -> crate::kvm::KvmControllerConfig {
        crate::kvm::KvmControllerConfig {
            device: self.device.clone(),
            baud_rate: self.baud_rate,
            send_template: self.send_template.clone(),
            recv_template: self.recv_template.clone(),
        }
    }
}
