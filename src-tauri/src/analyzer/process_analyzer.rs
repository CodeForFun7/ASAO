use std::collections::{HashMap, VecDeque};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::analyzer::classifier::classify_process;
use crate::analyzer::resource_analyzer::{
    classify_cpu_level, classify_memory_level, determine_process_status,
};
use crate::analyzer::{ProcessInfo, ProcessSnapshotPayload, ProcessStatus, SystemMetrics};
use crate::collector::cpu::CpuCollector;
use crate::collector::memory::MemoryCollector;
use crate::collector::processes::ProcessCollector;

struct PidSampleState {
    prev_cpu_time_100ns: u64,
    prev_disk_io_bytes: u64,
    prev_other_io_bytes: u64,
    recent_cpu_samples: VecDeque<f32>,
}

pub struct ProcessAnalyzerEngine {
    cpu_collector: CpuCollector,
    memory_collector: MemoryCollector,
    process_collector: ProcessCollector,
    pid_states: HashMap<u32, PidSampleState>,
    last_sample_instant: Option<Instant>,
    cached_snapshot: Option<ProcessSnapshotPayload>,
}

impl ProcessAnalyzerEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            cpu_collector: CpuCollector::new(),
            memory_collector: MemoryCollector::new(),
            process_collector: ProcessCollector::new(),
            pid_states: HashMap::with_capacity(512),
            last_sample_instant: None,
            cached_snapshot: None,
        };
        // Prime initial baseline so the very first UI request already has valid deltas
        let _ = engine.collect_and_analyze();
        engine
    }

    pub fn get_latest_or_collect(&mut self) -> Result<ProcessSnapshotPayload, String> {
        if let Some(ref snapshot) = self.cached_snapshot {
            if let Some(last_inst) = self.last_sample_instant {
                if last_inst.elapsed().as_millis() < 750 {
                    return Ok(snapshot.clone());
                }
            }
        }
        self.collect_and_analyze()
    }

    pub fn collect_and_analyze(&mut self) -> Result<ProcessSnapshotPayload, String> {
        let now_instant = Instant::now();
        let elapsed_secs = self
            .last_sample_instant
            .map(|prev| now_instant.duration_since(prev).as_secs_f64().max(0.15))
            .unwrap_or(1.0);
        self.last_sample_instant = Some(now_instant);

        let (sys_cpu_percent, sys_cpu_delta, total_sys_delta_100ns) = self.cpu_collector.sample();
        let mem_sample = self.memory_collector.sample();
        let raw_processes = self.process_collector.collect_all()?;

        let mut active_pids = HashMap::with_capacity(raw_processes.len());
        let mut analyzed_processes = Vec::with_capacity(raw_processes.len());

        let mut attention_count = 0usize;
        let mut protected_count = 0usize;
        let mut high_resource_count = 0usize;

        for raw in raw_processes {
            let pid = raw.pid;

            let (cpu_percent, sustained_cpu, disk_bps, net_bps) =
                if let Some(prev_state) = self.pid_states.remove(&pid) {
                    let cpu_delta_100ns = raw
                        .total_cpu_time_100ns
                        .saturating_sub(prev_state.prev_cpu_time_100ns);

                    let raw_cpu_pct = if total_sys_delta_100ns > 0 {
                        ((cpu_delta_100ns as f64 / total_sys_delta_100ns as f64) * 100.0)
                            .clamp(0.0, 100.0) as f32
                    } else {
                        0.0
                    };
                    let rounded_cpu = (raw_cpu_pct * 10.0).round() / 10.0;

                    let disk_delta = raw
                        .disk_io_bytes
                        .saturating_sub(prev_state.prev_disk_io_bytes);
                    let other_delta = raw
                        .other_io_bytes
                        .saturating_sub(prev_state.prev_other_io_bytes);

                    let disk_rate = (disk_delta as f64 / elapsed_secs) as u64;
                    // Scale non-file I/O transfer rate for network/socket activity representation
                    let net_rate = ((other_delta as f64 / elapsed_secs) * 0.35) as u64;

                    let mut samples = prev_state.recent_cpu_samples;
                    if samples.len() >= 5 {
                        samples.pop_front();
                    }
                    samples.push_back(rounded_cpu);
                    let avg_cpu = samples.iter().sum::<f32>() / (samples.len() as f32);

                    active_pids.insert(
                        pid,
                        PidSampleState {
                            prev_cpu_time_100ns: raw.total_cpu_time_100ns,
                            prev_disk_io_bytes: raw.disk_io_bytes,
                            prev_other_io_bytes: raw.other_io_bytes,
                            recent_cpu_samples: samples,
                        },
                    );

                    (rounded_cpu, avg_cpu, disk_rate, net_rate)
                } else {
                    let mut samples = VecDeque::with_capacity(5);
                    samples.push_back(0.0);
                    active_pids.insert(
                        pid,
                        PidSampleState {
                            prev_cpu_time_100ns: raw.total_cpu_time_100ns,
                            prev_disk_io_bytes: raw.disk_io_bytes,
                            prev_other_io_bytes: raw.other_io_bytes,
                            recent_cpu_samples: samples,
                        },
                    );
                    (0.0, 0.0, 0, 0)
                };

            let classification = classify_process(
                raw.pid,
                &raw.name,
                raw.executable_path.as_deref(),
                raw.publisher.as_deref(),
                raw.product_name.as_deref(),
            );

            let cpu_level = classify_cpu_level(sustained_cpu);
            let memory_level = classify_memory_level(raw.memory_bytes);
            let status = determine_process_status(
                classification.category,
                classification.is_system_critical,
                cpu_percent,
                sustained_cpu,
                raw.memory_bytes,
                disk_bps,
            );

            match status {
                ProcessStatus::Attention => attention_count += 1,
                ProcessStatus::Protected => protected_count += 1,
                ProcessStatus::HighResource => high_resource_count += 1,
                _ => {}
            }

            let publisher = raw.publisher.or(classification.fallback_publisher);
            let product_name = raw.product_name.or(classification.fallback_product);
            let description = raw.file_description.or(classification.description);

            analyzed_processes.push(ProcessInfo {
                pid: raw.pid,
                name: raw.name,
                executable_path: raw.executable_path,
                parent_pid: raw.parent_pid,
                publisher,
                product_name,
                cpu_percent,
                memory_bytes: raw.memory_bytes,
                disk_bytes_per_sec: disk_bps,
                network_bytes_per_sec: net_bps,
                thread_count: raw.thread_count,
                category: classification.category,
                status,
                cpu_level,
                memory_level,
                is_startup: classification.is_startup,
                is_system_critical: classification.is_system_critical,
                is_restricted: raw.is_restricted,
                started_seconds_ago: raw.started_seconds_ago,
                description,
            });
        }

        // Retain only currently running PIDs to keep memory strictly bounded
        self.pid_states = active_pids;

        // Default sort by highest combined resource usage (CPU % + Memory weight)
        analyzed_processes.sort_by(|a, b| {
            let score_a = (a.cpu_percent as f64 * 25.0) + (a.memory_bytes as f64 / (1024.0 * 1024.0));
            let score_b = (b.cpu_percent as f64 * 25.0) + (b.memory_bytes as f64 / (1024.0 * 1024.0));
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let system_status = if sys_cpu_percent > 88.0 || mem_sample.usage_percent > 90.0 {
            "critical".to_string()
        } else if sys_cpu_percent > 70.0 || mem_sample.usage_percent > 80.0 || attention_count > 12 {
            "warning".to_string()
        } else {
            "healthy".to_string()
        };

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let metrics = SystemMetrics {
            cpu_usage_percent: sys_cpu_percent,
            cpu_delta_percent: sys_cpu_delta,
            memory_usage_percent: mem_sample.usage_percent,
            memory_used_bytes: mem_sample.used_bytes,
            memory_total_bytes: mem_sample.total_bytes,
            memory_delta_percent: mem_sample.delta_percent,
            total_processes: analyzed_processes.len(),
            attention_processes: attention_count,
            protected_processes: protected_count,
            high_resource_processes: high_resource_count,
            system_status,
            timestamp_ms,
        };

        let snapshot = ProcessSnapshotPayload {
            processes: analyzed_processes,
            metrics,
        };

        self.cached_snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }
}
