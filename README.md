# mazu.sh 🙏

賽博媽祖廟。祭拜走 ssh，`curl` 是介紹頁。

## 祭拜

```console
$ ssh mazu.sh
你是今天第 42 位祭拜媽祖 🙏
```

一天可以拜很多次，第二次起會告訴你今天拜了第幾次：

```console
$ ssh mazu.sh
你今天第 2 次祭拜媽祖，今天共 87 位信眾 🙏
```

來者皆是信眾，無需密碼。按公鑰指紋認人，同一把金鑰當天再拜會累加次數。封了 22 埠的網路走 `ssh -p 2222 mazu.sh`。

## 介紹頁 / skill

```console
$ curl mazu.sh          # 命令列給純文字，瀏覽器給落地頁
$ curl mazu.sh/skill    # 給 AI agent 裝的上香技能
```

一行給 Claude Code 裝上，之後跟它說「拜一下媽祖」它會自己去 ssh：

```bash
mkdir -p ~/.claude/skills/mazu && curl -sL mazu.sh/skill -o ~/.claude/skills/mazu/SKILL.md
```

## 架構

一個 Rust 程式，同時開 SSH 與 HTTP 兩個監聽：

| 入口 | 埠 | 處理 |
| --- | --- | --- |
| `ssh mazu.sh` | 22 / 2222 | 祭拜，russh，認公鑰指紋 |
| `curl mazu.sh` | 80 / 443 | 介紹頁，按 UA 分流純文字 / HTML |
| `curl mazu.sh/skill` | 80 / 443 | 內嵌的 SKILL.md |

香火簿是本地 append-only 檔案（`counter.rs`），跑在 Fly.io 單實例上，掛 volume 持久化，443 的 TLS 由 Fly 終結。`mazu.sh` 灰雲直連 Fly，所以同一主機名能同時回應 ssh 和 https。

認人靠 ssh 公鑰指紋（沒有公鑰時退回 anonymous），加鹽雜湊後才落盤，只存 16 位十六進位，沒有明文。

## 開發

```bash
cargo run --release      # ssh :2222  http :8080
cargo test
```

| 變數 | 預設 | 說明 |
| --- | --- | --- |
| `SSH_PORT` | `2222` | ssh 監聽埠，逗號分隔可多個 |
| `HTTP_PORT` | `8080` | http 監聽埠 |
| `MAZU_DATA_DIR` | `data` | 香火簿目錄 |
| `MAZU_SALT` | `mazu` | 雜湊鹽，上線務必換掉 |
| `MAZU_HOST_KEY_PEM` | — | 主機金鑰全文；不設則用 `MAZU_HOST_KEY` 指向的檔案 |

## 部署

```bash
fly deploy --remote-only
```
