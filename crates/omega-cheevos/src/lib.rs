//! RetroAchievements, through rcheevos, the site's own client library (MIT, in rcheevos/): one
//! client for the handheld, signed in with the player's account, that identifies the game put in
//! by hashing its ROM, reads its memory every frame and says when an achievement is earned.
//!
//! rcheevos asks for the network through this crate: each request goes out on a thread of its
//! own through `curl`, as label fetches do, and its answer is handed back to the client the next
//! time it is pumped, on whichever thread pumps it. Every call into the client is made under one
//! lock, so the main thread (signing in, between games) and the emulator's (a game's frames)
//! never meet inside it.
//!
//! With no network, the answers kept on the card stand in for the site's, and unlocks wait on
//! the card to be sent: see offline.rs.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use omega_retro::{CoreMemory, MemoryDescriptor};

mod offline;
pub use offline::{pending, set_store, Pending};

/// The consoles omega-zero plays, by rcheevos' numbers.
pub const CONSOLE_SNES: u32 = 3;
pub const CONSOLE_GB: u32 = 4;
pub const CONSOLE_GBA: u32 = 5;
pub const CONSOLE_GBC: u32 = 6;
pub const CONSOLE_PSP: u32 = 41;

/// How long one request may take before it is given up as a network error the client retries.
const REQUEST_SECS: &str = "30";

/// rcheevos' answer for a request that could not reach the server and may be tried again.
const RETRYABLE: c_int = -2;

extern "C" {
    fn oc_start(hardcore: c_int) -> c_int;
    fn oc_hardcore() -> c_int;
    fn oc_hash(console: u32, path: *const c_char, out: *mut c_char) -> c_int;
    fn oc_user_agent(buffer: *mut c_char, size: usize) -> usize;
    fn oc_login_password(user: *const c_char, password: *const c_char);
    fn oc_login_token(user: *const c_char, token: *const c_char);
    fn oc_login_state(error: *mut *const c_char) -> c_int;
    fn oc_login_unreachable() -> c_int;
    fn oc_game_unreachable() -> c_int;
    fn oc_has_memory() -> c_int;
    fn oc_user_scores(hardcore: *mut u32, softcore: *mut u32);
    #[allow(clippy::too_many_arguments)]
    fn oc_award_post(
        id: u32,
        user: *const c_char,
        token: *const c_char,
        hardcore: c_int,
        hash: *const c_char,
        seconds: u32,
        out: *mut c_char,
        size: usize,
    ) -> usize;
    fn oc_user_name() -> *const c_char;
    fn oc_user_token() -> *const c_char;
    fn oc_logout();
    #[allow(clippy::too_many_arguments)]
    fn oc_set_memory(
        descriptors: *const MemoryDescriptor,
        count: c_uint,
        console: u32,
        system: *mut u8,
        system_size: usize,
        save: *mut u8,
        save_size: usize,
        video: *mut u8,
        video_size: usize,
    );
    fn oc_load_game(console: u32, path: *const c_char);
    fn oc_load_game_hash(hash: *const c_char);
    fn oc_game_hashing();
    fn oc_game_state(error: *mut *const c_char) -> c_int;
    fn oc_unload_game();
    fn oc_summary(
        title: *mut *const c_char,
        id: *mut u32,
        unlocked: *mut u32,
        total: *mut u32,
        points_unlocked: *mut u32,
        points_total: *mut u32,
    ) -> c_int;
    fn oc_achievements(each: AchievementFn, user: *mut c_void);
    fn oc_frame();
    fn oc_idle();
    fn oc_reset();
    fn oc_answer(id: u32, status: c_int, body: *const c_char, length: usize);
}

type AchievementFn = extern "C" fn(
    user: *mut c_void,
    title: *const c_char,
    description: *const c_char,
    points: u32,
    unlocked: c_int,
    progress: *const c_char,
);

/// The client is made, and every oc_ call may be made.
static STARTED: AtomicBool = AtomicBool::new(false);
/// No request leaves the handheld: each is answered as a network error. For tests, which must
/// never send an account anywhere.
static OFFLINE: AtomicBool = AtomicBool::new(false);
/// Held across every call into the client.
static CLIENT: Mutex<()> = Mutex::new(());
/// Answers come back from the network, waiting to be handed to the client.
static ANSWERS: Mutex<Vec<(u32, c_int, Vec<u8>)>> = Mutex::new(Vec::new());
static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
/// The game hashed last, its console and the hash: held while a game is hashed, outside the
/// client's lock, so the game's achievements load again without hashing it again.
/// Counts the games begun and put away, so a hash made for one put away since loads nothing.
static GAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[allow(clippy::type_complexity)]
static HASHED: Mutex<Option<(u32, std::path::PathBuf, Option<String>)>> = Mutex::new(None);
/// The site's name and the address it was last reached at.
static RESOLVED: Mutex<Option<(String, String)>> = Mutex::new(None);
/// What every request says it is, as RetroAchievements asks: omega-zero and rcheevos' versions.
static AGENT: Mutex<String> = Mutex::new(String::new());

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Writes `what` to the log, with the time since omega-zero started, so the steps of a game's
/// achievements loading can be timed against one another.
pub fn trace(what: &str) {
    static BEGAN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let ms = BEGAN
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis();
    eprintln!("omega-zero: cheevos: {ms} ms: {what}");
}

