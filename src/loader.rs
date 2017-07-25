use std::io::prelude::*;
use std::io;
use std::path::Path;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use futures::Future;
use futures::stream::Stream;

use hyper::{Uri, Client, Request, Method, StatusCode};
use hyper::header::{UserAgent, Cookie};
use hyper_tls::HttpsConnector;

use tokio_core::reactor::Core;

#[derive(Debug, PartialEq)]
enum CacheStatus {
    Hit,
    Miss,
    Bypass,
    Unset,
}

#[derive(Debug)]
struct CacheResource {
    uri: Uri,
    cache_status: CacheStatus,
    http_status: StatusCode,
    captcha: bool,
}

#[derive(Debug)]
pub struct Loader {
    uris_todo: Mutex<Vec<CacheResource>>,
    uris_done: Mutex<Vec<CacheResource>>,
    user_agent: UserAgent,
    cookie: Cookie,
    captcha_string: String,
    keep_alive: bool,
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
        keep_alive: bool,
        bypass: bool,
    ) -> Result<Arc<Loader>, String> {
        let mut uris = Vec::new();
        let lines = match lines_from_file(uri_file) {
            Ok(file) => file,
            Err(err) => return Err(format!("Error opening file {}: {}", uri_file, err)),
        };

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
                cache_status: CacheStatus::Unset,
                http_status: StatusCode::Unregistered(0),
                captcha: false,
            });
        }

        let user_agent = UserAgent::new(ua_string.to_string());

        // Set cacheupdate=true cookie, to bypass (and update) existing cached sites
        let mut cookie = Cookie::new();
        if bypass {
            cookie.append("cacheupdate", "true");
        }

        Ok(Arc::new(Loader {
            uris_todo: Mutex::new(uris),
            uris_done: Mutex::new(Vec::new()),
            user_agent: user_agent,
            cookie: cookie,
            keep_alive: keep_alive,
            captcha_string: captcha_string.to_string(),
        }))
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
        let mut core = match Core::new() {
            Ok(core) => core,
            Err(err) => {
                println!("Error creating Tokio core: {} - stopping this thread!", err);
                return;
            }
        };
        let handle = core.handle();

        let client = Client::configure()
            .keep_alive(self.keep_alive)
            .connector(match HttpsConnector::new(4, &handle) {
                Ok(connector) => connector,
                Err(err) => {
                    println!(
                        "Error creating HttpsConnector: {} - stopping this thread!",
                        err
                    );
                    return;
                }
            })
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
            let mut req: Request = Request::new(Method::Get, cache_resource.uri.clone());
            req.headers_mut().set(self.user_agent.clone());
            req.headers_mut().set(self.cookie.clone());

            let task = client.request(req).and_then(|res| {
                cache_resource.cache_status = match res.headers().get::<XCacheStatus>() {
                    Some(s) => lookup_cache_status(s),
                    None => CacheStatus::Unset,
                };

                cache_resource.http_status = res.status();

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

            match core.run(task) {
                Ok(task) => task,
                Err(err) => println!("Error: {} (URL: {})", err, uri),
            }
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
        let captcha_found: Vec<_> = uris.iter().filter(|res| res.captcha).collect();


        println!("\n");
        if let Some(res) = captcha_found.first() {
            println!("Ran into Captcha at '{}', stopping...", res.uri);
        }
        println!("Processed {} URLs", uris.len());

        println!("\nX-Cache-Status header statistics:");
        for cache_status in vec![
            CacheStatus::Hit,
            CacheStatus::Miss,
            CacheStatus::Bypass,
            CacheStatus::Unset,
        ]
        {

            let results: Vec<_> = uris.iter()
                .filter(|res| res.cache_status == cache_status)
                .collect();
            println!("\t{:?}: {}", cache_status, results.len());
        }


        let mut http_status = HashMap::new();
        for uri in uris.iter() {
            let count = http_status.entry(uri.http_status).or_insert(0);
            *count += 1;
        }

        println!("\nHTTP Status Code statistics:");
        for (key, value) in http_status {
            println!("\t{:?}: {}", key, value);
        }
    }
}

fn lookup_cache_status(status: &str) -> CacheStatus {
    match status {
        "MISS" => CacheStatus::Miss,
        "HIT" => CacheStatus::Hit,
        "BYPASS" => CacheStatus::Bypass,
        _ => CacheStatus::Unset,
    }
}

fn lines_from_file<P>(filename: P) -> Result<io::Lines<io::BufReader<File>>, io::Error>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
