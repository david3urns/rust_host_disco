/*
VERSION 1.1
Host discovery tool created by David Burns. This script will accept an IP address with CIDR notation
from the user, convert it from string to Ipv4Addr and int, then iterate through all possible addresses
given the IP/CIDR combination. It prints every discovered host IP to the terminal.

TODO:
Add threading
*/

#![allow(unused_comparisons)]


use std::io::{self, Write};
use std::net::Ipv4Addr;
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

use threadpool::ThreadPool;
use num_cpus;

/// ---------------------------------------------------------------------------
/// Helper: print a nice boxed banner
/// ---------------------------------------------------------------------------
fn banner(title: &str) {
    let h = "═";
    let v = "║";
    let tl = "╔";
    let tr = "╗";
    let bl = "╚";
    let br = "╝";

    let w = title.len();
    let top = format!("{}{}{}{}{}", tl, h, h.repeat(w), h, tr);
    let bottom = format!("{}{}{}{}{}", bl, h, h.repeat(w), h, br);
    println!("{}", top);
    println!("{} {} {}", v, title, v);
    println!("{}", bottom);
}

/// ---------------------------------------------------------------------------
/// Helper: clear the terminal
/// ---------------------------------------------------------------------------
fn clear_screen() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    io::stdout().flush().unwrap();
}

/// ---------------------------------------------------------------------------
/// Validate `IP/CIDR`
/// ---------------------------------------------------------------------------
fn validate_ip_cidr(input: &str) -> Result<(Ipv4Addr, u8), String> {
    let parts: Vec<&str> = input.split('/').collect();
    if parts.len() != 2 {
        return Err("expected <IP>/<CIDR>".into());
    }

    let ip: Ipv4Addr = parts[0]
        .trim()
        .parse()
        .map_err(|_| "invalid IPv4 address".to_string())?;

    let cidr: u8 = parts[1]
        .trim()
        .parse()
        .map_err(|_| "CIDR must be a number".to_string())?;
    if cidr > 32 {
        return Err("CIDR must be in the range 0‑32".into());
    }

    Ok((ip, cidr))
}

/// ---------------------------------------------------------------------------
/// Loop‑back detection (only a warning)
/// ---------------------------------------------------------------------------
fn is_loopback_network(ip: Ipv4Addr, cidr: u8) -> bool {
    ip.octets()[0] == 127 && cidr >= 8
}

/// ---------------------------------------------------------------------------
/// Platform‑specific flag for “one ping packet”
/// ---------------------------------------------------------------------------
#[cfg(unix)]
fn ping_one_flag() -> &'static str {
    "-c"
}
#[cfg(windows)]
fn ping_one_flag() -> &'static str {
    "-n"
}

/// ---------------------------------------------------------------------------
/// Perform a single ping, returning true if the output looks successful
/// ---------------------------------------------------------------------------
fn ping_is_up(addr: Ipv4Addr) -> bool {
    let out = Command::new("ping")
        .arg(addr.to_string())
        .arg(ping_one_flag())
        .arg("1")
        .stdout(Stdio::piped())
        .output()
        .expect("failed to spawn ping");

    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout.contains("1 received")
        || stdout.contains("1 packets received")
        || stdout.contains("Received = 1")
}

