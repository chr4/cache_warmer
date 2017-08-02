# cache\_warmer

This tool is indented to be used to warm-up HTTP caches, e.g. services like [nginx](http://nginx.org/).

# Comparison to other tools

While most tools in this area are designed to apply a certain load to a web server (often for a specified time), cache\_warmer is designed to explicitly `GET` a set of URLs from a file to warm-up a cache.


# Usage

Warm-up URLs from a file using a mobile User-Agent

```bash
$ cache_warmer --mobile urls.txt

Spawning 4 threads to warm cache with 10000 URIs
10000 / 10000 [===================================================================] 100.00 % 133.99/s
Processed 10000 URLs

X-Cache-Status header statistics:
        Miss: 8991
        Hit: 1009

HTTP Status Code statistics:
        Ok: 9850
        NotFound: 150

Total time taken: 450.002s
```


# Features

## Multi-Threaded

cache\_warmer is multi-threaded. Threads can be specified using the `--threads` option.


## HTTP keep-alive

It uses keep-alive by default, which can be disabled with `--no-keep-alive`.


## Captcha detection

It supports a `--captcha-string` option, which scans the response body for certain strings to detect (and abort) when running into e.g. captchas.


## Base URI

If your URL file only contains the base URL (like `/products/spoons`), you can add a `--base-uri` to prepend the host and scheme, e.g. `--base-uri https://example.com`


## Request delay

Outgoing requests can be toned down with the `--delay` flag.


## X-Cache-Status header support

If your backend sets the `X-Cache-Status` header, you'll get nice statistics about your cache hit rates at the end of the run.

When using [nginx](http://nginx.org), such a header can be added with this directive:

```nginx
add_header X-Cache-Status $upstream_cache_status;
```

## Custom User-Agent

cache\_warmer defaults to a Googlebot-like User-Agent. You can use the corresponding mobile User-Agent when specifying the `--mobile` flag.

In case you need a custom User-Agent, you can set it with `--user-agent 'Your-User-Agent'`.

## Cookie support

Attach arbitrary cookies with the `--cookie key=value` option.


# Compile

```bash
# Compile locally
cargo build --release

# Cross compile to Linux using Docker
docker-compose up
```


# License and Author

Authors: Chris Aumann

```
Copyright (C) 2017  Chris Aumann

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
```
