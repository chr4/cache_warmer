#[macro_use]
extern crate hyper;

#[macro_use]
extern crate clap;

extern crate hyper_tls;
extern crate futures;
extern crate tokio_core;
extern crate pbr;

mod cli;

use std::io;
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use pbr::ProgressBar;
use futures::Future;
use futures::stream::Stream;
use hyper::{Uri, Client, Request, Method};
use hyper::header::{UserAgent, Cookie};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;

// Make custom X-Cache-Status header known
header! { (XCacheStatus, "X-Cache-Status") => [String] }


fn main() {
    let args = cli::get_args();
    let user_agent = UserAgent::new(args.user_agent.to_string());

    let uris = Arc::new(Mutex::new(vec![]));
    let lines = lines_from_file(args.uri_file.clone()).unwrap();

    // Collect lines, enqueue for workers
    for l in lines {
        let line = l.unwrap();
        let uri: Uri = format!("{}{}", args.base_uri, line).parse().unwrap();

        let mut uris = uris.lock().unwrap();
        uris.push(uri);
    }

    let len: u64 = {
        let uris = uris.lock().unwrap();
        uris.len() as u64
    };

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

        workers.push(thread::spawn(
            move || { spawn_worker(uris, user_agent, verbose, bypass); },
        ));
    }

    // Block until all work is done
    for h in workers {
        h.join().unwrap();
    }

    status.join().unwrap();
    println!("Done. Warmed up {} URLs.", len);
}

fn spawn_worker(uris: Arc<Mutex<Vec<Uri>>>, user_agent: UserAgent, verbose: bool, bypass: bool) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let client = Client::configure()
        .keep_alive(true)
        .connector(HttpsConnector::new(4, &handle).unwrap())
        .build(&handle);

    loop {
        let uri = match get_next_uri(&uris) {
            Some(uri) => uri,
            None => break,
        };

        // TODO: Can clone() be ommitted somehow?
        let mut req: hyper::Request = Request::new(Method::Get, uri.clone());
        req.headers_mut().set(user_agent.clone());

        // Set cookie to punch through cache
        if bypass {
            let mut cookie = Cookie::new();
            cookie.append("cacheupdate", "true");
            req.headers_mut().set(cookie);
        }

        let work = client.request(req).and_then(|res| {
            if verbose {
                println!(
                    "\t{}: {} {:?}",
                    uri,
                    res.status(),
                    res.headers().get::<XCacheStatus>()
                );
            }

            // We need to read out the full body, so the connection can be closed.
            // TODO: Is there a more efficient way of consuming the body?
            res.body().for_each(|_| Ok(()))
        });

        core.run(work).unwrap();
    }
}

// Returns next URI without blocking the mutex for longer than necessary
fn get_next_uri(uris: &Arc<Mutex<Vec<Uri>>>) -> Option<Uri> {
    let mut uris = uris.lock().unwrap();
    uris.pop()
}


use std::io::prelude::*;
use std::path::Path;
use std::fs::File;

fn lines_from_file<P>(filename: P) -> Result<io::Lines<io::BufReader<File>>, io::Error>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
