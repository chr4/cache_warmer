use clap::{Arg, App};

#[derive(Debug)]
pub struct Args {
    pub threads: u32,
    pub base_uri: String,
    pub uri_file: String,
    pub user_agent: String,
    pub captcha_string: String,
    pub bypass: bool,
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
            Arg::with_name("user-agent")
                .short("u")
                .long("--user-agent")
                .value_name("STRING")
                .help("User-Agent to use")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("captcha-string")
                .short("c")
                .long("--captcha-string")
                .value_name("STRING")
                .help("Stop processing when STRING was found in body")
                .takes_value(true),
        )
        .arg(Arg::with_name("bypass").short("p").long("--bypass").help(
            "Set cacheupdate cookie to bypass cache",
        ))
        .get_matches();

    Args {
        threads: if args.is_present("threads") {
            value_t!(args.value_of("threads"), u32).unwrap_or_else(|e| e.exit())
        } else {
            4
        },
        bypass: args.is_present("bypass"),
        base_uri: args.value_of("base-uri").unwrap_or("").to_string(),
        captcha_string: args.value_of("captcha-string").unwrap_or("").to_string(),
        uri_file: args.value_of("uri-file").unwrap().to_string(),
        user_agent: args.value_of("user-agent")
            .unwrap_or("Googlebot (cache warmer)")
            .to_string(),
    }
}
