use clap::{App, Arg, ArgGroup};
use std::process;

#[derive(Debug, Clone)]
pub struct Args {
    pub threads: u32,
    pub delay: u64,
    pub base_uri: String,
    pub uri_file: String,
    pub user_agent: String,
    pub keep_alive: bool,
    pub gzip: bool,
    pub quiet: bool,
    pub progress_bar: bool,
    pub captcha_string: String,
    pub cookies: Vec<(String, String)>,
}

pub fn get_args() -> Args {
    let args = App::new("cache_warmer")
        .version("0.1")
        .about("Fires mass requests to warm up nginx cache")
        .author("Chris Aumann <me@chr4.org>")
        .arg(
            Arg::with_name("uri-file")
                .help("File with URLs to warm up")
                .required(true),
        )
        .arg(
            Arg::with_name("threads")
                .short("t")
                .long("threads")
                .value_name("N")
                .help("Spawn N threads (defaults to 4)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("base-uri")
                .short("b")
                .long("base-uri")
                .value_name("Base URI")
                .help("")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("no-keep-alive")
                .short("n")
                .long("no-keep-alive")
                .help("Do not use keep-alive"),
        )
        .arg(
            Arg::with_name("no-gzip")
                .short("g")
                .long("no-gzip")
                .help("Do not set 'Accept-Encoding: br,gzip,deflate' header"),
        )
        .arg(
            Arg::with_name("delay")
                .short("d")
                .long("delay")
                .value_name("delay")
                .help("Add delay between requests (in ms)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("user-agent")
                .short("u")
                .long("user-agent")
                .value_name("STRING")
                .help("Use custom user-agent")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("mobile")
                .long("mobile")
                .help("Use mobile user agent"),
        )
        .arg(
            Arg::with_name("desktop")
                .long("desktop")
                .help("Use desktop user agent (default)"),
        )
        .group(ArgGroup::with_name("ua-group").args(&["user-agent", "mobile", "desktop"]))
        .arg(
            Arg::with_name("captcha-string")
                .long("captcha-string")
                .value_name("STRING")
                .help("Stop processing when STRING was found in body")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("no-progress-bar")
                .long("no-progress-bar")
                .help("Disable the progress bar"),
        )
        .arg(
            Arg::with_name("quiet")
                .long("quiet")
                .help("Only output errors, no statistics (implies --no-progress-bar)"),
        )
        .group(ArgGroup::with_name("verbosity").args(&["quiet", "no-progress-bar"]))
        .arg(
            Arg::with_name("cookie")
                .short("c")
                .long("cookie")
                .value_name("KEY=VALUE")
                .help("Set cookie")
                .takes_value(true)
                .multiple(true),
        )
        .get_matches();

    let mut cookies = vec![];
    let cookie_args = values_t!(args.values_of("cookie"), String).unwrap_or(vec![]);
    for cookie in cookie_args {
        // Split cookie in key=value pairs
        let vec: Vec<&str> = cookie.splitn(2, '=').collect();

        // Check whether key and value are given
        if vec.len() != 2 {
            println!("Invalid cookie '{}'. Correct syntax is key=val", cookie);
            process::exit(1);
        }

        // Convert result to tuple
        cookies.push((vec[0].to_string(), vec[1].to_string()));
    }

    Args {
        threads: if args.is_present("threads") {
            value_t!(args.value_of("threads"), u32).unwrap_or_else(|e| e.exit())
        } else {
            4
        },
        delay: if args.is_present("delay") {
            value_t!(args.value_of("delay"), u64).unwrap_or_else(|e| e.exit())
        } else {
            0
        },
        cookies: cookies,
        gzip: !args.is_present("no-gzip"),
        keep_alive: !args.is_present("no-keep-alive"),
        quiet: args.is_present("quiet"),
        progress_bar: !args.is_present("no-progress-bar") && !args.is_present("quiet"),
        base_uri: args.value_of("base-uri").unwrap_or("").to_string(),
        captcha_string: args.value_of("captcha-string").unwrap_or("").to_string(),
        uri_file: args.value_of("uri-file").unwrap().to_string(),
        user_agent: match args.value_of("user-agent") {
            Some(user_agent) => user_agent.to_string(),
            None => {
                // Default user agents are adapted from: https://support.google.com/webmasters/answer/1061943
                if args.is_present("mobile") {
                    "Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/41.0.2272.96 Mobile Safari/537.36 (compatible; Googlebot/cache_warmer; +https://example.com)".to_string()
                } else {
                    "Mozilla/5.0 (compatible; Googlebot/cache_warmer; +https://example.com)"
                        .to_string()
                }
            }
        },
    }
}
