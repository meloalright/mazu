//! curl mazu.sh 是介绍页，不祭拜；祭拜只走 ssh。
//! 按 UA 分流：命令行给纯文本，浏览器给一张极简暗色落地页。

const SKILL: &str = include_str!("../skill/SKILL.md");

/// 直接拿 🙏🏻 当图标，SVG 里塞个 emoji 交给系统字体渲染
const FAVICON: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\">\
<text x=\"50\" y=\"52\" font-size=\"76\" text-anchor=\"middle\" dominant-baseline=\"central\">🙏🏻</text></svg>";

fn is_terminal(user_agent: &str) -> bool {
    let ua = user_agent.to_ascii_lowercase();
    ["curl", "wget", "httpie", "fetch", "powershell"]
        .iter()
        .any(|c| ua.contains(c))
        || user_agent.is_empty()
}

/// 返回 (content_type, body)
pub fn route(path: &str, user_agent: &str) -> (u16, &'static str, String) {
    match path {
        "/" => {
            if is_terminal(user_agent) {
                (200, "text/plain; charset=utf-8", TEXT_HOME.to_string())
            } else {
                (200, "text/html; charset=utf-8", html_home())
            }
        }
        "/skill" | "/skill/SKILL.md" => {
            (200, "text/markdown; charset=utf-8", SKILL.to_string())
        }
        "/favicon.svg" | "/favicon.ico" => {
            (200, "image/svg+xml; charset=utf-8", FAVICON.to_string())
        }
        "/healthz" => (200, "application/json; charset=utf-8", "{\"ok\":true}".to_string()),
        _ => (404, "text/plain; charset=utf-8", "此路無廟\n".to_string()),
    }
}

const TEXT_HOME: &str = "\
祭拜媽祖 🙏

    ssh mazu.sh █
";

fn html_home() -> String {
    r####"<!doctype html>
<html lang="zh-Hant">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>媽祖 · mazu.sh</title>
<meta name="description" content="賽博媽祖廟，ssh mazu.sh 即可祭拜">
<link rel="icon" href="/favicon.svg">
<style>
  :root { color-scheme: dark; }
  * { box-sizing: border-box; }
  body {
    margin: 0; min-height: 100vh; display: grid; place-items: center;
    background: radial-gradient(120% 120% at 50% 0%, #2a0f0a 0%, #140807 60%);
    color: #f3e2c0; font: 15px/1.7 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    padding: 2rem;
  }
  main { width: 100%; max-width: 640px; }
  h1 { font-size: 2rem; letter-spacing: .1em; margin: 0 0 .75rem; color: #e8b923; }
  .cmd {
    display: block; margin: .5rem 0 1.5rem; padding: .8rem 1rem;
    background: #241210; border: 1px solid #4a2a22; border-radius: 8px;
    color: #f3e2c0; overflow-x: auto; white-space: pre-wrap; word-break: break-all;
  }
  .cmd b { color: #74c0c8; font-weight: normal; }
  .cmd a { color: #74c0c8; }
  /* 提示符只做装饰，选中复制时不带上它 */
  .prompt { color: #8a7a63; user-select: none; -webkit-user-select: none; }
  a { color: #e8b923; }
  footer { margin-top: 3rem; text-align: center; }
  footer img { height: 20px; }
  .cursor {
    display: inline-block; width: .55em; height: 1.1em;
    background: #e8b923; vertical-align: text-bottom;
    animation: blink 1.1s steps(1) infinite;
  }
  @keyframes blink { 50% { opacity: 0; } }
</style>
</head>
<body>
<main>
  <h1>祭拜媽祖 🙏</h1>
  <code class="cmd"><span class="prompt">$ </span><b>ssh</b> mazu.sh <span class="cursor"></span></code>

  <footer>
    <!-- GitHub 官方 star 按钮，异步加载，比 shields.io 稳 -->
    <a class="github-button" href="https://github.com/meloalright/mazu"
       data-icon="octicon-star" data-show-count="true"
       aria-label="Star meloalright/mazu on GitHub">Star</a>
    <noscript><a href="https://github.com/meloalright/mazu">GitHub</a></noscript>
  </footer>
  <script async defer src="https://buttons.github.io/buttons.js"></script>
</main>
</body>
</html>
"####
    .to_string()
}
