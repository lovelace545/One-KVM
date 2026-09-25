//! KVM Switch Controller
//!
//! Manages serial communication with an external 4-channel KVM switch.
//! Protocol: send "SW{N}\r\n" to switch to channel N; the switch responds
//! with "G0{N}gA" confirming the active channel.

use serialport::SerialPort;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::{AppError, Result};

pub type KvmSerialHandle = Arc<Mutex<Box<dyn SerialPort>>>;

#[derive(Debug, Clone)]
pub struct KvmControllerConfig {
    pub device: String,
    pub baud_rate: u32,
}

impl Default for KvmControllerConfig {
    fn default() -> Self {
        Self {
            device: String::new(),
            baud_rate: 19200,
        }
    }
}

/// How long to wait for a channel confirmation response after sending a switch command.
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(2000);
/// Per-read timeout on the serial port.
const PORT_READ_TIMEOUT: Duration = Duration::from_millis(50);

/// Manages a 4-channel KVM switch connected over UART/serial.
pub struct KvmController {
    config: KvmControllerConfig,
    serial: Mutex<Option<KvmSerialHandle>>,
    current_channel: RwLock<u8>, // 0 = unknown
}

impl KvmController {
    pub fn new(config: KvmControllerConfig) -> Self {
        Self {
            config,
            serial: Mutex::new(None),
            current_channel: RwLock::new(0),
        }
    }

    pub fn disabled() -> Self {
        Self::new(KvmControllerConfig::default())
    }

    pub async fn init(&self) -> Result<()> {
        if self.config.device.trim().is_empty() {
            info!("KVM switch device not configured — disabled");
            return Ok(());
        }

        info!(
            "Initializing KVM switch on {} at {} baud",
            self.config.device, self.config.baud_rate
        );

        let port = serialport::new(&self.config.device, self.config.baud_rate)
            .timeout(PORT_READ_TIMEOUT)
            .open()
            .map_err(|e| {
                AppError::Internal(format!("KVM switch serial port open failed: {}", e))
            })?;

        *self.serial.lock().unwrap() = Some(Arc::new(Mutex::new(port)));
        info!("KVM switch initialized successfully");
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        *self.serial.lock().unwrap() = None;
        info!("KVM switch shutdown complete");
        Ok(())
    }

    /// Returns true when a serial device path has been configured.
    pub fn is_configured(&self) -> bool {
        !self.config.device.trim().is_empty()
    }

    /// Returns the last known active channel (0 = unknown).
    pub async fn current_channel(&self) -> u8 {
        *self.current_channel.read().await
    }

    /// Switch to the given channel (1–4) and return the confirmed channel.
    pub async fn switch_channel(&self, channel: u8) -> Result<u8> {
        if !(1..=4).contains(&channel) {
            return Err(AppError::BadRequest(format!(
                "Invalid KVM channel: {} (expected 1–4)",
                channel
            )));
        }

        let confirmed = {
            let serial_guard = self.serial.lock().unwrap();
            let handle = serial_guard
                .as_ref()
                .cloned()
                .ok_or_else(|| AppError::Internal("KVM switch not initialized".to_string()))?;
            drop(serial_guard);

            let cmd = format!("SW{}\r\nG{:02}gA", channel, channel);
            let expected = format!("G{:02}gA", channel);
            let mut port = handle.lock().unwrap();

            // Drain stale data: one non-blocking read
            let mut drain = [0u8; 256];
            let _ = port.read(&mut drain);

            port.write_all(cmd.as_bytes())
                .map_err(|e| AppError::Internal(format!("KVM switch write failed: {}", e)))?;
            port.flush()
                .map_err(|e| AppError::Internal(format!("KVM switch flush failed: {}", e)))?;

            debug!("KVM switch: sent {:?}, waiting for {:?}", cmd, expected);

            // Read until we see the TARGET channel confirmation or timeout.
            // The switch continuously streams current channel; we only match the
            // target channel, so stale data (old channel) won't false-positive.
            let mut buf: Vec<u8> = Vec::with_capacity(32);
            let mut byte = [0u8; 1];
            let start = std::time::Instant::now();
            let mut found = false;

            while start.elapsed() < RESPONSE_TIMEOUT {
                match port.read(&mut byte) {
                    Ok(0) => continue,
                    Ok(_) => {
                        // Skip null bytes (the switch pads responses with \x00)
                        if byte[0] == 0 { continue; }
                        buf.push(byte[0]);
                        // Keep last 20 bytes to bound memory
                        if buf.len() > 80 {
                            let drain_count = buf.len() - 20;
                            buf.drain(..drain_count);
                        }
                        let text = String::from_utf8_lossy(&buf);
                        if text.contains(expected.as_str()) {
                            found = true;
                            break;
                        }
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        continue
                    },
                    Err(e) => {
                        return Err(AppError::Internal(format!(
                            "KVM switch read failed: {}",
                            e
                        )))
                    }
                }
            }

            if !found {
                // Log what we actually received to help debug
                let text = String::from_utf8_lossy(&buf);
                warn!("KVM switch: received bytes after timeout: {:?}", text);
            }

            if found { Some(channel) } else { None }
        };

        match confirmed {
            Some(ch) => {
                debug!("KVM switch confirmed channel {}", ch);
                *self.current_channel.write().await = ch;
                Ok(ch)
            }
            None => {
                warn!("KVM switch: no confirmation for channel {} within {:?}", channel, RESPONSE_TIMEOUT);
                Err(AppError::Internal(
                    "KVM switch did not confirm the channel switch".to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_controller_is_not_configured() {
        let ctl = KvmController::disabled();
        assert!(!ctl.is_configured());
    }
}
