use std::sync::{Arc, Mutex};
use futures::Future;
use futures::stream::Stream;
use hyper::{Uri, Client, Request, Method};
use hyper::header::{UserAgent, Cookie};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;

// Make custom X-Cache-Status header known
header! { (XCacheStatus, "X-Cache-Status") => [String] }

pub fn spawn(
    uris: Arc<Mutex<Vec<Uri>>>,
    user_agent: UserAgent,
    captcha_string: &str,
    verbose: bool,
    bypass: bool,
) {
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
        let mut req: Request = Request::new(Method::Get, uri.clone());
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

            res.body().concat2().and_then(move |body| {
                // body is a &[8], so from_utf8_lossy() is required here
                let html = String::from_utf8_lossy(body.as_ref());
                if captcha_string.len() > 0 && html.contains(captcha_string) {
                    println!(
                        "Found '{}' in response body. Stopping thread.",
                        captcha_string
                    );
                }
                Ok(())
            })
        });

        core.run(work).unwrap();
    }
}

// Returns next URI without blocking the mutex for longer than necessary
fn get_next_uri(uris: &Arc<Mutex<Vec<Uri>>>) -> Option<Uri> {
    let mut uris = uris.lock().unwrap();
    uris.pop()
}
