extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate tokio_core;

use std::io;
use futures::Future;
use hyper::Client;
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

        let work = client.get(uri).map(|res| {
            println!("{}: {}", line, res.status());
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