/// ---------------------------------------------------------------------------
/// Main – multithreaded, ordered, with a live progress indicator
/// ---------------------------------------------------------------------------
fn main() {
    clear_screen();
    banner("Network Host Discovery");
    println!();

    // --------------------------------------------------------------
    // Prompt for CIDR input
    // --------------------------------------------------------------
    print!("Please enter an IP address with CIDR notation (e.g. 192.168.1.0/24): ");
    io::stdout().flush().unwrap();

    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap();
    let line = line.trim();

    // --------------------------------------------------------------
    // Validate
    // --------------------------------------------------------------
    let (base_ip, cidr) = match validate_ip_cidr(line) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Input validation failed: {e}");
            return;
        }
    };

    if is_loopback_network(base_ip, cidr) {
        eprintln!("NOTE: The supplied network is inside the 127.0.0.0/8 loop‑back range.");
        eprintln!("All hosts in this range will appear \"up\" because they resolve locally.\n");
    }

    // --------------------------------------------------------------
    // Compute the address range (skip .0 and .255)
    // --------------------------------------------------------------
    let host_bits = 32 - cidr;
    let total_hosts: u32 = 1u32 << host_bits;          // includes .0 and .255
    let netmask: u32 = (!0u32) << host_bits;
    let network: u32 = u32::from(base_ip) & netmask;
    let host_count = (total_hosts - 2) as usize; // .0 and .255 are omitted

    // --------------------------------------------------------------
    // Shared data structures
    // --------------------------------------------------------------
    // 1️⃣ Ordered result vector (None = not finished yet)
    let results: Arc<Mutex<Vec<Option<bool>>>> = Arc::new(Mutex::new(vec![None; host_count]));

    // 2️⃣ Atomic counters for the progress thread
    let scanned = Arc::new(AtomicUsize::new(0));
    let up_cnt = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));

    // --------------------------------------------------------------
    // Thread‑pool (size = number of logical CPUs)
    // --------------------------------------------------------------
    let pool = ThreadPool::new(num_cpus::get());

    // --------------------------------------------------------------
    // Progress‑display thread
    // --------------------------------------------------------------
    let prog_scanned = Arc::clone(&scanned);
    let prog_total = host_count;
    let prog_finished = Arc::clone(&finished);

    let progress_handle = thread::spawn(move || {
        while !prog_finished.load(Ordering::Relaxed) {
            let done = prog_scanned.load(Ordering::Relaxed);
            let percent = (done as f64 / prog_total as f64) * 100.0;
            // carriage‑return keeps us on a single line
            print!("\rScanned {}/{} ({:.1}%)", done, prog_total, percent);
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_millis(200));
        }
        // make sure the line ends before the summary is printed
        println!();
    });

    // --------------------------------------------------------------
    // Submit a job for every host (the pool limits concurrency)
    // --------------------------------------------------------------
    for host_offset in 1..(total_hosts - 1) {
        let idx = (host_offset - 1) as usize;
        let addr = Ipv4Addr::from(network | host_offset);

        // cloned handles for the closure
        let results_clone = Arc::clone(&results);
        let scanned_clone = Arc::clone(&scanned);
        let up_clone = Arc::clone(&up_cnt);

        pool.execute(move || {
            let up = ping_is_up(addr);

            // store result in the correct slot
            {
                let mut guard = results_clone.lock().unwrap();
                guard[idx] = Some(up);
            }

            // update progress counters
            scanned_clone.fetch_add(1, Ordering::Relaxed);
            if up {
                up_clone.fetch_add(1, Ordering::Relaxed);
            }
        });
    }

    // --------------------------------------------------------------
    // Wait for the pool to finish all work
    // --------------------------------------------------------------
    pool.join(); // blocks until the work queue is empty

    // Signal the progress thread to stop and wait for it
    finished.store(true, Ordering::Relaxed);
    let _ = progress_handle.join();

    // --------------------------------------------------------------
    // Ordered final output (hosts that are up appear in green)
    // --------------------------------------------------------------
    println!();
    banner("Results");
    println!("\nThe following IP addresses responded to ping:");

    let guard = results.lock().unwrap();
    let mut up_total = 0usize;
    for (i, entry) in guard.iter().enumerate() {
        let host_offset = (i as u32) + 1;
        let addr = Ipv4Addr::from(network | host_offset);

        if let Some(true) = entry {
            up_total += 1;
            // green text
            println!("\x1b[0;32m{} is up\x1b[0m", addr);
        } else {
            // red text for down hosts (optional)
            println!("\x1b[31m{} is down\x1b[0m", addr);
        }
    }

    println!(
        "\nScanned a total of {} IP addresses, of which {} were up.",
        host_count, up_total
    );
}

