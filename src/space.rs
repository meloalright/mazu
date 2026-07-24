//! 终端里的媽祖廟：选头像、走动、上香、从下方出口离开。
//! 同时在廟里的人共享一个世界，彼此看得见。
//! 只在客户端申请了 PTY 时启用；无 PTY 的连接仍走一次性祭拜。

use std::collections::HashMap;

/// 可选头像，首次进廟时挑一个，之后按公钥记住。
/// 全部是单码位 emoji：ZWJ 组合序列（如 👩‍🦰）在不同终端会拆成两个字形、
/// 宽度从 2 格变 4 格，会把地图撑歪。
pub const AVATARS: &[&str] = &[
    "👩", "👨", "🧑", "👴", "👵", "👸", "🤴", "🧙", "🧚", "🧝", "👳", "🧕", "💂", "👮", "👷",
];

// 場景全用 ASCII，每個圖塊正好 2 個字元，才能和 2 格寬的 emoji 對齊成方格。
/// 灯笼红黄交错，按列号取色
const LANTERN_RED: &str = "\x1b[1;31m[]\x1b[0m";
const LANTERN_YELLOW: &str = "\x1b[1;33m[]\x1b[0m";
/// 廟的红墙
const WALL: &str = "\x1b[31m##\x1b[0m";
/// 神龕前的供桌，横向连成一片。棕色要用 256 色，8 色调色板里没有棕。
const TABLE: &str = "\x1b[38;5;130m==\x1b[0m";
/// 香火：未燃是暗灰兩炷，有人叩拜時轉亮黃加粗
const CANDLE: &str = "\x1b[90m||\x1b[0m";
const FIRE: &str = "\x1b[1;33m||\x1b[0m";
const FLOOR: &str = "\x1b[90m. \x1b[0m";
/// 廟外的空处，用来托出飞檐的层次
const VOID: &str = "  ";

pub const W: usize = 7;
pub const H: usize = 7;

/// 廟的平面图。'.' 可走，其余都挡路。
const MAP: [[char; W]; H] = [
    [' ', ' ', 'L', 'L', 'L', ' ', ' '],
    ['L', 'L', 'L', 'L', 'L', 'L', 'L'],
    ['W', 'B', 'B', 'C', 'B', 'B', 'W'],
    ['W', '.', '.', '.', '.', '.', 'W'],
    ['W', '.', '.', '.', '.', '.', 'W'],
    ['W', '.', '.', '.', '.', '.', 'W'],
    ['W', '.', '.', '.', '.', '.', 'W'],
];

/// 神龕香火的位置，站到它正下方才能上香
const CANDLE_AT: (usize, usize) = (2, 3);
/// 进廟时的落脚点。廟的下方整面敞开，往下走一步即离廟。
const START_AT: (usize, usize) = (6, 3);

pub type Id = u64;

#[derive(Clone)]
struct Pilgrim {
    avatar: usize,
    at: (usize, usize),
    /// 正在叩拜，头像显示为 🙏
    praying: bool,
    /// 叩拜后要显示给本人的那句话
    blessing: Option<String>,
}

/// 廟里此刻的所有人。所有会话共享一份，谁动了都要重画。
#[derive(Default)]
pub struct World {
    pilgrims: HashMap<Id, Pilgrim>,
}

pub enum Action {
    Idle,
    Redraw,
    /// 上香达成，把这次记进香火簿
    Worship,
    /// 离廟，结束会话
    Leave,
}

impl World {
    pub fn join(&mut self, id: Id, avatar: usize) {
        self.pilgrims.insert(
            id,
            Pilgrim {
                avatar,
                at: START_AT,
                praying: false,
                blessing: None,
            },
        );
    }

    pub fn leave(&mut self, id: Id) {
        self.pilgrims.remove(&id);
    }

    pub fn present(&self) -> usize {
        self.pilgrims.len()
    }

    pub fn is_in(&self, id: Id) -> bool {
        self.pilgrims.contains_key(&id)
    }

    pub fn set_blessing(&mut self, id: Id, line: String) {
        if let Some(p) = self.pilgrims.get_mut(&id) {
            p.blessing = Some(line);
        }
    }

    /// 有人正在叩拜，香火就是亮的——别人也看得见
    fn anyone_praying(&self) -> bool {
        self.pilgrims.values().any(|p| p.praying)
    }

    pub fn handle(&mut self, id: Id, key: Key) -> Action {
        let Some(me) = self.pilgrims.get(&id).cloned() else {
            return Action::Idle;
        };

        if me.praying {
            // 拜完按任意键起身，继续自由走动
            if let Some(p) = self.pilgrims.get_mut(&id) {
                p.praying = false;
                p.blessing = None;
            }
            return Action::Redraw;
        }

        match key {
            Key::Quit => Action::Leave,
            // 站在神龕前按空格：點香、叩拜、記香火，一气呵成
            Key::Space if facing_shrine(me.at) => {
                if let Some(p) = self.pilgrims.get_mut(&id) {
                    p.praying = true;
                }
                Action::Worship
            }
            _ => self.step(id, me.at, key),
        }
    }

