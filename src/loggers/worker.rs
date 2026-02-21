use tokio::sync::mpsc;
use sysinfo::System;
use crate::loggers::core::{LogRecord, SysInfo, LogLevel};
use std::process::Command;

pub struct LogWorker {
    receiver: mpsc::Receiver<LogRecord>,
    sys: System,
}

impl LogWorker {
    pub fn new(receiver: mpsc::Receiver<LogRecord>) -> Self {
        // sysinfo 0.30: System::new_all() includes CPU/Memory initialization
        let mut sys = System::new_all();
        sys.refresh_all();
        Self { receiver, sys }
    }

    pub async fn run(mut self) {
        while let Some(mut record) = self.receiver.recv().await {
            // Refresh logic for 0.30
            self.sys.refresh_cpu();
            self.sys.refresh_memory();
            
            record.sys = Some(SysInfo {
                // In 0.30, global_cpu_info() returns the aggregated CPU data
                cpu_usage: self.sys.global_cpu_info().cpu_usage(),
                mem_used_kb: self.sys.used_memory() / 1024,
                load_avg: vec![
                    System::load_average().one,
                    System::load_average().five,
                    System::load_average().fifteen,
                ],
                uptime_secs: System::uptime(),
            });

            if record.level == LogLevel::Fatal {
                self.trigger_alert();
            }

            if let Ok(json) = serde_json::to_string(&record) {
                println!("{}", json);
            }
        }
    }

    fn trigger_alert(&self) {
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("killall").arg("afplay").output();
            let _ = Command::new("afplay").arg("/System/Library/Sounds/Sosumi.aiff").spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("pkill").arg("aplay").output();
            let _ = Command::new("aplay").arg("/usr/share/sounds/alsa/Front_Center.wav").spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("powershell").args(&["-c", "[System.Media.SystemSounds]::Exclamation.Play()"]).spawn();
        }
    }
}
