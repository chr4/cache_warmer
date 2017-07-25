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

use std::thread;
use std::time::Duration;
use pbr::ProgressBar;


fn main() {
    let args = cli::get_args();
    let loader = loader::Loader::new(
        &args.uri_file,
        &args.base_uri,
        &args.user_agent,
        &args.captcha_string,
        args.keep_alive,
        args.bypass,
    ).unwrap_or_else(|err| {
        println!("{}", err);
        std::process::exit(-1);
    });

    println!(
        "Spawning {} threads to warm cache with {} URIs",
        args.threads,
        loader.length(),
    );

    let status_loader = loader.clone();
    let status = thread::spawn(move || {
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

    // Create threads and safe handles
    let mut workers: Vec<_> = vec![];
    for _ in 0..args.threads {
        let loader = loader.clone();
        workers.push(thread::spawn(move || { loader.spawn(); }));
    }

    // Block until all work is done
    for h in workers {
        h.join().expect("Error joining worker threads");
    }

    status.join().expect("Error joining status thread");
    loader.print_stats();
}
