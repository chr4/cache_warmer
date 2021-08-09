use std::io::prelude::*;
use std::{io, thread, time};
use std::path::Path;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use futures::Future;
use futures::stream::Stream;

use hyper::{Client, Method, Request, StatusCode, Uri};
use hyper::header::{Cookie, UserAgent, AcceptEncoding, Encoding, qitem};
use hyper_tls::HttpsConnector;

use tokio_core::reactor::Core;

use cli::Args;

#[derive(Debug, Hash, PartialEq, Eq)]
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
    gzip: bool,
    delay: u64,
}

// Make custom X-Cache-Status header known
header! { (XCacheStatus, "X-Cache-Status") => [String] }

impl Loader {
    pub fn new(args: Args) -> Result<Arc<Loader>, String> {
        let mut uris = Vec::new();
        let lines = match lines_from_file(&args.uri_file) {
            Ok(file) => file,
            Err(err) => return Err(format!("Error opening file {}: {}", &args.uri_file, err)),
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

            let uri: Uri = match format!("{}{}", &args.base_uri, line).parse() {
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

        let user_agent = UserAgent::new(args.user_agent.to_string());

        // Create cookie jar for all given cookies
        let mut cookie_jar = Cookie::new();
        for cookie in args.cookies {
            let (key, value) = cookie;
            cookie_jar.append(key, value);
        }

        Ok(Arc::new(Loader {
            uris_todo: Mutex::new(uris),
            uris_done: Mutex::new(Vec::new()),
            user_agent: user_agent,
            cookie: cookie_jar,
            keep_alive: args.keep_alive,
            gzip: args.gzip,
            delay: args.delay,
            captcha_string: args.captcha_string.to_string(),
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
            if self.gzip {
                req.headers_mut().set(AcceptEncoding(vec![
                    qitem(Encoding::Brotli),
                    qitem(Encoding::Gzip),
                    qitem(Encoding::Deflate),
                ]));
            }

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

            // Sleep before crafing new request when delay is given
            let duration = time::Duration::from_millis(self.delay);
            thread::sleep(duration);
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

        println!();
        if let Some(res) = captcha_found.first() {
            println!("Ran into Captcha at '{}', stopping...", res.uri);
        }
        println!("Processed {} URLs", uris.len());

        let mut cache_status = HashMap::new();
        for uri in uris.iter() {
            let count = cache_status.entry(&uri.cache_status).or_insert(0);
            *count += 1;
        }

        println!("\nX-Cache-Status header statistics:");
        for (key, value) in cache_status {
            println!("\t{:?}: {}", key, value);
        }

        let mut http_status = HashMap::new();
        for uri in uris.iter() {
            let count = http_status.entry(&uri.http_status).or_insert(0);
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
