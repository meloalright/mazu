mod counter;
mod http;
mod sha256;

use std::env;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use counter::Counter;

/// 同时在处理的连接上限，超了直接断开，免得线程失控
const MAX_INFLIGHT: usize = 256;

fn main() -> std::io::Result<()> {
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("PORT").unwrap_or_else(|_| "3000".into());
    let data_dir = PathBuf::from(env::var("MAZU_DATA_DIR").unwrap_or_else(|_| "data".into()));
    let salt = env::var("MAZU_SALT").unwrap_or_else(|_| "mazu".into());

    let counter = Arc::new(Mutex::new(Counter::open(&data_dir, salt)?));
    let listener = TcpListener::bind(format!("{host}:{port}"))?;
    println!("[mazu] http://{host}:{port}");

    let inflight = Arc::new(AtomicUsize::new(0));

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };

        if inflight.load(Ordering::Relaxed) >= MAX_INFLIGHT {
            drop(stream);
            continue;
        }
        inflight.fetch_add(1, Ordering::Relaxed);

        let counter = Arc::clone(&counter);
        let inflight = Arc::clone(&inflight);
        thread::spawn(move || {
            http::serve(stream, &counter);
            inflight.fetch_sub(1, Ordering::Relaxed);
        });
    }

    Ok(())
}