/// What rcheevos says, to the log.
#[no_mangle]
pub extern "C" fn omega_cheevos_log(message: *const c_char) {
    trace(&text(message));
}

/// Runs `f` on the client, once it is made and only if no other thread is in it: for what the
/// screen and the game look in on every frame, which skip a frame rather than wait out an
/// answer being taken in, as a game's achievements take up to a second to.
fn try_client<R>(f: impl FnOnce() -> R) -> Option<R> {
    let _held = match CLIENT.try_lock() {
        Ok(held) => held,
        Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return None,
    };
    STARTED.load(Ordering::Acquire).then(f)
}

/// An answer has come back: the thread that hands answers to the client is woken, and made if
/// it is not yet.
fn answer(id: u32, status: c_int, body: Vec<u8>) {
    lock(&ANSWERS).push((id, status, body));
    // offline, as tests are, answers wait for the next pump, so a test steps through them
    if OFFLINE.load(Ordering::Acquire) {
        return;
    }
    static WAKE: std::sync::Condvar = std::sync::Condvar::new();
    static MADE: std::sync::Once = std::sync::Once::new();
    MADE.call_once(|| {
        let spawned = std::thread::Builder::new()
            .name("omega-cheevos-answers".into())
            .spawn(|| loop {
                {
                    let mut answers = lock(&ANSWERS);
                    while answers.is_empty() {
                        answers = WAKE.wait(answers).unwrap_or_else(|e| e.into_inner());
                    }
                }
                with_client(deliver);
            });
        if let Err(e) = spawned {
            eprintln!("omega-cheevos: no thread to take answers in: {e}");
        }
    });
    WAKE.notify_one();
}

/// Runs `f` on the client, once it is made, under its lock.
fn with_client<R>(f: impl FnOnce() -> R) -> Option<R> {
    let asked = std::time::Instant::now();
    let held = lock(&CLIENT);
    let got = std::time::Instant::now();
    let out = STARTED.load(Ordering::Acquire).then(f);
    drop(held);
    // a call that held the client, or waited for it, long enough to hitch a frame: which, and
    // for how long, as every thread that calls into the client waits out the one in it
    let (waited, ran) = (got - asked, got.elapsed());
    if waited.as_millis() >= 50 || ran.as_millis() >= 50 {
        eprintln!(
            "omega-cheevos: {} waited {} ms for the client and held it {} ms",
            std::thread::current().name().unwrap_or("a thread"),
            waited.as_millis(),
            ran.as_millis()
        );
    }
    out
}

fn text(p: *const c_char) -> String {
    match p.is_null() {
        true => String::new(),
        // SAFETY: rcheevos hands NUL-terminated strings that live through the call
        false => unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned(),
    }
}

fn c(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

/// What happened in a game, for the player to be told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    Unlocked,
    LeaderboardStarted,
    LeaderboardFailed,
    LeaderboardSubmitted,
    /// Every achievement of the game is earned.
    Mastered,
    ServerError,
    /// An unlock could not reach the server; it is kept and sent again.
    Disconnected,
    Reconnected,
    /// Unlocks earned offline were sent, as many as `points` says.
    Synced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    pub title: String,
    pub description: String,
    pub points: u32,
}

/// Where signing in stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Login {
    Out,
    Pending,
    /// Signed in, with the name the site shows and the token to sign in with from now on.
    In {
        name: String,
        token: String,
    },
    Refused(String),
    /// The site could not be reached: no network yet. Worth trying again.
    Unreachable(String),
}

/// Where loading the game in the handheld's achievements stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameLoad {
    None,
    Pending,
    Loaded,
    /// No achievements for it, or the server would not say.
    Refused(String),
}

/// The game's achievements, and how many of them the player has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// The game's name on RetroAchievements, and its number there: what the ROM was taken for.
    pub title: String,
    pub id: u32,
    pub unlocked: u32,
    pub total: u32,
    pub points_unlocked: u32,
    pub points_total: u32,
}

/// One of the game's achievements, as the player stands with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Achievement {
    pub title: String,
    pub description: String,
    pub points: u32,
    pub unlocked: bool,
    /// How far along a measured one is, as "3/10"; empty for the rest.
    pub progress: String,
}

