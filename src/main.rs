#[macro_use]
extern crate hyper;

extern crate hyper_tls;
extern crate futures;
extern crate tokio_core;

use std::io;
use futures::Future;
use futures::stream::Stream;
use hyper::{Client, Request, Method};
use hyper::header::{UserAgent, SetCookie};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;

// Make custom X-Cache-Status header known
header! { (XCacheStatus, "X-Cache-Status") => [String] }

fn main() {
    let threads = 4;
    let user_agent_desktop = "Googlebot (cache warmer)";
    let user_agent_mobile = "Googlebot Android Mobile (cache warmer)";

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let client = Client::configure()
        .keep_alive(true)
        .connector(HttpsConnector::new(threads, &handle).unwrap())
        .build(&handle);

    let lines = lines_from_file("urls.txt").unwrap();

    for l in lines {
        let line = l.unwrap();
        let uri = format!("https://chr4.org/{}", line).parse().unwrap();

        let mut req: hyper::Request = Request::new(Method::Get, uri);
        req.headers_mut().set(UserAgent::new(user_agent_desktop));

        req.headers_mut().set(SetCookie(
            vec![String::from("cacheupdate=true")],
        ));

        let work = client.request(req).and_then(|res| {
            println!(
                "{}: {} {:?}",
                line,
                res.status(),
                res.headers().get::<XCacheStatus>()
            );

            // We need to read out the full body, so the connection can be closed.
            res.body().for_each(|_| Ok(()))
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
