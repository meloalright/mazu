//! mazu.sh 一肩挑：
//!   ssh mazu.sh       → 祭拜（认公钥指纹，本地计数）
//!   curl mazu.sh      → 介绍页
//!   curl mazu.sh/skill→ SKILL.md
//! 香火簿是本地文件，跑在 Fly 单实例上，挂 volume 持久化。

mod counter;
mod sha256;
mod site;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use counter::Counter;
use russh::keys::{HashAlg, PrivateKey};
use russh::server::{Auth, Handler, Msg, Server as _, Session};
use russh::MethodKind;
use russh::{Channel, ChannelId};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ssh_ports: Vec<u16> = env_ports("SSH_PORT", "2222");
    let http_ports: Vec<u16> = env_ports("HTTP_PORT", "8080");
    let host_key_path =
        PathBuf::from(std::env::var("MAZU_HOST_KEY").unwrap_or_else(|_| "host_key".into()));
    let data_dir = PathBuf::from(std::env::var("MAZU_DATA_DIR").unwrap_or_else(|_| "data".into()));
    let salt = std::env::var("MAZU_SALT").unwrap_or_else(|_| "mazu".into());

    let counter = Arc::new(Mutex::new(Counter::open(&data_dir, salt)?));

    let config = Arc::new(russh::server::Config {
        inactivity_timeout: Some(TIMEOUT),
        auth_rejection_time: std::time::Duration::from_secs(1),
        keys: vec![load_or_create_host_key(&host_key_path)?],
        ..Default::default()
    });

    let temple = Temple {
        counter: Arc::clone(&counter),
        fingerprint: None,
    };

    let mut tasks = Vec::new();

    for port in ssh_ports {
        let socket = TcpListener::bind(("0.0.0.0", port)).await?;
        let config = Arc::clone(&config);
        let mut temple = temple.clone();
        println!("[mazu] ssh  门开在 {port}");
        tasks.push(tokio::spawn(async move {
            if let Err(e) = temple.run_on_socket(config, &socket).await {
                eprintln!("[mazu] ssh {port} 挂了: {e}");
            }
        }));
    }

    for port in http_ports {
        let socket = TcpListener::bind(("0.0.0.0", port)).await?;
        println!("[mazu] http 门开在 {port}");
        tasks.push(tokio::spawn(async move { serve_http(socket).await }));
    }

    for t in tasks {
        let _ = t.await;
    }
    Ok(())
}

fn env_ports(name: &str, default: &str) -> Vec<u16> {
    std::env::var(name)
        .unwrap_or_else(|_| default.into())
        .split(',')
        .filter_map(|v| v.trim().parse().ok())
        .collect()
}

/// Fly 在 443 终结 TLS 后转发明文 HTTP 到这里，所以只做纯 HTTP。
async fn serve_http(socket: TcpListener) {
    loop {
        let Ok((mut stream, _)) = socket.accept().await else {
            continue;
        };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = match stream.read(&mut buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };
            let req = String::from_utf8_lossy(&buf[..n]);
            let mut lines = req.lines();
            let (method, path) = match lines.next().and_then(parse_request_line) {
                Some(v) => v,
                None => return,
            };
            let ua = lines
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("user-agent").then(|| v.trim())
                })
                .unwrap_or("");

            let (status, ctype, body) = if method != "GET" && method != "HEAD" {
                (405, "text/plain; charset=utf-8", "媽祖只受 GET 之禮\n".to_string())
            } else {
                site::route(&path, ua)
            };

            let head = format!(
                "HTTP/1.1 {status} {}\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n",
                reason(status),
                body.len()
            );
            let _ = stream.write_all(head.as_bytes()).await;
            if method != "HEAD" {
                let _ = stream.write_all(body.as_bytes()).await;
            }
            let _ = stream.shutdown().await;
        });
    }
}

fn parse_request_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?;
    let path = target.split('?').next().unwrap_or(target).to_string();
    Some((method, path))
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    }
}

/// 主机密钥必须固定，换了所有拜过的人都会看到大红警告。
/// 优先读环境变量里的 OpenSSH 私钥全文，这样跑在无状态容器里也不用挂盘。
fn load_or_create_host_key(path: &PathBuf) -> Result<PrivateKey, Box<dyn std::error::Error>> {
    if let Ok(pem) = std::env::var("MAZU_HOST_KEY_PEM") {
        if !pem.trim().is_empty() {
            return Ok(PrivateKey::from_openssh(pem.replace("\\n", "\n").as_bytes())?);
        }
    }
    if path.exists() {
        return Ok(PrivateKey::read_openssh_file(path)?);
    }
    let key = PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)?;
    key.write_openssh_file(path, russh::keys::ssh_key::LineEnding::LF)?;
    println!("[mazu] 已生成主机密钥 {}", path.display());
    Ok(key)
}

#[derive(Clone)]
struct Temple {
    counter: Arc<Mutex<Counter>>,
    /// 本次连接用的公钥指纹，认证时记下
    fingerprint: Option<String>,
}

impl russh::server::Server for Temple {
    type Handler = Self;
    fn new_client(&mut self, _peer: Option<SocketAddr>) -> Self {
        self.clone()
    }
    fn handle_session_error(&mut self, error: russh::Error) {
        eprintln!("[mazu] 会话出错: {error}");
    }
}

impl Handler for Temple {
    type Error = russh::Error;

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    /// 来者皆是信众，不设门槛；公钥只用来认人，不做授权
    async fn auth_publickey(
        &mut self,
        _user: &str,
        key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        self.fingerprint = Some(key.fingerprint(HashAlg::Sha256).to_string());
        Ok(Auth::Accept)
    }

    /// ssh 客户端总是先试 none。这里必须拒掉并要求 publickey，
    /// 否则认证在客户端出示公钥之前就成功了，所有人都会变成同一个 anonymous。
    async fn auth_none(&mut self, _user: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Reject {
            proceed_with_methods: Some(
                [MethodKind::PublicKey, MethodKind::KeyboardInteractive]
                    .as_slice()
                    .into(),
            ),
            partial_success: false,
        })
    }

    /// 没有公钥的信众走这条路，不问任何问题直接放行，身份记为 anonymous
    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        _user: &str,
        _submethods: &str,
        _response: Option<russh::server::Response<'a>>,
    ) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        _cw: u32,
        _rh: u32,
        _pw: u32,
        _ph: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.bless(channel, session)
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        _data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.bless(channel, session)
    }
}

impl Temple {
    /// 上一炷香：本地记一笔，把那句话写回去然后关门
    fn bless(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), russh::Error> {
        session.channel_success(channel)?;

        let identity = self
            .fingerprint
            .clone()
            .unwrap_or_else(|| "anonymous".to_string());
        let line = {
            let mut c = self.counter.lock().unwrap_or_else(|e| e.into_inner());
            let w = c.worship(&identity);
            if w.visits > 1 {
                format!("你今天第 {} 次祭拜媽祖，今天共 {} 位信眾 🙏", w.visits, w.total)
            } else {
                format!("你是今天第 {} 位祭拜媽祖 🙏", w.rank)
            }
        };

        session.data(channel, format!("{line}\r\n").into_bytes())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}
