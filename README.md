# mazu.sh 🙏

赛博妈祖庙。开发者一行命令即可祭拜：

```console
$ curl mazu.sh
你是今天第 42 位祭拜媽祖 🙏
```

同一个人当天再拜，返回他第一次的号码：

```console
$ curl mazu.sh
你今天已經拜過了，仍是第 42 位 🙏
```

Rust 写的，**零 crate 依赖**——HTTP 解析、SHA-256、时区换算都在 `src/` 里，`cargo build` 不联网。release 产物 369 KB，常驻内存 ~1.5 MB。

## 接口

| 路径 | 说明 |
| --- | --- |
| `GET /` | 一行纯文本，带 ANSI 颜色 |
| `GET /json` | `{"day","rank","total","repeat","message"}` |
| `GET /healthz` | 健康检查，不计入祭拜 |

参数：`?nocolor` 去色，`?format=json` 等价于 `/json`。

## 计数

- 「今天」按 **Asia/Shanghai** 算，每日零点重新起香。
- 按客户端 IP 认人，走代理时读 `X-Forwarded-For` 第一个地址。落盘的是 `sha256(salt + ip)` 前 16 位十六进制，没有明文 IP。
- 同一位信众当天重复祭拜返回**第一次**的号码，不灌水。
- 存储是 `data/worship.log`，append-only 文本，每行 `日期\t哈希\t号码`。启动时读回内存，之后每来一位新信众追加一行。只留最近 30 天，跨天超期时整份重写。
- 全局一把 `Mutex`，发号原子；单进程模型，多实例会各写各的日志。

## 运行

```bash
cargo run --release          # http://0.0.0.0:3000
cargo test                   # 6 个单测：SHA-256 向量、日期换算、发号去重、HTTP 细节
```

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `PORT` | `3000` | 监听端口 |
| `HOST` | `0.0.0.0` | 监听地址 |
| `MAZU_DATA_DIR` | `data` | 日志目录 |
| `MAZU_SALT` | `mazu` | 哈希盐，上线务必换掉 |

## 部署

不用域名也能先跑起来，平台会送一个二级域名。

### Fly.io（代码零改动）

`Dockerfile` + `fly.toml` 已备好，香火簿挂在 volume `/data` 上，不用时机器缩到 0。

```bash
flyctl auth login                       # 或 export FLY_API_TOKEN=...
flyctl launch --no-deploy --copy-config # 认领 app 名字
flyctl volumes create mazu_data --size 1 --region nrt
flyctl secrets set MAZU_SALT="$(head -c32 /dev/urandom | base64)"
flyctl deploy --remote-only             # 本机没 docker，交给 Fly 远端构建
curl https://mazu.fly.dev
```

注意 `fly.toml` 里刻意只留一台机器：号码要连续，多实例各写各的日志会重号。

### Render / Railway

同一个 `Dockerfile`，网页上连 GitHub 仓库即可。记得挂一块盘到 `/data`，并设 `MAZU_SALT`；免费档没有持久盘的话，实例一重启号码就归零。

### Cloudflare Workers

真 serverless，但当前这版跑不了：Workers 没有 TCP 监听和本地文件。要上得移植成 `workers-rs` 编 WASM，计数换成 Durable Object（正好是强一致的原子计数器）。等于重写一版，且不再是零依赖。

## 域名

`curl mazu.sh` 要真能用，还差两件事：

1. **域名**：注册 `mazu.sh`（`.sh` 是圣赫勒拿国别域名），A 记录指向服务器。
2. **80/443**：拿 Caddy 反代到 `:3000`，证书自动签，真实 IP 靠 `X-Forwarded-For` 透传。

```caddyfile
mazu.sh {
    reverse_proxy 127.0.0.1:3000
}
```

```ini
[Unit]
Description=mazu.sh
After=network.target

[Service]
WorkingDirectory=/root/mazu
Environment=PORT=3000 MAZU_SALT=换成随机串
ExecStart=/root/mazu/target/release/mazu
Restart=always

[Install]
WantedBy=multi-user.target
```
