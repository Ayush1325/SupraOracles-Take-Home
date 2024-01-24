# Simple

This repo contains my solution for Rust Internship Take Home Interview for Supra Oracles. Not sure if I can post the question here, so let's not do that.

## Notes

- You probably want to use the CLI in release mode. DSA signature in debug mode is slow.
- I am using [DSA](https://github.com/RustCrypto/signatures/tree/master/dsa) for signature.
- I am using [bybit API](https://bybit-exchange.github.io/docs/v5/websocket/public/ticker) since most of the crypto exchange APIs do not seem to currently work in India after the ban.