/// Makes the client, in hardcore or not; `version` is omega-zero's, for the requests' agent.
/// Calling it again only switches hardcore.
pub fn start(hardcore: bool, version: &str) -> bool {
    let _held = lock(&CLIENT);
    // SAFETY: under the lock, as every call into the client
    let made = unsafe { oc_start(c_int::from(hardcore)) } != 0;
    if made {
        let mut clause = vec![0u8; 128];
        // SAFETY: the buffer is as long as said
        let n = unsafe { oc_user_agent(clause.as_mut_ptr() as *mut c_char, clause.len()) };
        clause.truncate(n.min(clause.len()));
        *lock(&AGENT) = format!(
            "omega-zero-matan/{version} {}",
            String::from_utf8_lossy(&clause).trim_end_matches('\0')
        );
        STARTED.store(true, Ordering::Release);
    }
    made
}

/// Keeps every request on the handheld from now on, answered as a network error the client
/// retries: for tests, so none ever reaches RetroAchievements.
pub fn go_offline() {
    OFFLINE.store(true, Ordering::Release);
}

pub fn login_with_password(user: &str, password: &str) {
    let (user, password) = (c(user), c(password));
    // SAFETY: the strings outlive the call, which copies what it keeps
    with_client(|| unsafe { oc_login_password(user.as_ptr(), password.as_ptr()) });
}

pub fn login_with_token(user: &str, token: &str) {
    let (user, token) = (c(user), c(token));
    // SAFETY: as above
    with_client(|| unsafe { oc_login_token(user.as_ptr(), token.as_ptr()) });
}

pub fn logout() {
    // SAFETY: under the client's lock
    with_client(|| unsafe { oc_logout() });
}

pub fn login() -> Login {
    let Some(login) = try_client(|| {
        let mut error = std::ptr::null();
        // SAFETY: under the lock; the error lives in the shim
        match unsafe { oc_login_state(&mut error) } {
            1 => Login::Pending,
            2 => Login::In {
                name: text(unsafe { oc_user_name() }),
                token: text(unsafe { oc_user_token() }),
            },
            3 if unsafe { oc_login_unreachable() } != 0 => Login::Unreachable(text(error)),
            3 => Login::Refused(text(error)),
            _ => Login::Out,
        }
    }) else {
        // another thread is in the client: as the client is made, or busy
        return match STARTED.load(Ordering::Acquire) {
            true => Login::Pending,
            false => Login::Out,
        };
    };
    login
}

/// Starts the achievements of the game `rom` of `console` the core has just loaded, read from
/// `memory`. Called on the thread that runs the core, which `frame` must be called on too.
pub fn begin_game(console: u32, rom: &Path, memory: &CoreMemory) {
    trace("the game begins");
    let game = GAME.fetch_add(1, Ordering::AcqRel) + 1;
    let begun = with_client(|| unsafe {
        // SAFETY: the shim keeps the region pointers, the core's, for as long as the game is
        // loaded; `end_game` drops them before the core goes
        oc_set_memory(
            memory.map.as_ptr(),
            memory.map.len() as c_uint,
            console,
            memory.system.0,
            memory.system.1,
            memory.save.0,
            memory.save.1,
            memory.video.0,
            memory.video.1,
        );
        oc_game_hashing();
    });
    if begun.is_some() {
        load_in_turn(game, console, rom);
    }
}

/// The calling thread at the lowest priority: hashing reads the game's file while the core,
/// starting, reads it too, and the cart going in is drawn meanwhile, which come first.
fn stand_back() {
    #[cfg(target_os = "linux")]
    {
        extern "C" {
            fn setpriority(which: c_int, who: c_uint, prio: c_int) -> c_int;
        }
        // SAFETY: setpriority only sets the calling thread's nice value; a refusal leaves the
        // thread as it was
        unsafe {
            setpriority(0, 0, 19);
        }
    }
}

/// Hashes the game `rom` of `console` on a thread of its own, and then starts loading its
/// achievements, unless the game has been put away meanwhile. Hashing reads all of a
/// cartridge's ROM, or much of a PSP game's disc, off the card, which takes up to a second:
/// neither the game, which starts meanwhile, nor anything else that calls into the client
/// waits for it.
fn load_in_turn(game: u64, console: u32, rom: &Path) {
    let rom = rom.to_path_buf();
    let spawned = std::thread::Builder::new()
        .name("omega-cheevos-hash".into())
        .spawn(move || {
            stand_back();
            let hash = hash_once(console, &rom);
            with_client(|| {
                // SAFETY: under the lock
                if GAME.load(Ordering::Acquire) == game && unsafe { oc_has_memory() } != 0 {
                    load_game(console, &rom, hash.as_deref());
                }
            });
        });
    if let Err(e) = spawned {
        eprintln!("omega-cheevos: could not hash the game: {e}");
    }
}

