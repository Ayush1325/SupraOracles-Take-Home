# Simple

This repo contains my solution for Rust Internship Take Home Interview for Supra Oracles. Not sure if I can post the question here, so let's not do that.

## Notes

- You probably want to use the CLI in release mode. DSA signature in debug mode is slow.
- I am using [DSA](https://github.com/RustCrypto/signatures/tree/master/dsa) for signature.
- I am using [bybit API](https://bybit-exchange.github.io/docs/v5/websocket/public/ticker) since most of the crypto exchange APIs do not seem to currently work in India after the ban.

# Help

Here is the CLI help page

```shell
❯ ./target/release/simple --help
A simple CLI tool for Supra Oracles take home.

Usage: simple [OPTIONS] --mode <MODE>

Options:
      --mode <MODE>
          Possible values:
          - cache: Fetch from websocket and cache
          - read:  Read from cache file

      --times <TIMES>
          Time in seconds for reading from websocket

          [default: 10]

      --file <FILE>
          Path to cache file

          [default: data.json]

      --clients <CLIENTS>
          Number of clients to spawn

          [default: 5]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
