#[macro_use]
extern crate hyper;

extern crate hyper_tls;
extern crate futures;
extern crate tokio_core;

use std::io;
use std::thread;
use std::sync::{Arc, Mutex};
use futures::Future;
use futures::stream::Stream;
use hyper::{Uri, Client, Request, Method};
use hyper::header::{UserAgent, SetCookie};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;

// Make custom X-Cache-Status header known
header! { (XCacheStatus, "X-Cache-Status") => [String] }

fn main() {
    let threads = 4;
    let user_agent_desktop = "Googlebot (cache warmer)";
    let user_agent_mobile = "Googlebot Android Mobile (cache warmer)";

    let uris = Arc::new(Mutex::new(vec![]));
    let lines = lines_from_file("urls.txt").unwrap();

    // Collect lines, enqueue for workers
    for l in lines {
        let line = l.unwrap();
        let uri: Uri = format!("https://chr4.org/{}", line).parse().unwrap();

        let mut input = uris.lock().unwrap();
        input.push(uri);
    }

    let handles: Vec<_> = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        .iter()
        .map(|_| {
            let x = uris.clone();
            thread::spawn(move || {
                spawn_worker(x, UserAgent::new(user_agent_desktop));
            })
        })
        .collect();

    // Block until all work is done
    for h in handles {
        h.join().unwrap();
    }
}

fn spawn_worker(uris: Arc<Mutex<Vec<Uri>>>, user_agent: UserAgent) {
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

        req.headers_mut().set(SetCookie(
            vec![String::from("cacheupdate=true")],
        ));

        let work = client.request(req).and_then(|res| {
            println!(
                "{}: {} {:?}",
                uri,
                res.status(),
                res.headers().get::<XCacheStatus>()
            );

            // We need to read out the full body, so the connection can be closed.
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