/// The hash of the game `rom` of `console`, made outside the client's lock, as hashing a PSP
/// game reads much of its disc: once for each game, kept for its achievements to load again.
fn hash_once(console: u32, rom: &Path) -> Option<String> {
    let mut hashed = lock(&HASHED);
    if let Some((c, r, hash)) = hashed.as_ref() {
        if *c == console && r == rom {
            return hash.clone();
        }
    }
    let began = std::time::Instant::now();
    let hash = game_hash(console, rom);
    trace(&format!(
        "hashed in {} ms{}",
        began.elapsed().as_millis(),
        if hash.is_some() { "" } else { ", no hash" }
    ));
    *hashed = Some((console, rom.to_path_buf(), hash.clone()));
    hash
}

/// Starts loading the game's achievements, by its hash where it could be made, or by the
/// client's own look at `rom`, which says why the game is not known. Called under the lock.
fn load_game(console: u32, rom: &Path, hash: Option<&str>) {
    trace(if hash.is_some() {
        "loading by hash"
    } else {
        "loading by identifying"
    });
    // SAFETY: under the lock; the shim copies the hash and the path
    unsafe {
        match hash {
            Some(hash) => oc_load_game_hash(c(hash).as_ptr()),
            None => oc_load_game(console, c(&rom.to_string_lossy()).as_ptr()),
        }
    }
}

pub fn game_load() -> GameLoad {
    let Some(load) = try_client(|| {
        let mut error = std::ptr::null();
        // SAFETY: under the lock
        match unsafe { oc_game_state(&mut error) } {
            1 => GameLoad::Pending,
            2 => GameLoad::Loaded,
            3 => GameLoad::Refused(text(error)),
            _ => GameLoad::None,
        }
    }) else {
        return match STARTED.load(Ordering::Acquire) {
            true => GameLoad::Pending,
            false => GameLoad::None,
        };
    };
    load
}

/// Loading the game's achievements failed for want of a network: worth trying again.
pub fn game_unreachable() -> bool {
    // SAFETY: under the lock
    try_client(|| unsafe { oc_game_unreachable() } != 0).unwrap_or(false)
}

/// The hash RetroAchievements knows the game `rom` of `console` by, where it can be made: a
/// cartridge's of its ROM, a PSP game's of the files on its disc.
pub fn game_hash(console: u32, rom: &Path) -> Option<String> {
    let path = c(&rom.to_string_lossy());
    let mut out = [0 as c_char; 33];
    // SAFETY: the path lives through the call, and `out` holds the 32 digits and their end
    let made = unsafe { oc_hash(console, path.as_ptr(), out.as_mut_ptr()) };
    // SAFETY: rcheevos ends the digits it wrote
    (made != 0).then(|| text(out.as_ptr()))
}

/// Loads the achievements of the game `rom` of `console` again, its memory as `begin_game`
/// set it: after a sign-in or a network that came too late for the game's start.
pub fn reload_game(console: u32, rom: &Path) {
    trace("loading again");
    let game = GAME.load(Ordering::Acquire);
    let has_memory = with_client(|| {
        // SAFETY: under the lock
        let has = unsafe { oc_has_memory() } != 0;
        if has {
            // SAFETY: under the lock
            unsafe { oc_game_hashing() };
        }
        has
    });
    if has_memory == Some(true) {
        load_in_turn(game, console, rom);
    }
}

/// The game's achievements, once they are loaded.
pub fn summary() -> Option<Summary> {
    try_client(|| {
        let mut title = std::ptr::null();
        let (mut id, mut unlocked, mut total, mut points_unlocked, mut points_total) =
            (0, 0, 0, 0, 0);
        // SAFETY: under the lock; the title lives in the client with the game
        let loaded = unsafe {
            oc_summary(
                &mut title,
                &mut id,
                &mut unlocked,
                &mut total,
                &mut points_unlocked,
                &mut points_total,
            )
        } != 0;
        loaded.then(|| Summary {
            title: text(title),
            id,
            unlocked,
            total,
            points_unlocked,
            points_total,
        })
    })
    .flatten()
}

/// The game's achievements, once they are loaded: none before, or with no game.
pub fn achievements() -> Vec<Achievement> {
    extern "C" fn each(
        user: *mut c_void,
        title: *const c_char,
        description: *const c_char,
        points: u32,
        unlocked: c_int,
        progress: *const c_char,
    ) {
        // SAFETY: `user` is the Vec handed to oc_achievements below, alive through its call
        let list = unsafe { &mut *(user as *mut Vec<Achievement>) };
        list.push(Achievement {
            title: text(title),
            description: text(description),
            points,
            unlocked: unlocked != 0,
            progress: text(progress),
        });
    }
    let mut list: Vec<Achievement> = Vec::new();
    // SAFETY: under the lock; `each` only pushes onto `list`, which outlives the call
    with_client(|| unsafe { oc_achievements(each, &mut list as *mut _ as *mut c_void) });
    list
}

