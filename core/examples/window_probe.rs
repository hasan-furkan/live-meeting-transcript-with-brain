//! Manual check: prints how much of a window's first frame is black, 5 times.
//!
//! Usage: `cargo run -p lma_core --example window_probe -- chrome`
//! Nothing is written to disk.

use lma_core::screenshot::{list_windows, probe_window};
use std::error::Error;
use std::time::Duration;

const SAMPLES: u32 = 5;

fn main() -> Result<(), Box<dyn Error>> {
    let Some(needle) = std::env::args().nth(1) else {
        eprintln!("usage: window_probe <app-name-part>");
        return Ok(());
    };
    let needle = needle.to_lowercase();
    let list = list_windows(true, None)?;
    for window in &list.windows {
        let id = window
            .id
            .map_or_else(|| "-".to_string(), |id| id.to_string());
        println!("{id} | {} | {}", window.app, window.title);
    }
    for window in list
        .windows
        .iter()
        .filter(|window| window.app.to_lowercase().contains(&needle))
    {
        let id = window
            .id
            .map_or_else(|| "-".to_string(), |id| id.to_string());
        for sample in 0..SAMPLES {
            if sample > 0 {
                std::thread::sleep(Duration::from_secs(1));
            }
            match probe_window(window) {
                Ok((width, height, share)) => {
                    println!("{id}, {width} x {height}, {share:.4}");
                }
                Err(error) => println!("{id}, {error}"),
            }
        }
    }
    Ok(())
}