    fn step(&mut self, id: Id, from: (usize, usize), key: Key) -> Action {
        let (r, c) = from;
        let to = match key {
            Key::Up if r > 0 => (r - 1, c),
            // 下方整面敞开，从最后一排再往下就是出廟
            Key::Down => {
                if r + 1 >= H {
                    return Action::Leave;
                }
                (r + 1, c)
            }
            Key::Left if c > 0 => (r, c - 1),
            Key::Right if c + 1 < W => (r, c + 1),
            _ => return Action::Idle,
        };
        if MAP[to.0][to.1] != '.' {
            return Action::Idle; // 墙、供桌、香火都挡路
        }
        if let Some(p) = self.pilgrims.get_mut(&id) {
            p.at = to;
        }
        Action::Redraw
    }

    /// 画出此刻的廟，视角是 id 这个人
    pub fn render(&self, id: Id) -> String {
        let mut out = String::from("\x1b[2J\x1b[H\x1b[?25l\r\n");
        let lit = self.anyone_praying();

        for (r, row) in MAP.iter().enumerate() {
            out.push_str("  ");
            for (c, tile) in row.iter().enumerate() {
                // 同格有多人时本人优先显示，免得自己被别人盖住
                let here = self
                    .pilgrims
                    .iter()
                    .filter(|(_, p)| p.at == (r, c))
                    .max_by_key(|(pid, _)| u8::from(**pid == id));
                if let Some((_, p)) = here {
                    out.push_str(AVATARS[p.avatar]);
                    continue;
                }
                if (r, c) == CANDLE_AT && lit {
                    out.push_str(FIRE);
                    continue;
                }
                out.push_str(match tile {
                    ' ' => VOID,
                    'L' if c % 2 == 0 => LANTERN_RED,
                    'L' => LANTERN_YELLOW,
                    'W' => WALL,
                    'B' => TABLE,
                    'C' => CANDLE,
                    _ => FLOOR,
                });
            }
            out.push_str("\r\n");
        }

        out.push_str("\r\n");
        match self.pilgrims.get(&id) {
            Some(p) if p.praying => {
                if let Some(line) = &p.blessing {
                    out.push_str(&format!("  \x1b[33m{line}\x1b[0m\r\n"));
                }
                out.push_str("  \x1b[2m按任意鍵起身\x1b[0m\r\n");
            }
            Some(p) if facing_shrine(p.at) => {
                out.push_str("  \x1b[2m↑ 神龕在前，按空格上香\x1b[0m\r\n")
            }
            _ => out.push_str("  \x1b[2m方向鍵走動，走到下方出口即離廟\x1b[0m\r\n"),
        }
        out.push_str(&format!("  \x1b[2m廟中此刻 {} 人\x1b[0m\r\n", self.present()));
        out
    }
}

/// 是否正站在神龕前（香火正下方）
fn facing_shrine(at: (usize, usize)) -> bool {
    at == (CANDLE_AT.0 + 1, CANDLE_AT.1)
}

/// 首次进廟的选头像画面。这时还没进世界，所以状态是会话私有的。
pub struct Choosing {
    cursor: usize,
}

impl Choosing {
    pub fn new() -> Self {
        Self { cursor: 0 }
    }

    /// 返回 Some(下标) 表示选定
    pub fn handle(&mut self, key: Key) -> Option<usize> {
        match key {
            Key::Left => {
                self.cursor = (self.cursor + AVATARS.len() - 1) % AVATARS.len();
                None
            }
            Key::Right => {
                self.cursor = (self.cursor + 1) % AVATARS.len();
                None
            }
            Key::Enter | Key::Space => Some(self.cursor),
            _ => None,
        }
    }