/// The game is played in hardcore: no states loaded, no rewinding.
pub fn hardcore_active() -> bool {
    // SAFETY: under the lock
    with_client(|| unsafe { oc_hardcore() } != 0).unwrap_or(false)
}

/// One frame of the game has run: the client checks the game's memory for achievements. A
/// frame the client is busy for, taking in an answer, goes unchecked rather than waiting.
pub fn frame() {
    // SAFETY: under the lock, on the thread that runs the core
    try_client(|| unsafe { oc_frame() });
}

/// Lets the client do what it waits to do, without reading the game's memory: between games,
/// or with the game paused. Answers come back are taken in on a thread of their own, but for
/// tests, offline, whose answers a pump takes in; a pump the client is busy for is skipped.
pub fn pump() {
    try_client(|| {
        if OFFLINE.load(Ordering::Acquire) {
            deliver();
        }
        // SAFETY: under the lock
        unsafe { oc_idle() };
    });
}

/// The game's memory jumped, as a state loads or the game rewinds: achievements start watching
/// again from here, so none is earned by the jump.
pub fn reset() {
    // SAFETY: under the lock
    with_client(|| unsafe { oc_reset() });
}

/// The game is put away: the client lets go of it and of its memory, before the core goes.
pub fn end_game() {
    GAME.fetch_add(1, Ordering::AcqRel);
    // SAFETY: under the lock
    with_client(|| unsafe { oc_unload_game() });
}

pub fn take_events() -> Vec<Event> {
    std::mem::take(&mut *lock(&EVENTS))
}

/// Hands the answers come back to the client. Called under its lock.
fn deliver() {
    let answers = std::mem::take(&mut *lock(&ANSWERS));
    for (id, status, mut body) in answers {
        let length = body.len();
        body.push(0);
        let began = std::time::Instant::now();
        // SAFETY: under the lock; the body is NUL-terminated and lives through the call
        unsafe { oc_answer(id, status, body.as_ptr() as *const c_char, length) };
        trace(&format!(
            "answer {id} ({status}, {length} bytes) taken in, {} ms",
            began.elapsed().as_millis()
        ));
    }
}

