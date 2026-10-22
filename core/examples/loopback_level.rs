//! Manual check: prints the RMS of system audio once per second for 10 s.

use lma_core::capture::system_source;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const SECONDS: u64 = 10;

fn main() {
    let (tx, rx) = mpsc::channel();
    let mut source = match system_source(None) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("capture failed: {error}");
            return;
        }
    };
    if let Err(error) = source.start(tx) {
        eprintln!("capture failed: {error}");
        return;
    }
    let started = Instant::now();
    let mut second = 0;
    let mut sum = 0.0_f64;
    let mut count = 0_usize;
    while second < SECONDS {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(chunk) => {
                sum += chunk
                    .iter()
                    .map(|s| f64::from(*s) * f64::from(*s))
                    .sum::<f64>();
                count += chunk.len();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let elapsed = started.elapsed().as_secs();
        if elapsed > second {
            let rms = if count == 0 {
                0.0
            } else {
                (sum / count as f64).sqrt()
            };
            println!("rms {rms:.4}");
            second = elapsed;
            sum = 0.0;
            count = 0;
        }
    }
    source.stop();
}
