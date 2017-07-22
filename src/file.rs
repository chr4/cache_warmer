use std::io::prelude::*;
use std::io;
use std::path::Path;
use std::fs::File;
use std::sync::{Arc, Mutex};
use hyper::Uri;

pub fn read_uris(base_uri: &str, file: &str) -> (Arc<Mutex<Vec<Uri>>>, u64) {
    let uris = Arc::new(Mutex::new(vec![]));
    let lines = lines_from_file(file).unwrap();

    // Collect lines, enqueue for workers
    for l in lines {
        let line = l.unwrap();
        let uri: Uri = format!("{}{}", base_uri, line).parse().unwrap();

        let mut uris = uris.lock().unwrap();
        uris.push(uri);
    }

    let len: u64 = {
        let uris = uris.lock().unwrap();
        uris.len() as u64
    };

    (uris, len)
}

fn lines_from_file<P>(filename: P) -> Result<io::Lines<io::BufReader<File>>, io::Error>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
