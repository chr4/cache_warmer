extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate tokio_core;

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

    let uri = "https://chr4.org".parse().unwrap();

    let work = client.get(uri).map(|res| {
        println!("Response: {}", res.status());
    });

    core.run(work).unwrap();
}
