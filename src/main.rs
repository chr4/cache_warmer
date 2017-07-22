#[macro_use]
extern crate hyper;

#[macro_use]
extern crate clap;

extern crate hyper_tls;
extern crate futures;
extern crate tokio_core;
extern crate pbr;

mod cli;
mod file;
mod worker;

use std::thread;
use std::time::Duration;
use pbr::ProgressBar;
use hyper::header::UserAgent;


fn main() {
    let args = cli::get_args();
    let user_agent = UserAgent::new(args.user_agent.to_string());

    let (uris, len) = file::read_uris(&args.base_uri, &args.uri_file);

    println!(
        "Spawning {} threads to warm cache with {} URIs",
        args.threads,
        len
    );

    let clone = uris.clone();
    let status = thread::spawn(move || {
        let mut len = len;
        let mut pb = ProgressBar::new(len);

        loop {
            let new_len: u64 = {
                let uris = clone.lock().unwrap();
                uris.len() as u64
            };

            pb.add(len - new_len);
            len = new_len;
            thread::sleep(Duration::from_secs(1));

            // Break once all work is done
            if len == 0 {
                pb.finish();
                break;
            }
        }
    });

    // Create threads and safe handles
    let mut workers: Vec<_> = vec![];
    for _ in 0..args.threads {
        // Clone values before move
        let uris = uris.clone();
        let user_agent = user_agent.clone();
        let verbose = args.verbose;
        let bypass = args.bypass;

        workers.push(thread::spawn(move || {
            worker::spawn(uris, user_agent, verbose, bypass);
        }));
    }

    // Block until all work is done
    for h in workers {
        h.join().unwrap();
    }

    status.join().unwrap();
    println!("Done. Warmed up {} URLs.", len);
}
