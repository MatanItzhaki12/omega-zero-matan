//! Playing without a network. What the site answers is kept on the card, so a sign-in and a
//! game played before can start again with no network: the sign-in, the game's identity, its
//! achievements and the unlocks it had. An achievement earned with no network is kept on the card
//! too, in `pending.txt`, and the client is told it was awarded; the next time the site answers,
//! every kept unlock is sent, each saying how long ago it was earned, as RetroArch sends them.
//!
//! The store is `Config/retroachievements/` on the card: `cache/` holds the answers, by what
//! they answer, and `pending.txt` the unlocks to send, one a line.

use std::ffi::c_int;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::lock;

/// Where every request is sent.
pub const URL: &str = "https://retroachievements.org/dorequest.php";
const FORM: &str = "application/x-www-form-urlencoded";
const PENDING: &str = "pending.txt";

/// Posts a form to an address with its content type, and gives the status and body back.
type Fetch<'a> = &'a dyn Fn(&str, &str, &str) -> (c_int, Vec<u8>);

static STORE: Mutex<Option<PathBuf>> = Mutex::new(None);
/// The game a session was last started for, whose kept unlocks an unlock offline joins.
static GAME: Mutex<Option<String>> = Mutex::new(None);
/// The kept unlocks are being sent.
static SENDING: AtomicBool = AtomicBool::new(false);
/// Held across every change to pending.txt: the network's threads add to it as the sync takes
/// from it.
static PENDING_FILE: Mutex<()> = Mutex::new(());

/// Keeps answers and unlocks in `dir` from now on.
pub fn set_store(dir: &Path) {
    let _ = std::fs::create_dir_all(dir.join("cache"));
    *lock(&STORE) = Some(dir.to_path_buf());
}

fn store() -> Option<PathBuf> {
    lock(&STORE).clone()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A form's fields, decoded: `+` is a space and `%XX` a byte.
pub fn fields(form: &str) -> Vec<(String, String)> {
    let decode = |s: &str| {
        let b = s.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'+' => out.push(b' '),
                b'%' if i + 2 < b.len() => {
                    let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
                    match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                        Some(v) => {
                            out.push(v);
                            i += 2;
                        }
                        None => out.push(b'%'),
                    }
                }
                c => out.push(c),
            }
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    };
    form.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (decode(k), decode(v)),
            None => (decode(p), String::new()),
        })
        .collect()
}

fn field<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.is_empty())
}

/// The name an answer is kept by: what it answers, for the answers a game started offline
/// needs. None for the rest.
pub fn cache_key(fields: &[(String, String)]) -> Option<String> {
    let of = |what: &str| field(fields, what);
    let key = match of("r")? {
        "login2" => format!("login-{}", of("u")?.to_lowercase()),
        "gameid" => format!("gameid-{}", of("m")?),
        "achievementsets" => format!("achievementsets-{}", of("m").or_else(|| of("g"))?),
        "patch" => format!("patch-{}", of("g")?),
        "startsession" => format!("startsession-{}", of("g")?),
        _ => return None,
    };
    Some(
        key.chars()
            .map(|c| match c.is_ascii_alphanumeric() || c == '-' {
                true => c,
                false => '_',
            })
            .collect(),
    )
}

fn cached(dir: &Path, key: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join("cache").join(format!("{key}.json"))).ok()
}

fn keep(dir: &Path, key: &str, body: &[u8]) {
    let _ = std::fs::create_dir_all(dir.join("cache"));
    let path = dir.join("cache").join(format!("{key}.json"));
    let part = path.with_extension("part");
    if std::fs::write(&part, body).is_ok() {
        let _ = std::fs::rename(&part, &path);
    }
}

fn succeeded(body: &[u8]) -> bool {
    let text = String::from_utf8_lossy(body);
    text.contains("\"Success\":true")
}

/// A request is about to go out: notes the game a session starts for.
pub fn requested(post: &str) {
    let f = fields(post);
    if field(&f, "r") == Some("startsession") {
        *lock(&GAME) = field(&f, "g").map(str::to_string);
    }
}

