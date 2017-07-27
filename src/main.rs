#[macro_use]
extern crate hyper;

#[macro_use]
extern crate clap;

extern crate hyper_tls;
extern crate futures;
extern crate tokio_core;
extern crate pbr;

mod cli;
mod loader;

use std::{thread, process};
use std::time::{SystemTime, Duration};
use pbr::ProgressBar;


fn main() {
    let args = cli::get_args();
    let main_args = args.clone();
    let loader = loader::Loader::new(args).unwrap_or_else(|err| {
        println!("{}", err);
        process::exit(1);
    });

    if !main_args.quiet {
        println!(
        "Spawning {} threads to warm cache with {} URIs",
        main_args.threads,
        loader.length(),
    );
    }

    // Create vector to store thread handles
    let mut thread_handles: Vec<_> = vec![];

    if main_args.progress_bar {
        let status_loader = loader.clone();
        let handle = thread::spawn(move || {
            let count = status_loader.length() as u64;
            let mut pb = ProgressBar::new(count);

            loop {
                thread::sleep(Duration::from_secs(1));
                pb.set(status_loader.length_done() as u64);

                // Break when captcha was found
                if status_loader.found_captcha() {
                    break;
                }

                // Finish drawing progress bar and exit once all work is done
                if status_loader.length() == 0 {
                    pb.finish();
                    break;
                }
            }
        });

        thread_handles.push(handle);
    }

    // Start the timer
    let start_time = SystemTime::now();

    // Create threads and safe handles
    for _ in 0..main_args.threads {
        let loader = loader.clone();
        let handle = thread::spawn(move || { loader.spawn(); });
        thread_handles.push(handle);
    }

    // Block until all work is done
    for h in thread_handles {
        h.join().expect("Error joining thread");
    }

    if !main_args.quiet {
        loader.print_stats();

        match start_time.elapsed() {
            Ok(duration) => {
                println!(
                    "\nTotal time taken: {:.3}s",
                    duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9
                )
            }
            Err(err) => println!("Error getting elapsed time: {}", err),
        }
    }
}
