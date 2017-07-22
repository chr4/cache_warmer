#[macro_use]
extern crate hyper;

#[macro_use]
extern crate clap;

extern crate hyper_tls;
extern crate futures;
extern crate tokio_core;
extern crate pbr;

use std::io;
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use pbr::ProgressBar;
use clap::{Arg, App};
use futures::Future;
use futures::stream::Stream;
use hyper::{Uri, Client, Request, Method};
use hyper::header::{UserAgent, Cookie};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;

// Make custom X-Cache-Status header known
header! { (XCacheStatus, "X-Cache-Status") => [String] }

fn main() {
    let args = App::new("cache_warmer")
        .version("0.1")
        .about("Fires mass requests to warm up nginx cache")
        .author("Chris Aumann <me@chr4.org>")
        .arg(
            Arg::with_name("threads")
                .short("t")
                .long("threads")
                .value_name("N")
                .help("Spawn N threads (defaults to 4)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("base-uri")
                .short("b")
                .long("base-uri")
                .value_name("Base URI")
                .help("")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("uri-file")
                .short("f")
                .long("uri-file")
                .value_name("FILE")
                .help("File with URLs to warm up")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("user-agent")
                .short("u")
                .long("--user-agent")
                .value_name("STRING")
                .help("User-Agent to use")
                .takes_value(true),
        )
        .arg(Arg::with_name("bypass").short("c").long("--bypass").help(
            "Set cacheupdate cookie to bypass cache",
        ))
        .arg(Arg::with_name("verbose").short("v").long("verbose").help(
            "Be verbose",
        ))
        .get_matches();

    // Default to 4 threads
    let threads = if args.is_present("threads") {
        value_t!(args.value_of("threads"), u32).unwrap_or_else(|e| e.exit())
    } else {
        4
    };

    let verbose = args.is_present("verbose");
    let bypass = args.is_present("bypass");
    let base_uri = args.value_of("base-uri").unwrap_or("");
    let uri_file = args.value_of("uri-file").unwrap();

    let ua_string = args.value_of("user-agent").unwrap_or(
        "Googlebot (cache warmer)",
    );
    let user_agent = UserAgent::new(ua_string.to_string());

    let uris = Arc::new(Mutex::new(vec![]));
    let lines = lines_from_file(uri_file).unwrap();

    // Collect lines, enqueue for workers
    for l in lines {
        let line = l.unwrap();
        let uri: Uri = format!("{}{}", base_uri, line).parse().unwrap();

        let mut uris = uris.lock().unwrap();
        uris.push(uri);
    }

    let len: u64 = {
        let uris = uris.lock().unwrap();
        uris.len() as u64
    };

    println!(
        "Spawning {} threads to warm cache with {} URIs",
        threads,
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
    for _ in 0..threads {
        let uris = uris.clone();
        let user_agent = user_agent.clone();
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