/// The site answered `post`: the answer is kept, where a game started offline would need it.
pub fn answered(post: &str, status: c_int, body: &[u8]) {
    let (Some(dir), true) = (store(), status == 200 && succeeded(body)) else {
        return;
    };
    if let Some(key) = cache_key(&fields(post)) {
        keep(&dir, &key, body);
    }
}

/// The answer to `post` with no network, where there is one: a kept answer, or for an unlock,
/// the unlock kept to be sent and the client told it was awarded, with the scores it had.
pub fn answer_offline(post: &str, scores: (u32, u32)) -> Option<(c_int, Vec<u8>)> {
    let f = fields(post);
    let dir = store();
    match field(&f, "r")? {
        // only a sign-in with the token the kept answer holds: never a password unchecked
        "login2" => {
            let token = field(&f, "t")?;
            let body = cached(dir.as_deref()?, &cache_key(&f)?)?;
            String::from_utf8_lossy(&body)
                .contains(&format!("\"Token\":\"{token}\""))
                .then_some((200, body))
        }
        "awardachievement" => {
            let dir = dir?;
            let id: u32 = field(&f, "a")?.parse().ok()?;
            let hardcore = field(&f, "h") == Some("1");
            let line = [
                id.to_string(),
                u8::from(hardcore).to_string(),
                field(&f, "m").unwrap_or("").to_string(),
                field(&f, "u")?.to_string(),
                field(&f, "t")?.to_string(),
                now().to_string(),
            ]
            .join("\t");
            let _held = lock(&PENDING_FILE);
            let mut text = std::fs::read_to_string(dir.join(PENDING)).unwrap_or_default();
            text.push_str(&line);
            text.push('\n');
            std::fs::write(dir.join(PENDING), text).ok()?;
            if let Some(game) = lock(&GAME).clone() {
                note_unlock(&dir, &game, id, hardcore);
            }
            let body = format!(
                "{{\"Success\":true,\"Score\":{},\"SoftcoreScore\":{},\"AchievementID\":{id}}}",
                scores.0, scores.1
            );
            Some((200, body.into_bytes()))
        }
        // the client only says it is still playing
        "ping" => Some((200, b"{\"Success\":true}".to_vec())),
        _ => cached(dir.as_deref()?, &cache_key(&f)?).map(|body| (200, body)),
    }
}

/// An unlock earned offline joins the game's kept unlocks, so a game started offline again
/// counts it unlocked and does not award it twice.
fn note_unlock(dir: &Path, game: &str, id: u32, hardcore: bool) {
    let key = format!("startsession-{game}");
    let Some(body) = cached(dir, &key) else {
        return;
    };
    let mut text = String::from_utf8_lossy(&body).into_owned();
    let lists: &[&str] = match hardcore {
        true => &["\"HardcoreUnlocks\":[", "\"Unlocks\":["],
        false => &["\"Unlocks\":["],
    };
    let entry = format!("{{\"ID\":{id},\"When\":{}}}", now());
    for list in lists {
        if let Some(at) = text.find(list) {
            let at = at + list.len();
            let comma = if text[at..].starts_with(']') { "" } else { "," };
            text.insert_str(at, &format!("{entry}{comma}"));
        }
    }
    keep(dir, &key, text.as_bytes());
}

/// One unlock kept to be sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub id: u32,
    pub hardcore: bool,
    pub hash: String,
    pub user: String,
    pub token: String,
    pub when: u64,
}

fn parse(line: &str) -> Option<Pending> {
    let p: Vec<&str> = line.split('\t').collect();
    let [id, hardcore, hash, user, token, when] = p[..] else {
        return None;
    };
    Some(Pending {
        id: id.parse().ok()?,
        hardcore: hardcore == "1",
        hash: hash.to_string(),
        user: user.to_string(),
        token: token.to_string(),
        when: when.parse().ok()?,
    })
}

/// The unlocks kept to be sent, oldest first.
pub fn pending() -> Vec<Pending> {
    let Some(dir) = store() else {
        return Vec::new();
    };
    std::fs::read_to_string(dir.join(PENDING))
        .unwrap_or_default()
        .lines()
        .filter_map(parse)
        .collect()
}

