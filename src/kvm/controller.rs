//! KVM Switch Controller
use serialport::SerialPort;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::{AppError, Result};

pub type KvmSerialHandle = Arc<Mutex<Box<dyn SerialPort>>>;

#[derive(Debug, Clone)]
pub struct KvmControllerConfig {
    pub device: String,
    pub baud_rate: u32,
    pub send_template: String,
    pub recv_template: String,
}

impl Default for KvmControllerConfig {
    fn default() -> Self {
        Self {
            device: String::new(),
            baud_rate: 19200,
            send_template: "SW{ch}\\r\\nG{ch2}gA".to_string(),
            recv_template: "G{ch2}gA".to_string(),
        }
    }
}

const RESPONSE_TIMEOUT: Duration = Duration::from_millis(2000);
const PORT_READ_TIMEOUT: Duration = Duration::from_millis(10);
const STALE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct KvmController {
    config: KvmControllerConfig,
    serial: Mutex<Option<KvmSerialHandle>>,
    current_channel: Arc<RwLock<u8>>,
    last_seen: Arc<RwLock<Option<Instant>>>,
    stop: Arc<AtomicBool>,
}

impl KvmController {
    pub fn new(config: KvmControllerConfig) -> Self {
        Self {
            config,
            serial: Mutex::new(None),
            current_channel: Arc::new(RwLock::new(0)),
            last_seen: Arc::new(RwLock::new(None)),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn disabled() -> Self {
        Self::new(KvmControllerConfig::default())
    }

    fn fmt_template(&self, tmpl: &str, ch: u8) -> String {
        let mut s = tmpl.replace("{ch}", &ch.to_string());
        s = s.replace("{ch2}", &format!("{:02}", ch));
        s.replace("\\r", "\r").replace("\\n", "\n")
    }

    pub async fn init(&self) -> Result<()> {
        if self.config.device.trim().is_empty() {
            info!("KVM switch device not configured - disabled");
            return Ok(());
        }
        info!("Initializing KVM switch on {} at {} baud", self.config.device, self.config.baud_rate);
        let port = serialport::new(&self.config.device, self.config.baud_rate)
            .timeout(PORT_READ_TIMEOUT)
            .open()
            .map_err(|e| AppError::Internal(format!("KVM switch serial port open failed: {}", e)))?;
        let handle: KvmSerialHandle = Arc::new(Mutex::new(port));
        *self.serial.lock().unwrap() = Some(handle.clone());
        info!("KVM switch initialized successfully");

        let chan_lock = self.current_channel.clone();
        let seen_lock = self.last_seen.clone();
        let stop_flag = self.stop.clone();
        let recv_tmpl = self.config.recv_template.clone();
        tokio::spawn(async move {
            let mut buf: Vec<u8> = Vec::with_capacity(32);
            loop {
                if stop_flag.load(Ordering::Relaxed) { break; }
                let byte_opt = {
                    let mut port = match handle.lock() { Ok(p) => p, Err(_) => break };
                    let mut byte = [0u8; 1];
                    match port.read(&mut byte) {
                        Ok(_) => Some(byte[0]),
                        Err(_) => None,
                    }
                };
                match byte_opt {
                    None => continue,
                    Some(b) => {
                        if b == 0 { continue; }
                        buf.push(b);
                        if buf.len() > 80 { let d = buf.len() - 20; buf.drain(..d); }
                        let text = String::from_utf8_lossy(&buf).to_string();
                        for n in 1..=4u8 {
                            let pat = recv_tmpl.replace("{ch}", &n.to_string()).replace("{ch2}", &format!("{:02}", n));
                            if text.contains(&pat) {
                                let mut cur = chan_lock.write().await;
                                if *cur != n { info!("KVM switch: current channel CH{}", n); }
                                *cur = n;
                                drop(cur);
                                *seen_lock.write().await = Some(Instant::now());
                                buf.clear();
                                break;
                            }
                        }
                    }
                }
            }
            info!("KVM background reader stopped");
        });
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        *self.serial.lock().unwrap() = None;
        info!("KVM switch shutdown complete");
        Ok(())
    }

    pub fn is_configured(&self) -> bool { !self.config.device.trim().is_empty() }

    pub async fn current_channel(&self) -> u8 {
        let seen = *self.last_seen.read().await;
        match seen {
            Some(t) if t.elapsed() < STALE_TIMEOUT => *self.current_channel.read().await,
            Some(t) => {
                warn!("KVM stale: no data for {}s, returning 0", t.elapsed().as_secs());
                0
            }
            None => 0,
        }
    }

    pub async fn switch_channel(&self, channel: u8) -> Result<u8> {
        if !(1..=4).contains(&channel) {
            return Err(AppError::BadRequest(format!("Invalid KVM channel: {} (expected 1-4)", channel)));
        }
        let confirmed = {
            let sg = self.serial.lock().unwrap();
            let handle = sg.as_ref().cloned().ok_or_else(|| AppError::Internal("KVM switch not initialized".to_string()))?;
            drop(sg);
            let cmd = self.fmt_template(&self.config.send_template, channel);
            let expected = self.fmt_template(&self.config.recv_template, channel);
            let mut port = handle.lock().unwrap();
            let mut drain = [0u8; 256];
            let _ = port.read(&mut drain);
            port.write_all(cmd.as_bytes()).map_err(|e| AppError::Internal(format!("KVM write: {}", e)))?;
            port.flush().map_err(|e| AppError::Internal(format!("KVM flush: {}", e)))?;
            let mut buf: Vec<u8> = Vec::with_capacity(32);
            let mut byte = [0u8; 1];
            let start = std::time::Instant::now();
            let mut found = false;
            while start.elapsed() < RESPONSE_TIMEOUT {
                match port.read(&mut byte) {
                    Ok(0) => continue,
                    Ok(_) => {
                        if byte[0] == 0 { continue; }
                        buf.push(byte[0]);
                        if buf.len() > 80 { let d = buf.len() - 20; buf.drain(..d); }
                        if String::from_utf8_lossy(&buf).contains(&expected) { found = true; break; }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut || e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(e) => return Err(AppError::Internal(format!("KVM read: {}", e))),
                }
            }
            if !found { warn!("KVM timeout, received: {:?}", String::from_utf8_lossy(&buf)); }
            if found { Some(channel) } else { None }
        };
        match confirmed {
            Some(ch) => {
                *self.current_channel.write().await = ch;
                *self.last_seen.write().await = Some(Instant::now());
                Ok(ch)
            }
            None => Err(AppError::Internal("KVM switch did not confirm the channel switch".to_string())),
        }
    }
}
