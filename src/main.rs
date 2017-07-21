extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate tokio_core;

use std::io;
use futures::Future;
use hyper::{Client, Request, Method};
use hyper::header::{UserAgent, SetCookie};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;

fn main() {
    let threads = 4;

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let client = Client::configure()
        .connector(HttpsConnector::new(threads, &handle).unwrap())
        .build(&handle);

    let lines = lines_from_file("urls.txt").unwrap();

    for l in lines {
        let line = l.unwrap();
        let uri = format!("https://chr4.org/{}", line).parse().unwrap();

        let mut req: hyper::Request = Request::new(Method::Get, uri);
        req.headers_mut().set(
            UserAgent::new("Googlebot (cache warmer)"),
        );

        req.headers_mut().set(SetCookie(
            vec![String::from("cacheupdate=true")],
        ));

        let work = client.request(req).map(|res| {
            println!("{}: {} (Cache: {})", line, res.status(), res.headers());
        });

        core.run(work).unwrap();
    }
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