/// Sends the kept unlocks, on a thread of its own, once at a time: each the site answers leaves
/// the file; with the network gone again the rest stay. `award` makes an unlock's request, the
/// seconds since it was earned given; `fetch` sends it; `done` hears how many were sent.
pub fn send_pending(
    award: impl Fn(&Pending, u32) -> Option<String> + Send + 'static,
    fetch: impl Fn(&str, &str, &str) -> (c_int, Vec<u8>) + Send + 'static,
    done: impl FnOnce(usize) + Send + 'static,
) {
    if pending().is_empty() || SENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("omega-cheevos-sync".into())
        .spawn(move || {
            let sent = send_now(&award, &fetch);
            SENDING.store(false, Ordering::Release);
            if sent > 0 {
                done(sent);
            }
        });
    if spawned.is_err() {
        SENDING.store(false, Ordering::Release);
    }
}

fn send_now(award: &dyn Fn(&Pending, u32) -> Option<String>, fetch: Fetch) -> usize {
    let Some(dir) = store() else {
        return 0;
    };
    let mut sent = 0;
    for p in pending() {
        let ago = now().saturating_sub(p.when).min(u32::MAX as u64) as u32;
        let Some(post) = award(&p, ago) else {
            continue;
        };
        let (status, _) = fetch(URL, &post, FORM);
        match status {
            // answered: awarded, or already had, or never to be; it leaves the file
            200 => sent += 1,
            // the network is gone again: this one and the rest wait for the next time
            s if s <= 0 => break,
            _ => continue,
        }
        drop_pending(&dir, &p);
    }
    sent
}

