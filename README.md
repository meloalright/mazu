# mazu 🙏

A Mazu temple you can walk around, over SSH.

```console
$ ssh mazu.sh
```

```
      [][][]
  [][][][][][][]
  ##====||====##
  ##. . . . . ##
  ##. . 👵. . ##
  ##. . . . . ##
  ##. . 👩. . ##

  ↑ 神龕在前，按空格上香
  廟中此刻 2 人
```

Everyone connected at the same time shares one courtyard and sees each other
move, live. Pick a face on your first visit and it is remembered by your public
key. Walk with the arrow keys, stand below the shrine and press Space to offer
incense — the offering glows for the whole hall — then leave through the open
back of the courtyard to end the session.

`curl mazu.sh` just points you at SSH.

### 🎮 Controls

| Key | |
| --- | --- |
| `← →` then `Enter` | pick your avatar (first visit only) |
| arrows, `WASD` or `hjkl` | walk |
| `Space` | offer incense, when standing below the shrine |
| any key | rise again |
| walk off the bottom | leave the temple |
| `q` / `Ctrl-C` | leave immediately |

Visitors without an SSH key can still connect, but there is no way to tell them
apart, so Mazu greets them with instructions for `ssh-keygen` instead of letting
them in. Sessions without a PTY — scripts, agents — get a one-line blessing
rather than the interactive space.

### 🧪 Test

```shell
cargo test
```

### ▶️ Run

```shell
cargo run --release
```

Configuration is via environment variables:

| Variable | Default | Description |
| --- | --- | --- |
| `SSH_PORT` | `2222` | SSH listen port(s), comma-separated for multiple |
| `HTTP_PORT` | `8080` | HTTP listen port |
| `MAZU_DATA_DIR` | `data` | where the worship log and chosen avatars are written |
| `MAZU_SALT` | `mazu` | hashing salt — change it in production |
| `MAZU_MAX_SESSIONS` | `128` | concurrent SSH sessions before new ones are refused |
| `MAZU_HOST_KEY` | `host_key` | OpenSSH host key path (generated on first run if absent) |
| `MAZU_HOST_KEY_PEM` | — | host key as inline PEM; overrides the file, handy for stateless containers |

Visits are counted per public-key fingerprint, stored as
`sha256(salt + fingerprint)` truncated to 16 hex chars — no plaintext.

### 📄 License

[MIT](LICENSE)