/// The client asks for the network: the request goes out on a thread of its own, and its
/// answer is taken in on another. Called by the shim, under the client's lock.
#[no_mangle]
pub extern "C" fn omega_cheevos_server_call(
    id: u32,
    url: *const c_char,
    post_data: *const c_char,
    content_type: *const c_char,
) {
    let url = text(url);
    let post = (!post_data.is_null()).then(|| text(post_data));
    // which request: its r=, never its account fields
    let kind = post
        .as_deref()
        .and_then(|p| p.split('&').find_map(|f| f.strip_prefix("r=")))
        .unwrap_or("?")
        .to_string();
    trace(&format!("request {id}: {kind}"));
    let content_type = text(content_type);
    let agent = lock(&AGENT).clone();
    if let Some(post) = &post {
        offline::requested(post);
    }
    let (mut hardcore, mut softcore) = (0, 0);
    // SAFETY: called under the client's lock, from inside a call into it
    unsafe { oc_user_scores(&mut hardcore, &mut softcore) };
    let scores = (hardcore, softcore);
    if OFFLINE.load(Ordering::Acquire) {
        answer(id, RETRYABLE, Vec::new());
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("omega-cheevos-net".into())
        .spawn(move || {
            let (status, body) = fetch(&url, post.as_deref(), &content_type, &agent);
            trace(&format!("answer {id} came back ({status})"));
            let answer = match (status > 0, post.as_deref()) {
                (true, post) => {
                    if let Some(post) = post {
                        offline::answered(post, status, &body);
                    }
                    (status, body)
                }
                // no network: an answer kept on the card, or the unlock kept for later
                (false, Some(post)) => match offline::answer_offline(post, scores) {
                    Some(kept) => {
                        if post.contains("r=awardachievement") {
                            event(EventKind::Disconnected, "", "", 0);
                        }
                        kept
                    }
                    None => (status, body),
                },
                (false, None) => (status, body),
            };
            self::answer(id, answer.0, answer.1);
            if status == 200 {
                sync(&agent);
            }
        });
    if spawned.is_err() {
        answer(id, RETRYABLE, Vec::new());
    }
}

/// Sends the unlocks earned offline, now the site answers.
fn sync(agent: &str) {
    let agent = agent.to_string();
    offline::send_pending(
        |p, seconds| {
            let (user, token, hash) = (c(&p.user), c(&p.token), c(&p.hash));
            let mut out = vec![0u8; 1024];
            // SAFETY: pure request building, no client; the buffer is as long as said
            let n = unsafe {
                oc_award_post(
                    p.id,
                    user.as_ptr(),
                    token.as_ptr(),
                    c_int::from(p.hardcore),
                    hash.as_ptr(),
                    seconds,
                    out.as_mut_ptr() as *mut c_char,
                    out.len(),
                )
            };
            (n > 0).then(|| String::from_utf8_lossy(&out[..n]).into_owned())
        },
        move |url, post, content_type| fetch(url, Some(post), content_type, &agent),
        |sent| event(EventKind::Synced, "", "", sent as u32),
    );
}

fn event(kind: EventKind, title: &str, description: &str, points: u32) {
    lock(&EVENTS).push(Event {
        kind,
        title: title.to_string(),
        description: description.to_string(),
        points,
    });
}

#[no_mangle]
pub extern "C" fn omega_cheevos_event(
    kind: u32,
    title: *const c_char,
    description: *const c_char,
    points: u32,
) {
    let kind = match kind {
        1 => EventKind::Unlocked,
        2 => EventKind::LeaderboardStarted,
        3 => EventKind::LeaderboardFailed,
        4 => EventKind::LeaderboardSubmitted,
        15 => EventKind::Mastered,
        16 => EventKind::ServerError,
        17 => EventKind::Disconnected,
        18 => EventKind::Reconnected,
        _ => return,
    };
    event(kind, &text(title), &text(description), points);
}

/// rcheevos opens a compressed PSP image to hash its game's files: the image, open, or null.
#[no_mangle]
pub extern "C" fn omega_cheevos_cso_open(path: *const c_char) -> *mut c_void {
    match omega_store::psp::Cso::open(Path::new(&text(path))) {
        Some(cso) => Box::into_raw(Box::new(cso)).cast(),
        None => std::ptr::null_mut(),
    }
}

/// `bytes` of the image from its sector `sector` on, into `buffer`: how many there were.
///
/// # Safety
///
/// `cso` is one `omega_cheevos_cso_open` gave and has not closed, and `buffer` holds `bytes`.
#[no_mangle]
pub unsafe extern "C" fn omega_cheevos_cso_read(
    cso: *mut c_void,
    sector: u32,
    buffer: *mut c_void,
    bytes: usize,
) -> usize {
    if cso.is_null() || buffer.is_null() {
        return 0;
    }
    // SAFETY: as the caller promises
    let (cso, out) = unsafe {
        (
            &mut *cso.cast::<omega_store::psp::Cso>(),
            std::slice::from_raw_parts_mut(buffer.cast::<u8>(), bytes),
        )
    };
    let mut done = 0;
    for (i, chunk) in out.chunks_mut(omega_store::psp::SECTOR).enumerate() {
        let Some(data) = cso.sector(sector + i as u32) else {
            break;
        };
        chunk.copy_from_slice(&data[..chunk.len()]);
        done += chunk.len();
    }
    done
}

/// The image put away.
///
/// # Safety
///
/// `cso` is one `omega_cheevos_cso_open` gave, not closed before.
#[no_mangle]
pub unsafe extern "C" fn omega_cheevos_cso_close(cso: *mut c_void) {
    if !cso.is_null() {
        // SAFETY: as the caller promises
        drop(unsafe { Box::from_raw(cso.cast::<omega_store::psp::Cso>()) });
    }
}

/// One request through curl: the post data goes in on its standard input, so a password never
/// stands on a command line, and the HTTP status comes after the body on a line of its own.
fn fetch(url: &str, post: Option<&str>, content_type: &str, agent: &str) -> (c_int, Vec<u8>) {
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "--max-time", REQUEST_SECS, "-A", agent])
        .args(["-w", "\n%{remote_ip} %{http_code}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // the address the site was last reached at, so a request does not wait on a name lookup,
    // which on the handheld's Wi-Fi now and then goes unanswered for the resolver's five seconds
    let host = host_of(url);
    let known = host.as_ref().and_then(|(name, port)| {
        lock(&RESOLVED)
            .as_ref()
            .filter(|(n, _)| n == name)
            .map(|(_, ip)| format!("{name}:{port}:{ip}"))
    });
    if let Some(resolve) = &known {
        cmd.args(["--resolve", resolve]);
    }
    if post.is_some() {
        cmd.args(["--data-binary", "@-"]);
        if !content_type.is_empty() {
            cmd.args(["-H", &format!("Content-Type: {content_type}")]);
        }
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    cmd.arg(url);
    let Ok(mut child) = cmd.spawn() else {
        return (RETRYABLE, Vec::new());
    };
    if let (Some(post), Some(mut stdin)) = (post, child.stdin.take()) {
        use std::io::Write;
        let _ = stdin.write_all(post.as_bytes());
    }
    let Ok(out) = child.wait_with_output() else {
        return (RETRYABLE, Vec::new());
    };
    let (status, body) = answer_of(&out.stdout);
    match (status > 0, host) {
        (true, Some((name, _))) => {
            if let Some(ip) = remote_ip_of(&out.stdout) {
                *lock(&RESOLVED) = Some((name, ip));
            }
        }
        (false, _) => {
            // an address gone stale is looked up afresh next time
            *lock(&RESOLVED) = None;
            trace(&format!(
                "curl: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        _ => {}
    }
    (status, body)
}

/// The site's name and port in `url`.
fn host_of(url: &str) -> Option<(String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let host = rest.split(['/', '?']).next()?;
    let (name, port) = match host.rsplit_once(':') {
        Some((name, port)) => (name, port.parse().ok()?),
        None => (host, if scheme == "http" { 80 } else { 443 }),
    };
    (!name.is_empty()).then(|| (name.to_string(), port))
}

/// The address curl reached, from the last line it printed: `<ip> <status>`.
fn remote_ip_of(out: &[u8]) -> Option<String> {
    let last = out.rsplit(|b| *b == b'\n').next()?;
    let text = std::str::from_utf8(last).ok()?;
    let mut words = text.split_whitespace();
    let (ip, _status) = (words.next()?, words.next()?);
    ip.parse::<std::net::IpAddr>().ok().map(|ip| ip.to_string())
}

/// curl's output split into the body and the status on its last line; a status curl could not
/// give, as when the network is down, is one the client retries.
pub fn answer_of(out: &[u8]) -> (c_int, Vec<u8>) {
    let at = out.iter().rposition(|b| *b == b'\n');
    let (body, code) = match at {
        Some(i) => (&out[..i], &out[i + 1..]),
        None => (&out[..0], out),
    };
    // the last line is the status, after the address it came from where curl gave one
    let status = std::str::from_utf8(code)
        .ok()
        .and_then(|s| s.split_whitespace().last()?.parse::<c_int>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(RETRYABLE);
    (status, body.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_is_the_line_after_the_body() {
        assert_eq!(
            answer_of(b"{\"Success\":true}\n200"),
            (200, b"{\"Success\":true}".to_vec())
        );
        assert_eq!(
            answer_of(b"line one\nline two\n404"),
            (404, b"line one\nline two".to_vec())
        );
    }

    #[test]
    fn the_address_comes_before_the_status_and_the_site_is_named_by_its_url() {
        let out = b"{\"Success\":true}\n104.21.1.1 200";
        assert_eq!(answer_of(out), (200, b"{\"Success\":true}".to_vec()));
        assert_eq!(remote_ip_of(out).as_deref(), Some("104.21.1.1"));
        assert_eq!(remote_ip_of(b"body\n 000"), None);
        assert_eq!(
            host_of("https://retroachievements.org/dorequest.php"),
            Some(("retroachievements.org".into(), 443))
        );
        assert_eq!(
            host_of("http://127.0.0.1:8080/x?y"),
            Some(("127.0.0.1".into(), 8080))
        );
    }

    #[test]
    fn no_status_is_a_network_error_the_client_retries() {
        assert_eq!(answer_of(b"").0, RETRYABLE);
        assert_eq!(answer_of(b"\n000").0, RETRYABLE);
    }

    /// The tests that call into the client, one at a time, as it is the one client for all.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    #[test]
    fn nothing_happens_before_the_client_is_made() {
        let _turn = lock(&ONE_AT_A_TIME);
        assert_eq!(login(), Login::Out);
        assert_eq!(game_load(), GameLoad::None);
        assert!(!hardcore_active());
        frame();
        pump();
    }

    /// A directory record for `name` at sector `lba`, `len` long.
    fn record(name: &str, lba: u32, len: u32, folder: bool) -> Vec<u8> {
        let name = name.as_bytes();
        let size = 33 + name.len() + (1 - name.len() % 2);
        let mut r = vec![0u8; size];
        r[0] = size as u8;
        r[2..6].copy_from_slice(&lba.to_le_bytes());
        r[6..10].copy_from_slice(&lba.to_be_bytes());
        r[10..14].copy_from_slice(&len.to_le_bytes());
        r[14..18].copy_from_slice(&len.to_be_bytes());
        r[25] = if folder { 2 } else { 0 };
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    const S: usize = omega_store::psp::SECTOR;

    /// A UMD image: the descriptor at 16, the root at 18, PSP_GAME at 19, SYSDIR at 20, its
    /// PARAM.SFO at 21 and EBOOT.BIN at 22.
    fn umd(eboot: &[u8]) -> Vec<u8> {
        let sectors = 24u32;
        let mut img = vec![0u8; sectors as usize * S];
        let pvd = 16 * S;
        img[pvd] = 1;
        img[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        img[pvd + 80..pvd + 84].copy_from_slice(&sectors.to_le_bytes());
        img[pvd + 156..pvd + 190].copy_from_slice(&record("\0", 18, S as u32, true)[..34]);
        let mut put = |at: usize, bytes: &[u8]| img[at..at + bytes.len()].copy_from_slice(bytes);
        put(18 * S, &record("PSP_GAME", 19, S as u32, true));
        let sfo = omega_store::psp::param_sfo(&[("DISC_ID", "ULUS10391"), ("TITLE", "Test")]);
        let sfo_record = record("PARAM.SFO;1", 21, sfo.len() as u32, false);
        put(19 * S, &sfo_record);
        put(
            19 * S + sfo_record.len(),
            &record("SYSDIR", 20, S as u32, true),
        );
        put(
            20 * S,
            &record("EBOOT.BIN;1", 22, eboot.len() as u32, false),
        );
        put(21 * S, &sfo);
        put(22 * S, eboot);
        img
    }

    /// `img` as a CISO, every block kept as it is.
    fn cso(img: &[u8]) -> Vec<u8> {
        let blocks: Vec<&[u8]> = img.chunks(S).collect();
        let mut out = b"CISO".to_vec();
        out.extend_from_slice(&24u32.to_le_bytes());
        out.extend_from_slice(&(img.len() as u64).to_le_bytes());
        out.extend_from_slice(&(S as u32).to_le_bytes());
        out.extend_from_slice(&[1, 0, 0, 0]);
        let mut at = (24 + (blocks.len() + 1) * 4) as u32;
        for b in &blocks {
            out.extend_from_slice(&(at | 0x8000_0000).to_le_bytes());
            at += b.len() as u32;
        }
        out.extend_from_slice(&at.to_le_bytes());
        for b in blocks {
            out.extend_from_slice(b);
        }
        out
    }

    /// A PSP game is known by its disc's PARAM.SFO and EBOOT.BIN, whether its image is an ISO
    /// or the same disc compressed, and two games apart by their files.
    #[test]
    fn a_psp_game_is_hashed_from_its_disc_iso_or_cso_alike() {
        let d = tempfile::tempdir().unwrap();
        let iso = d.path().join("Game.iso");
        std::fs::write(&iso, umd(b"one game's code")).unwrap();
        let hash = game_hash(CONSOLE_PSP, &iso).expect("the ISO hashed");
        assert_eq!(hash.len(), 32, "{hash}");
        let packed = d.path().join("Game.cso");
        std::fs::write(&packed, cso(&umd(b"one game's code"))).unwrap();
        assert_eq!(game_hash(CONSOLE_PSP, &packed), Some(hash.clone()));
        let other = d.path().join("Other.iso");
        std::fs::write(&other, umd(b"another game's")).unwrap();
        assert_ne!(game_hash(CONSOLE_PSP, &other), Some(hash));
        let junk = d.path().join("Junk.cso");
        std::fs::write(&junk, b"not a disc").unwrap();
        assert_eq!(game_hash(CONSOLE_PSP, &junk), None);
    }

    #[test]
    fn the_client_is_made_and_says_what_it_is() {
        let _turn = lock(&ONE_AT_A_TIME);
        assert!(start(false, "9.9.9"));
        let agent = lock(&AGENT).clone();
        assert!(
            agent.starts_with("omega-zero-matan/9.9.9 rcheevos/"),
            "{agent}"
        );
        assert_eq!(login(), Login::Out);
        assert_eq!(summary(), None);
    }

    #[test]
    fn a_look_in_on_a_busy_client_does_not_wait_for_it() {
        let _turn = lock(&ONE_AT_A_TIME);
        assert!(start(false, "9.9.9"));
        let held = lock(&CLIENT);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let began = std::time::Instant::now();
            let seen = (login(), game_load(), summary(), game_unreachable());
            frame();
            pump();
            let _ = tx.send((began.elapsed(), seen));
        });
        let answer = rx.recv_timeout(std::time::Duration::from_secs(2));
        drop(held);
        let (took, seen) = answer.expect("a look in on the client waited for it");
        assert!(took < std::time::Duration::from_millis(500), "{took:?}");
        assert_eq!(seen, (Login::Pending, GameLoad::Pending, None, false));
    }

    #[test]
    fn a_game_is_hashed_once_however_often_its_achievements_load() {
        let d = tempfile::tempdir().unwrap();
        let rom = d.path().join("Game.gba");
        std::fs::write(&rom, [1u8; 4096]).unwrap();
        let first = hash_once(CONSOLE_GBA, &rom).expect("the ROM hashed");
        // the ROM changed under it: the hash kept stands, as nothing was hashed again
        std::fs::write(&rom, [2u8; 4096]).unwrap();
        assert_eq!(
            hash_once(CONSOLE_GBA, &rom).as_deref(),
            Some(first.as_str())
        );
        // another game is hashed afresh
        let other = d.path().join("Other.gba");
        std::fs::write(&other, [2u8; 4096]).unwrap();
        assert_ne!(hash_once(CONSOLE_GBA, &other), Some(first));
    }
}
