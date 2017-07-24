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

#[derive(Debug, PartialEq)]
enum CacheStatus {
    Hit,
    Miss,
    Bypass,
    Unset,
    Unknown,
}

#[derive(Debug)]
struct CacheResource {
    uri: Uri,
    cache_status: CacheStatus,
    captcha: bool,
}

#[derive(Debug)]
pub struct Loader {
    uris_todo: Mutex<Vec<CacheResource>>,
    uris_done: Mutex<Vec<CacheResource>>,
    user_agent: UserAgent,
    cookie: Cookie,
    captcha_string: String,
}

// Make custom X-Cache-Status header known
header! { (XCacheStatus, "X-Cache-Status") => [String] }


impl Loader {
    // TODO: I'd like to have, but I'm lazy to fight to mutable borrow-checker hell
    //       self.bypass()
    //       self.user_agent(ua: &str)
    //       self.captcha_string(s: &str)
    pub fn new(
        uri_file: &str,
        base_uri: &str,
        ua_string: &str,
        captcha_string: &str,
        bypass: bool,
    ) -> Arc<Loader> {
        let mut uris = Vec::new();
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
            uris_todo: Mutex::new(uris),
            uris_done: Mutex::new(Vec::new()),
            user_agent: user_agent,
            cookie: cookie,
            captcha_string: captcha_string.to_string(),
        })
    }

    pub fn length(&self) -> usize {
        let uris = self.uris_todo.lock().unwrap();
        uris.len()
    }

    pub fn length_done(&self) -> usize {
        let uris = self.uris_done.lock().unwrap();
        uris.len()
    }

    fn pop(&self) -> Option<CacheResource> {
        let mut uris = self.uris_todo.lock().unwrap();
        uris.pop()
    }

    fn push(&self, cache_resource: CacheResource) {
        let mut uris = self.uris_done.lock().unwrap();
        uris.push(cache_resource);
    }

    pub fn spawn(&self) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let client = Client::configure()
            .keep_alive(true)
            .connector(HttpsConnector::new(4, &handle).unwrap())
            .build(&handle);

        loop {
            if self.found_captcha() {
                break;
            }

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

                    // Take note if captcha string was found in body
                    if self.captcha_string.len() > 0 && html.contains(&self.captcha_string) {
                        cache_resource.captcha = true;
                    }

                    // Add CacheResource to done
                    self.push(cache_resource);

                    Ok(())
                })
            });

            core.run(work).unwrap();
        }
    }

    pub fn found_captcha(&self) -> bool {
        let uris = self.uris_done.lock().unwrap();
        let captcha_found: Vec<_> = uris.iter().filter(|res| res.captcha).collect();

        match captcha_found.first() {
            Some(_) => true,
            None => false,
        }
    }

    pub fn print_stats(&self) {
        let uris = self.uris_done.lock().unwrap();
        let cache_hit: Vec<_> = uris.iter()
            .filter(|res| res.cache_status == CacheStatus::Hit)
            .collect();
        let cache_miss: Vec<_> = uris.iter()
            .filter(|res| res.cache_status == CacheStatus::Miss)
            .collect();
        let cache_bypass: Vec<_> = uris.iter()
            .filter(|res| res.cache_status == CacheStatus::Bypass)
            .collect();
        let captcha_found: Vec<_> = uris.iter().filter(|res| res.captcha).collect();

        println!("\n");
        if let Some(res) = captcha_found.first() {
            println!("Ran into Captcha at '{}', stopping...", res.uri);
        }
        println!("Processed {} URLs", uris.len());
        println!("\tCache HIT: {}", cache_hit.len());
        println!("\tCache MISS: {}", cache_miss.len());
        println!("\tCache BYPASS: {}", cache_bypass.len());
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