/// Takes one unlock out of the file, as it stands now: an unlock kept while sending stays.
fn drop_pending(dir: &Path, gone: &Pending) {
    let _held = lock(&PENDING_FILE);
    let text = std::fs::read_to_string(dir.join(PENDING)).unwrap_or_default();
    let mut dropped = false;
    let rest: String = text
        .lines()
        .filter(|line| {
            let same = !dropped && parse(line).as_ref() == Some(gone);
            dropped |= same;
            !same
        })
        .fold(String::new(), |mut rest, line| {
            rest.push_str(line);
            rest.push('\n');
            rest
        });
    let _ = std::fs::write(dir.join(PENDING), rest);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The store is one for the process: the tests that use it hold this.
    static TEST_STORE: Mutex<()> = Mutex::new(());

    fn with_store(f: impl FnOnce(&Path)) {
        let _held = lock(&TEST_STORE);
        let d = tempfile::tempdir().unwrap();
        set_store(d.path());
        f(d.path());
        *lock(&STORE) = None;
        *lock(&GAME) = None;
    }

    #[test]
    fn a_form_decodes_its_escapes() {
        let f = fields("r=login2&u=Ash+Ketchum&t=a%2Fb%3D&x");
        assert_eq!(field(&f, "u"), Some("Ash Ketchum"));
        assert_eq!(field(&f, "t"), Some("a/b="));
        assert_eq!(field(&f, "x"), None);
    }

    #[test]
    fn answers_are_kept_by_what_they_answer() {
        let key = |post: &str| cache_key(&fields(post));
        assert_eq!(key("r=login2&u=Ash&t=tok").as_deref(), Some("login-ash"));
        assert_eq!(
            key("r=achievementsets&u=a&t=b&m=0123abcd").as_deref(),
            Some("achievementsets-0123abcd")
        );
        assert_eq!(
            key("r=startsession&u=a&t=b&g=515").as_deref(),
            Some("startsession-515")
        );
        assert_eq!(key("r=awardachievement&a=1"), None);
        assert_eq!(
            key("r=gameid&m=..%2F..%2Fx").as_deref(),
            Some("gameid-______x")
        );
    }

    #[test]
    fn a_sign_in_offline_takes_the_kept_answer_only_for_its_own_token() {
        with_store(|_| {
            let body = br#"{"Success":true,"User":"Ash","Token":"tok123","Score":5}"#;
            answered("r=login2&u=Ash&p=secret", 200, body);
            assert_eq!(
                answer_offline("r=login2&u=Ash&t=tok123", (0, 0)),
                Some((200, body.to_vec()))
            );
            assert_eq!(answer_offline("r=login2&u=Ash&t=other", (0, 0)), None);
            assert_eq!(answer_offline("r=login2&u=Ash&p=guess", (0, 0)), None);
        });
    }

    #[test]
    fn a_refusal_is_not_kept() {
        with_store(|_| {
            answered("r=patch&g=1", 200, br#"{"Success":false}"#);
            answered("r=patch&g=1", 500, br#"{"Success":true}"#);
            assert_eq!(answer_offline("r=patch&g=1", (0, 0)), None);
            answered("r=patch&g=1", 200, br#"{"Success":true,"PatchData":{}}"#);
            assert!(answer_offline("r=patch&g=1", (0, 0)).is_some());
        });
    }

    #[test]
    fn an_unlock_offline_is_kept_awarded_and_counted_unlocked_next_time() {
        with_store(|dir| {
            answered(
                "r=startsession&u=Ash&t=tok&g=515",
                200,
                br#"{"Success":true,"HardcoreUnlocks":[],"Unlocks":[{"ID":7,"When":1}]}"#,
            );
            requested("r=startsession&u=Ash&t=tok&g=515");
            let (status, body) = answer_offline(
                "r=awardachievement&u=Ash&t=tok&a=42&h=1&m=abcd&v=x",
                (100, 20),
            )
            .expect("the unlock was not taken offline");
            assert_eq!(status, 200);
            let body = String::from_utf8(body).unwrap();
            assert!(body.contains("\"Success\":true"), "{body}");
            assert!(body.contains("\"AchievementID\":42"), "{body}");
            assert!(body.contains("\"Score\":100"), "{body}");
            let kept = pending();
            assert_eq!(kept.len(), 1);
            assert_eq!(
                (kept[0].id, kept[0].hardcore, kept[0].hash.as_str()),
                (42, true, "abcd")
            );
            let session = String::from_utf8(cached(dir, "startsession-515").unwrap()).unwrap();
            assert!(
                session.contains("\"HardcoreUnlocks\":[{\"ID\":42,"),
                "{session}"
            );
            assert!(session.contains("\"Unlocks\":[{\"ID\":42,"), "{session}");
            assert!(session.contains("},{\"ID\":7,"), "{session}");
        });
    }

    #[test]
    fn kept_unlocks_leave_once_sent_and_stay_while_there_is_no_network() {
        with_store(|_| {
            for a in [1, 2, 3] {
                answer_offline(&format!("r=awardachievement&u=Ash&t=tok&a={a}&h=0"), (0, 0));
            }
            let award = |p: &Pending, _ago: u32| Some(format!("r=awardachievement&a={}", p.id));
            // the second request finds no network: it and the third stay
            let calls = Mutex::new(0);
            let fetch = |_: &str, post: &str, _: &str| {
                *lock(&calls) += 1;
                match post.ends_with("a=2") {
                    true => (-2, Vec::new()),
                    false => (200, b"{\"Success\":true}".to_vec()),
                }
            };
            assert_eq!(send_now(&award, &fetch), 1);
            assert_eq!(*lock(&calls), 2, "it kept trying with no network");
            let left: Vec<u32> = pending().iter().map(|p| p.id).collect();
            assert_eq!(left, [2, 3]);
            let fetch = |_: &str, _: &str, _: &str| (200, b"{\"Success\":true}".to_vec());
            assert_eq!(send_now(&award, &fetch), 2);
            assert!(pending().is_empty());
        });
    }

    #[test]
    fn with_no_store_nothing_is_kept() {
        let _held = lock(&TEST_STORE);
        assert_eq!(
            answer_offline("r=awardachievement&u=a&t=b&a=1", (0, 0)),
            None
        );
        assert_eq!(
            answer_offline("r=ping&u=a&t=b", (0, 0)).map(|a| a.0),
            Some(200)
        );
    }
}
