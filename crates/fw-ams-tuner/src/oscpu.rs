//! CPU utilisation and effective clock per logical processor from Windows performance
//! counters (PDH). Needs no driver and no admin rights, works for every CPU.
//!
//! Effective clock = nominal frequency × "% Processor Performance" (same as Task Manager).

use crate::types::OsCpuTelemetry;

#[cfg(windows)]
mod imp {
    use super::OsCpuTelemetry;
    use crate::client::wide;
    use windows_sys::Win32::System::Performance::*;

    pub struct OsCpuMonitor {
        query: PDH_HQUERY,
        perf: PDH_HCOUNTER,
        freq: PDH_HCOUNTER,
        time: PDH_HCOUNTER,
    }

    // PDH handles are only used from the thread owning the monitor (behind a mutex)
    unsafe impl Send for OsCpuMonitor {}

    impl OsCpuMonitor {
        pub fn new() -> Option<Self> {
            // SAFETY: out pointers valid; query closed in Drop
            unsafe {
                let mut query = std::ptr::null_mut();
                if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 {
                    return None;
                }
                let add = |path: &str| {
                    let mut c = std::ptr::null_mut();
                    let p = wide(path);
                    (PdhAddEnglishCounterW(query, p.as_ptr(), 0, &mut c) == 0).then_some(c)
                };
                let perf = add(r"\Processor Information(*)\% Processor Performance");
                let freq = add(r"\Processor Information(*)\Processor Frequency");
                let time = add(r"\Processor Information(*)\% Processor Time");
                let (Some(perf), Some(freq), Some(time)) = (perf, freq, time) else {
                    PdhCloseQuery(query);
                    return None;
                };
                // Rate counters need a first sample
                PdhCollectQueryData(query);
                Some(Self { query, perf, freq, time })
            }
        }

        /// Values per instance name ("0,3", "0,_Total", "_Total")
        fn read(&self, counter: PDH_HCOUNTER) -> Vec<(String, f64)> {
            // SAFETY: buffer sized as requested by PDH
            unsafe {
                let (mut size, mut count) = (0u32, 0u32);
                let r = PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, std::ptr::null_mut());
                if r != PDH_MORE_DATA || size == 0 {
                    return vec![];
                }
                let items = (size as usize).div_ceil(std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>());
                let mut buf: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = vec![std::mem::zeroed(); items];
                if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, buf.as_mut_ptr()) != 0 {
                    return vec![];
                }
                buf.iter()
                    .take(count as usize)
                    .map(|it| {
                        let len = (0..).take_while(|i| *it.szName.add(*i) != 0).count();
                        let name = String::from_utf16_lossy(std::slice::from_raw_parts(it.szName, len));
                        (name, it.FmtValue.Anonymous.doubleValue)
                    })
                    .collect()
            }
        }

        pub fn sample(&mut self) -> OsCpuTelemetry {
            // SAFETY: valid query handle
            if unsafe { PdhCollectQueryData(self.query) } != 0 {
                return OsCpuTelemetry::default();
            }
            let index = |name: &str| -> Option<usize> {
                let (_, n) = name.split_once(',')?;
                n.parse().ok()
            };
            let per_thread = |vals: Vec<(String, f64)>| -> Vec<(usize, f64)> {
                let mut v: Vec<(usize, f64)> = vals.into_iter().filter_map(|(n, x)| Some((index(&n)?, x))).collect();
                v.sort_by_key(|(i, _)| *i);
                v
            };
            let total = |vals: &[(String, f64)]| vals.iter().find(|(n, _)| n == "_Total").map(|(_, v)| *v as f32);

            let perf_raw = self.read(self.perf);
            let freq_raw = self.read(self.freq);
            let time_raw = self.read(self.time);
            let base = total(&freq_raw).filter(|f| *f > 0.0);
            let usage = total(&time_raw);
            let perf = per_thread(perf_raw);
            let freq = per_thread(freq_raw);
            let thread_mhz = perf
                .iter()
                .map(|(i, p)| {
                    let f = freq.iter().find(|(j, _)| j == i).map(|(_, f)| *f).unwrap_or(0.0);
                    (f * p / 100.0) as f32
                })
                .collect();
            let thread_usage = per_thread(time_raw).into_iter().map(|(_, v)| v.clamp(0.0, 100.0) as f32).collect();
            OsCpuTelemetry {
                usage_percent: usage.map(|u| u.clamp(0.0, 100.0)),
                thread_mhz,
                thread_usage,
                base_mhz: base,
            }
        }
    }

    impl Drop for OsCpuMonitor {
        fn drop(&mut self) {
            // SAFETY: query opened in new()
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::OsCpuTelemetry;

    pub struct OsCpuMonitor;

    impl OsCpuMonitor {
        pub fn new() -> Option<Self> {
            None
        }
        pub fn sample(&mut self) -> OsCpuTelemetry {
            OsCpuTelemetry::default()
        }
    }
}

pub use imp::OsCpuMonitor;

/// Simulated values for `--mock`
pub fn mock_sample(t: f32, threads: usize) -> OsCpuTelemetry {
    let load = (0.5 + 0.5 * (t / 20.0 * std::f32::consts::TAU).sin()).powi(3);
    OsCpuTelemetry {
        usage_percent: Some(5.0 + load * 80.0),
        thread_mhz: (0..threads)
            .map(|i| {
                let big = i < threads / 2;
                1400.0 + load * if big { 3600.0 } else { 1900.0 } + (t + i as f32).sin() * 80.0
            })
            .collect(),
        thread_usage: (0..threads)
            .map(|i| (load * 90.0 + ((t * 1.7 + i as f32).sin() * 8.0)).clamp(0.0, 100.0))
            .collect(),
        base_mhz: Some(2000.0),
    }
}