    pub fn render(&self) -> String {
        let mut out = String::from("\x1b[2J\x1b[H\x1b[?25l");
        out.push_str("\r\n  媽祖廟前，先擇一副面容 🙏\r\n\r\n  ");
        for (n, a) in AVATARS.iter().enumerate() {
            if n == self.cursor {
                out.push_str(&format!("\x1b[43;30m {a} \x1b[0m"));
            } else {
                out.push_str(&format!(" {a} "));
            }
        }
        out.push_str("\r\n\r\n  \x1b[2m← → 挑選，Enter 確定\x1b[0m\r\n");
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Space,
    Enter,
    Quit,
    Other,
}

/// 解析终端按键。方向键是 ESC [ A~D 三字节序列。
pub fn parse_keys(buf: &[u8]) -> Vec<Key> {
    let mut keys = Vec::new();
    let mut i = 0;
    while i < buf.len() {
        match buf[i] {
            0x1b if i + 2 < buf.len() && buf[i + 1] == b'[' => {
                keys.push(match buf[i + 2] {
                    b'A' => Key::Up,
                    b'B' => Key::Down,
                    b'C' => Key::Right,
                    b'D' => Key::Left,
                    _ => Key::Other,
                });
                i += 3;
            }
            b' ' => {
                keys.push(Key::Space);
                i += 1;
            }
            b'\r' | b'\n' => {
                keys.push(Key::Enter);
                i += 1;
            }
            0x03 | 0x04 | b'q' => {
                keys.push(Key::Quit);
                i += 1;
            }
            b'w' | b'k' => {
                keys.push(Key::Up);
                i += 1;
            }
            b's' | b'j' => {
                keys.push(Key::Down);
                i += 1;
            }
            b'a' | b'h' => {
                keys.push(Key::Left);
                i += 1;
            }
            b'd' | b'l' => {
                keys.push(Key::Right);
                i += 1;
            }
            _ => {
                keys.push(Key::Other);
                i += 1;
            }
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(w: &mut World, id: Id, at: (usize, usize)) {
        w.pilgrims.get_mut(&id).unwrap().at = at;
    }

    fn one() -> (World, Id) {
        let mut w = World::default();
        w.join(1, 0);
        (w, 1)
    }

    #[test]
    fn walls_block_movement() {
        let (mut w, me) = one();
        put(&mut w, me, (3, 1)); // 左边就是墙
        assert!(matches!(w.handle(me, Key::Left), Action::Idle));
        assert_eq!(w.pilgrims[&me].at, (3, 1));
        put(&mut w, me, (3, 3)); // 上面是香火
        assert!(matches!(w.handle(me, Key::Up), Action::Idle));
    }

    #[test]
    fn bottom_gap_leaves() {
        let (mut w, me) = one();
        // 下方整面都是出口，不只中间一格
        put(&mut w, me, (6, 3));
        assert!(matches!(w.handle(me, Key::Down), Action::Leave));
        put(&mut w, me, (6, 1));
        assert!(matches!(w.handle(me, Key::Down), Action::Leave));
        put(&mut w, me, (6, 5));
        assert!(matches!(w.handle(me, Key::Down), Action::Leave));
    }

    #[test]
    fn one_press_bows_and_rises() {
        let (mut w, me) = one();
        put(&mut w, me, (3, 3));
        assert!(matches!(w.handle(me, Key::Space), Action::Worship));
        // 上香时头像不变，只有香火转亮
        assert!(w.render(me).contains(AVATARS[0]));
        assert!(w.render(me).contains(FIRE));
        assert!(matches!(w.handle(me, Key::Other), Action::Redraw));
        assert!(w.render(me).contains(AVATARS[0]));
    }

    #[test]
    fn space_elsewhere_does_nothing() {
        let (mut w, me) = one();
        put(&mut w, me, (5, 2));
        assert!(matches!(w.handle(me, Key::Space), Action::Idle));
    }

    #[test]
    fn pilgrims_see_each_other() {
        let mut w = World::default();
        w.join(1, 0);
        w.join(2, 5);
        put(&mut w, 2, (4, 1));

        let seen = w.render(1);
        assert!(seen.contains(AVATARS[0]), "看得见自己");
        assert!(seen.contains(AVATARS[5]), "看得见别人");
        assert!(seen.contains("廟中此刻 2 人"));

        w.leave(2);
        assert!(!w.render(1).contains(AVATARS[5]), "走了就看不见了");
    }

    #[test]
    fn one_person_praying_lights_it_for_everyone() {
        let mut w = World::default();
        w.join(1, 0);
        w.join(2, 5);
        put(&mut w, 1, (3, 3));
        w.handle(1, Key::Space);
        assert!(w.render(2).contains(FIRE), "别人也看得见香火亮了");
    }

    #[test]
    fn choosing_then_confirm() {
        let mut c = Choosing::new();
        assert!(c.render().contains("先擇一副面容"));
        assert_eq!(c.handle(Key::Right), None);
        assert_eq!(c.handle(Key::Right), None);
        assert_eq!(c.handle(Key::Enter), Some(2));
    }

    #[test]
    fn arrow_sequences_parse() {
        assert_eq!(parse_keys(b"\x1b[A"), vec![Key::Up]);
        assert_eq!(parse_keys(b"\x1b[B\x1b[C"), vec![Key::Down, Key::Right]);
        assert_eq!(parse_keys(b" "), vec![Key::Space]);
        assert_eq!(parse_keys(b"q"), vec![Key::Quit]);
    }
}
