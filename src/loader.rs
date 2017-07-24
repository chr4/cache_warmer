use std::io::prelude::*;
use std::io;
use std::path::Path;
use std::fs::File;
use std::sync::{Arc, Mutex};

use futures::Future;
use futures::stream::Stream;

use hyper::{Uri, Client, Request, Method};
use hyper::header::{UserAgent, Cookie};
use hyper_tls::HttpsConnector;

use tokio_core::reactor::Core;


#[derive(Debug)]
pub enum CacheStatus {
    Hit,
    Miss,
    Bypass,
    Unset,
    Unknown,
}

#[derive(Debug)]
pub struct CacheResource {
    uri: Uri,
    cache_status: CacheStatus,
    captcha: bool,
}

#[derive(Debug)]
pub struct Loader {
    uris: Mutex<Vec<CacheResource>>,
    user_agent: UserAgent,
    cookie: Cookie,
    captcha_string: String,
}

// Make custom X-Cache-Status header known
header! { (XCacheStatus, "X-Cache-Status") => [String] }


impl Loader {
    // TODO: I'd like to have self.bypass() and self.user_agent(ua: &str),
    //       but this is borrow checker hell
    pub fn new(
        uri_file: &str,
        base_uri: &str,
        ua_string: &str,
        captcha_string: &str,
        bypass: bool,
    ) -> Arc<Loader> {
        let mut uris = Vec::new();
        println!("Loading from {}", uri_file);
        let lines = lines_from_file(uri_file).unwrap();

        for l in lines {
            // Skip erroneous lines
            let line = match l {
                Ok(s) => s,
                Err(err) => {
                    println!("WARN: Error reading line from file: {}", err);
                    continue;
                }
            };

            let uri: Uri = match format!("{}{}", base_uri, line).parse() {
                Ok(s) => s,
                Err(err) => {
                    println!("WARN: Error parsing URI: {}", err);
                    continue;
                }
            };

            uris.push(CacheResource {
                uri: uri,
                cache_status: CacheStatus::Unknown,
                captcha: false,
            });
        }

        let user_agent = UserAgent::new(ua_string.to_string());

        // Set cacheupdate=true cookie, to bypass (and update) existing cached sites
        let mut cookie = Cookie::new();
        if bypass {
            cookie.append("cacheupdate", "true");
        }

        Arc::new(Loader {
            uris: Mutex::new(uris),
            user_agent: user_agent,
            cookie: cookie,
            captcha_string: captcha_string.to_string(),
        })
    }

    pub fn length(&self) -> usize {
        let uris = self.uris.lock().unwrap();
        uris.len()
    }

    pub fn pop(&self) -> Option<CacheResource> {
        let mut uris = self.uris.lock().unwrap();
        uris.pop()
    }

    pub fn spawn(&self) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let client = Client::configure()
            .keep_alive(true)
            .connector(HttpsConnector::new(4, &handle).unwrap())
            .build(&handle);

        loop {
            let mut cache_resource = match self.pop() {
                Some(uri) => uri,
                None => break, // Break when no URLs are left
            };

            let uri = cache_resource.uri.clone();
            let mut req: Request = Request::new(Method::Get, uri);
            req.headers_mut().set(self.user_agent.clone());
            req.headers_mut().set(self.cookie.clone());

            let work = client.request(req).and_then(|res| {
                cache_resource.cache_status = match res.headers().get::<XCacheStatus>() {
                    Some(s) => lookup_cache_status(s),
                    None => CacheStatus::Unset,
                };

                res.body().concat2().and_then(move |body| {
                    // body is a &[8], so from_utf8_lossy() is required here
                    let html = String::from_utf8_lossy(body.as_ref());

                    if self.captcha_string.len() > 0 && html.contains(&self.captcha_string) {
                        cache_resource.captcha = true;
                        println!(
                            "Found '{}' in response body. Stopping thread.",
                            self.captcha_string
                        );
                    }

                    Ok(())
                })
            });

            core.run(work).unwrap();
        }
    }
}

fn lookup_cache_status(status: &str) -> CacheStatus {
    match status {
        "MISS" => CacheStatus::Miss,
        "HIT" => CacheStatus::Hit,
        "BYPASS" => CacheStatus::Bypass,
        _ => CacheStatus::Unknown,
    }
}

fn lines_from_file<P>(filename: P) -> Result<io::Lines<io::BufReader<File>>, io::Error>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
