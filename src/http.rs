use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::counter::{Counter, Worship};

const GOLD: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";
const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HEADERS: usize = 64;
const MAX_LINE: u64 = 8 * 1024;

pub fn serve(stream: TcpStream, counter: &Arc<Mutex<Counter>>) {
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let _ = stream.set_write_timeout(Some(TIMEOUT));

    let peer = stream
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "unknown".into());

    let mut out = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(stream);

    let Some((method, target, forwarded)) = parse_request(&mut reader) else {
        let _ = out.write_all(&response(400, "text/plain; charset=utf-8", "看不懂的請求\n"));
        return;
    };

    let full = handle(&method, &target, forwarded.as_deref(), &peer, counter);
    let reply = if method == "HEAD" { headers_only(&full) } else { full };
    let _ = out.write_all(&reply);
    let _ = out.flush();
}

/// 返回 (method, target, x-forwarded-for)
fn parse_request(reader: &mut BufReader<TcpStream>) -> Option<(String, String, Option<String>)> {
    let mut line = String::new();
    reader.by_ref().take(MAX_LINE).read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();

    let mut forwarded = None;
    for _ in 0..MAX_HEADERS {
        let mut header = String::new();
        if reader.by_ref().take(MAX_LINE).read_line(&mut header).ok()? == 0 {
            break;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("x-forwarded-for") {
                forwarded = Some(value.trim().to_string());
            }
        }
    }
    Some((method, target, forwarded))
}

fn handle(
    method: &str,
    target: &str,
    forwarded: Option<&str>,
    peer: &str,
    counter: &Arc<Mutex<Counter>>,
) -> Vec<u8> {
    if method != "GET" && method != "HEAD" {
        return response(405, "text/plain; charset=utf-8", "媽祖只受 GET 之禮\n");
    }

    let (path, query) = target.split_once('?').unwrap_or((target, ""));

    if path == "/healthz" {
        return response(200, "application/json; charset=utf-8", "{\"ok\":true}\n");
    }
    if path != "/" && path != "/json" {
        return response(404, "text/plain; charset=utf-8", "此路無廟\n");
    }

    // 走代理时信 X-Forwarded-For 的第一个地址
    let identity = forwarded
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(peer);

    let result = match counter.lock() {
        Ok(mut c) => c.worship(identity),
        Err(poisoned) => poisoned.into_inner().worship(identity),
    };

    let wants_json = path == "/json" || has_flag(query, "format=json");
    if wants_json {
        return response(200, "application/json; charset=utf-8", &json(&result));
    }

    let color = !has_flag(query, "nocolor");
    let text = if color {
        format!("{GOLD}{}{RESET}\n", line(&result))
    } else {
        format!("{}\n", line(&result))
    };
    response(200, "text/plain; charset=utf-8", &text)
}

fn has_flag(query: &str, flag: &str) -> bool {
    query.split('&').any(|p| p == flag || p.split('=').next() == Some(flag) && !flag.contains('='))
}

fn line(r: &Worship) -> String {
    if r.repeat {
        format!("你今天已經拜過了，仍是第 {} 位 🙏", r.rank)
    } else {
        format!("你是今天第 {} 位祭拜媽祖 🙏", r.rank)
    }
}

fn json(r: &Worship) -> String {
    format!(
        "{{\"day\":\"{}\",\"rank\":{},\"total\":{},\"repeat\":{},\"message\":\"{}\"}}\n",
        r.day,
        r.rank,
        r.total,
        r.repeat,
        line(r)
    )
}

fn response(status: u16, content_type: &str, body: &str) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    let mut out = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         content-type: {content_type}\r\n\
         content-length: {}\r\n\
         cache-control: no-store\r\n\
         connection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

/// HEAD 只回头部，但 content-length 要保留 GET 的值
fn headers_only(full: &[u8]) -> Vec<u8> {
    match full.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(i) => full[..i + 4].to_vec(),
        None => full.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_parse() {
        assert!(has_flag("nocolor", "nocolor"));
        assert!(has_flag("a=1&nocolor", "nocolor"));
        assert!(has_flag("format=json", "format=json"));
        assert!(!has_flag("", "nocolor"));
        assert!(!has_flag("nocolors=1", "nocolor"));
    }

    #[test]
    fn head_keeps_headers_only() {
        let full = response(200, "text/plain", "hi\n");
        let head = headers_only(&full);
        assert!(head.ends_with(b"\r\n\r\n"));
        assert!(String::from_utf8_lossy(&head).contains("content-length: 3"));
    }
}
