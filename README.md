A minimal reproducer for an odd Firefox hang.

1. Run with `cargo run`
2. Open Firefox
3. Open the Network tab in the developer tools
4. Disable the browser cache (radio toggle, second row, near the end)
5. Navigate to https://127.0.0.1:3000
6. Acknowledge that these certs are self-signed, and load the page anyway
7. Possibly see some stalled requests

If you don't see any stalled requests, **restarting your computer, and relaunching the crate should work**. On linux, at least, you can get the same effect by logging out and logging back in, skipping the restart.

This repository is based off of Axum's `example-tls-rustls`;
https://github.com/tokio-rs/axum/tree/main/examples/tls-rustls
