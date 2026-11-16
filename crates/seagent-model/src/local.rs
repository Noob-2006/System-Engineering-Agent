//! Managed local inference (Hermes `hermes_cli/local_runtime/supervisor.py`, OpenClaw
//! `extensions/llama-cpp`): a provider with `model_path` gets its own llama.cpp server,
//! started on first use on a free loopback port, health-checked (`/health`), restarted
//! if it died, stopped after `idle_timeout_secs` without requests, and stopped with seagent.
//!
//! "Stopped with seagent" holds however seagent ends:
//!
//! * a clean exit — `seagent run` ends `main` with `std::process::exit`, which runs no
//!   destructors, so `main` calls [`shutdown_all`] first (every live server is registered);
//! * a crash or `SIGKILL` — the server is a tied child (`seagent_core::proc::spawn_tied`):
//!   `PR_SET_PDEATHSIG` on Linux / Android, a kill-on-close Job Object on Windows. macOS has no
//!   parent-death signal, so there (and on the BSDs) the server runs under a tiny `sh` guard
//!   that holds the read end of a pipe from seagent: when seagent dies, however it dies, the
//!   kernel closes the write end and the guard `SIGTERM`s the server.
//!
//! Timeouts: `startup_timeout_secs` (default 180) is how long the first `/health` may take
//! (loading a large GGUF). `idle_timeout_secs`, when set, frees the model's memory: the server
//! is stopped once it has served nothing for that long (llama.cpp's `/slots` tells a running
//! generation from an idle one), and the next request starts it again.

use crate::auth::{Cred, CredSource};
use crate::ProviderConfig;
use seagent_core::{ProviderError, ProviderErrorKind as K};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

/// Default wait for the first `/health` of a freshly started server.
pub const DEFAULT_STARTUP_SECS: u64 = 180;

struct Server {
    child: Child,
    port: u16,
    /// The write end of the guard pipe (macOS / BSD; see the module docs). Dropping it stops
    /// the server; it is kept here so the server lives exactly as long as this entry.
    guard: Option<std::process::ChildStdin>,
}

impl Server {
    fn stop(mut self) {
        self.guard.take();
        // The child leads its own process group (Windows: its own job), so this reaches the
        // guard shell and the server alike: SIGTERM, a grace period, then SIGKILL.
        seagent_core::proc::kill_group(self.child.id() as i32, Duration::from_secs(3));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Shared {
    id: String,
    state: Mutex<Option<Server>>,
    last_use: Mutex<Instant>,
}

impl Shared {
    fn stop(&self) -> bool {
        let s = self.state.lock().unwrap_or_else(|e| e.into_inner()).take();
        let had = s.is_some();
        if let Some(s) = s {
            s.stop();
        }
        had
    }
}

/// Every managed server of this process, for [`shutdown_all`].
fn live() -> &'static Mutex<Vec<Weak<Shared>>> {
    static LIVE: OnceLock<Mutex<Vec<Weak<Shared>>>> = OnceLock::new();
    LIVE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Stop every managed local server this process started. `main` calls it before
/// `std::process::exit` (which skips destructors). Returns how many servers were running.
pub fn shutdown_all() -> usize {
    let all: Vec<Arc<Shared>> = {
        let mut l = live().lock().unwrap_or_else(|e| e.into_inner());
        l.retain(|w| w.strong_count() > 0);
        l.iter().filter_map(Weak::upgrade).collect()
    };
    all.iter().filter(|s| s.stop()).count()
}

pub struct Managed {
    id: String,
    command: String,
    args: Vec<String>,
    inner: Arc<dyn CredSource>,
    startup: Duration,
    idle: Option<Duration>,
    shared: Arc<Shared>,
}

/// Start-up wait and idle stop for `cfg` (see the module docs). Before 0.2.2 the health wait
/// read `idle_timeout_secs`; a config that set only that keeps at least that long to start.
fn timeouts(cfg: &ProviderConfig) -> (Duration, Option<Duration>) {
    let idle = cfg.idle_timeout_secs.filter(|s| *s > 0).map(Duration::from_secs);
    let startup = match cfg.startup_timeout_secs {
        Some(s) => s,
        None => {
            if let Some(i) = cfg.idle_timeout_secs {
                note_once(&cfg.id, i);
            }
            cfg.idle_timeout_secs.unwrap_or(0).max(DEFAULT_STARTUP_SECS)
        }
    };
    (Duration::from_secs(startup.max(5)), idle)
}

fn note_once(id: &str, idle: u64) {
    static SEEN: OnceLock<Mutex<std::collections::BTreeSet<String>>> = OnceLock::new();
    if SEEN.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string()) {
        eprintln!(
            "seagent: provider `{id}`: idle_timeout_secs = {idle} now stops the local server after {idle}s without requests; the start-up wait is startup_timeout_secs (default {DEFAULT_STARTUP_SECS})"
        );
    }
}

impl Managed {
    pub fn new(cfg: &ProviderConfig, inner: Arc<dyn CredSource>) -> Result<Managed, String> {
        let path = cfg.model_path.clone().unwrap_or_default();
        let p = std::path::Path::new(&path);
        if !p.exists() {
            return Err(format!(
                "provider `{}`: model_path {path} does not exist",
                cfg.id
            ));
        }
        let mut args = vec![
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            "{port}".to_string(),
        ];
        if p.is_dir() {
            // Router mode: every GGUF in the directory, loaded on demand.
            args.extend([
                "--models-dir".into(),
                path.clone(),
                "--models-autoload".into(),
            ]);
        } else {
            args.extend(["-m".into(), path.clone()]);
        }
        args.extend(cfg.args.iter().cloned());
        let (startup, idle) = timeouts(cfg);
        let shared = Arc::new(Shared {
            id: cfg.id.clone(),
            state: Mutex::new(None),
            last_use: Mutex::new(Instant::now()),
        });
        live().lock().unwrap_or_else(|e| e.into_inner()).push(Arc::downgrade(&shared));
        Ok(Managed {
            id: cfg.id.clone(),
            command: cfg.command.clone().unwrap_or_else(|| "llama-server".into()),
            args,
            inner,
            startup,
            idle,
            shared,
        })
    }

    fn agent() -> ureq::Agent {
        seagent_net::http::builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(3)))
            .proxy(None)
            .build()
            .into()
    }

    fn healthy(port: u16) -> bool {
        let url = format!("http://127.0.0.1:{port}/health");
        Self::agent().get(&url).call().is_ok_and(|r| r.status().as_u16() == 200)
    }

    /// Whether llama.cpp is generating right now (`/slots`, one entry per slot with
    /// `is_processing`). A server without the endpoint (`--no-slots`) counts as idle, so its
    /// idle time runs from the last request.
    fn busy(port: u16) -> bool {
        let url = format!("http://127.0.0.1:{port}/slots");
        let Ok(mut r) = Self::agent().get(&url).call() else { return false };
        if r.status().as_u16() != 200 {
            return false;
        }
        let Ok(v) = r.body_mut().read_to_string().map_err(|_| ()).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|_| ())) else { return false };
        v.as_array().is_some_and(|a| a.iter().any(|s| s.get("is_processing").and_then(|b| b.as_bool()) == Some(true)))
    }

    #[cfg(not(all(unix, not(any(target_os = "linux", target_os = "android")))))]
    fn command_line(&self, args: &[String]) -> std::process::Command {
        let mut c = crate::auth::program(&self.command);
        c.args(args).stdin(Stdio::null());
        c
    }

    /// macOS / BSD: the server runs under an `sh` guard that kills it when the pipe on the
    /// guard's stdin reaches EOF, i.e. when seagent is gone (see the module docs).
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    fn command_line(&self, args: &[String]) -> std::process::Command {
        const GUARD: &str = r#"exec 3<&0 0</dev/null
"$@" 3<&- &
p=$!
( read -r _ <&3; kill -TERM "$p" 2>/dev/null ) &
w=$!
exec 3<&-
wait "$p"; s=$?
kill "$w" 2>/dev/null
exit $s"#;
        let mut c = seagent_core::platform::proc::command("/bin/sh");
        c.arg("-c").arg(GUARD).arg("seagent-llama-guard").arg(&self.command).args(args).stdin(Stdio::piped());
        c
    }

    /// Port of a running, healthy server (starting one if needed).
    pub fn ensure(&self) -> Result<u16, String> {
        let mut st = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        *self.shared.last_use.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        if let Some(s) = st.as_mut() {
            if s.child.try_wait().ok().flatten().is_none() {
                return Ok(s.port);
            }
            eprintln!("seagent: {}: local server exited; restarting", self.id);
            if let Some(s) = st.take() {
                s.stop();
            }
        }
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .map_err(|e| e.to_string())?;
        let args: Vec<String> = self
            .args
            .iter()
            .map(|a| a.replace("{port}", &port.to_string()))
            .collect();
        let mut cmd = self.command_line(&args);
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = seagent_core::proc::spawn_tied(cmd, true).map_err(|e| {
            format!(
                "cannot start `{}`: {e} (install llama.cpp or set `command`)",
                self.command
            )
        })?;
        let guard = child.stdin.take();
        let mut server = Server { child, port, guard };
        let start = Instant::now();
        loop {
            if let Ok(Some(s)) = server.child.try_wait() {
                server.stop();
                return Err(format!("`{}` exited during startup ({s})", self.command));
            }
            if Self::healthy(port) {
                break;
            }
            if start.elapsed() > self.startup {
                server.stop();
                return Err(format!(
                    "`{}` not healthy after {}s (raise startup_timeout_secs for a large model)",
                    self.command,
                    self.startup.as_secs()
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        *st = Some(server);
        *self.shared.last_use.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        drop(st);
        if let Some(idle) = self.idle {
            reaper(Arc::downgrade(&self.shared), idle);
        }
        Ok(port)
    }

    /// Whether a server is running now.
    pub fn running(&self) -> bool {
        self.shared.state.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }
}

/// Watch one started server: stop it once it has been idle for `idle`. Ends when the server is
/// gone (stopped, idle-stopped or its provider dropped); the next start runs a new watcher.
fn reaper(shared: Weak<Shared>, idle: Duration) {
    let tick = (idle / 4).clamp(Duration::from_millis(50), Duration::from_secs(5));
    let _ = std::thread::Builder::new().name("seagent-llama-idle".into()).spawn(move || loop {
        std::thread::sleep(tick);
        let Some(sh) = shared.upgrade() else { return };
        let port = match sh.state.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            Some(s) => s.port,
            None => return,
        };
        if Managed::busy(port) {
            *sh.last_use.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
            continue;
        }
        let mut st = sh.state.lock().unwrap_or_else(|e| e.into_inner());
        // A request may have stopped or restarted the server since the check above.
        match st.as_ref().map(|s| s.port) {
            None => return,
            Some(p) if p != port => return, // a newer server has its own watcher
            Some(_) => {}
        }
        let quiet = sh.last_use.lock().unwrap_or_else(|e| e.into_inner()).elapsed();
        if quiet >= idle {
            if let Some(s) = st.take() {
                s.stop();
            }
            eprintln!("seagent: {}: local server stopped after {}s idle (starts again on the next request)", sh.id, idle.as_secs());
            return;
        }
    });
}

/// Models of a managed provider without starting its server (`seagent models list`): the
/// configured `models`, else the GGUF the server would load — the file itself, or every
/// `*.gguf` in a router-mode models directory (named by file stem, as llama.cpp names them).
pub fn offline_models(cfg: &ProviderConfig) -> Result<Vec<String>, String> {
    if !cfg.models.is_empty() {
        return Ok(cfg.models.clone());
    }
    let path = cfg.model_path.clone().unwrap_or_default();
    let p = std::path::Path::new(&path);
    let stem = |p: &std::path::Path| p.file_stem().map(|s| s.to_string_lossy().into_owned());
    if p.is_dir() {
        let mut v: Vec<String> = std::fs::read_dir(p)
            .map_err(|e| format!("provider `{}`: model_path {path}: {e}", cfg.id))?
            .flatten()
            .map(|e| e.path())
            .filter(|f| f.extension().is_some_and(|x| x.eq_ignore_ascii_case("gguf")))
            .filter_map(|f| stem(&f))
            .collect();
        v.sort();
        return Ok(v);
    }
    if !p.exists() {
        return Err(format!("provider `{}`: model_path {path} does not exist", cfg.id));
    }
    Ok(stem(p).into_iter().collect())
}

impl CredSource for Managed {
    fn get(&self) -> Result<Cred, ProviderError> {
        let port = self
            .ensure()
            .map_err(|e| ProviderError::new(K::Network, format!("{}: {e}", self.id)))?;
        let mut c = self.inner.get()?;
        c.base = Some(format!("http://127.0.0.1:{port}/v1"));
        Ok(c)
    }
    fn invalidate(&self) -> bool {
        self.inner.invalidate()
    }
    fn describe(&self) -> String {
        format!("managed `{}`", self.command)
    }
}

impl Drop for Managed {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A llama-server stand-in: `/health` and `/slots` (a `busy` file next to it = generating).
    fn fake(d: &std::path::Path) -> std::path::PathBuf {
        let fake = d.join("fake_server.py");
        std::fs::write(&fake, "import sys, os, http.server\nport=int(sys.argv[sys.argv.index('--port')+1])\nbusy=os.path.join(os.path.dirname(os.path.abspath(sys.argv[0])),'busy')\nclass H(http.server.BaseHTTPRequestHandler):\n    def log_message(self,*a): pass\n    def do_GET(self):\n        body = (b'[{\"id\":0,\"is_processing\":true}]' if os.path.exists(busy) else b'[{\"id\":0,\"is_processing\":false}]') if self.path=='/slots' else b'ok'\n        self.send_response(200); self.end_headers(); self.wfile.write(body)\nhttp.server.HTTPServer(('127.0.0.1',port),H).serve_forever()\n").unwrap();
        fake
    }

    /// `shutdown_all` stops every server of the process: tests that start one run one at a time.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: Mutex<()> = Mutex::new(());
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn managed(tag: &str) -> (Managed, std::path::PathBuf) {
        let d = seagent_core::tmp::scratch_path(format!("seagent-llama-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let model = d.join("m.gguf");
        std::fs::write(&model, b"GGUF").unwrap();
        let fake = fake(&d);
        let cfg = ProviderConfig {
            id: format!("llama-{tag}"),
            model_path: Some(model.to_string_lossy().into()),
            command: Some("python3".into()),
            startup_timeout_secs: Some(60),
            args: vec![],
            ..Default::default()
        };
        let mut m = Managed::new(&cfg, Arc::new(crate::auth::Static::new(None))).unwrap();
        m.args.insert(0, fake.to_string_lossy().into());
        m.args.insert(0, "-I".into());
        (m, d)
    }

    fn pid_of(m: &Managed) -> i32 {
        m.shared.state.lock().unwrap().as_ref().unwrap().child.id() as i32
    }

    #[test]
    fn starts_health_checks_and_restarts() {
        let _serial = serial();
        let (m, _d) = managed("restart");
        let c = m.get().unwrap();
        let base = c.base.unwrap();
        assert!(base.starts_with("http://127.0.0.1:") && base.ends_with("/v1"));
        let p1 = m.ensure().unwrap();
        assert_eq!(base, format!("http://127.0.0.1:{p1}/v1"), "reused");
        let pid = pid_of(&m);
        seagent_core::proc::kill_group(pid, Duration::from_millis(500));
        assert!(seagent_core::proc::wait_until(Duration::from_secs(5), || !seagent_core::proc::group_alive(pid)));
        assert!(m.ensure().is_ok(), "restarted after exit");
        let bad = ProviderConfig {
            id: "bad".into(),
            model_path: Some("/nonexistent.gguf".into()),
            ..Default::default()
        };
        assert!(Managed::new(&bad, Arc::new(crate::auth::Static::new(None))).is_err());
    }

    /// #17: `std::process::exit` skips `Drop`, so `shutdown_all` must stop the server (and the
    /// whole group: on macOS the guard shell and the server under it).
    #[test]
    fn shutdown_all_stops_every_server() {
        let _serial = serial();
        let (m, _d) = managed("shutdown");
        m.ensure().unwrap();
        let pid = pid_of(&m);
        assert!(seagent_core::proc::group_alive(pid));
        assert!(shutdown_all() >= 1);
        assert!(!m.running());
        assert!(seagent_core::proc::wait_until(Duration::from_secs(5), || !seagent_core::proc::group_alive(pid)), "server group {pid} survived shutdown_all");
        assert!(m.ensure().is_ok(), "the next request starts it again");
    }

    /// #19: `idle_timeout_secs` stops an idle server, never a generating one, and the next
    /// request starts it again.
    #[test]
    fn idle_server_is_stopped_and_a_busy_one_is_not() {
        let _serial = serial();
        let (mut m, d) = managed("idle");
        m.idle = Some(Duration::from_millis(600));
        std::fs::write(d.join("busy"), b"").unwrap();
        let p1 = m.ensure().unwrap();
        std::thread::sleep(Duration::from_millis(1500));
        assert!(m.running(), "a generating server is never idle");
        assert_eq!(m.ensure().unwrap(), p1);
        std::fs::remove_file(d.join("busy")).unwrap();
        let pid = pid_of(&m);
        assert!(seagent_core::proc::wait_until(Duration::from_secs(5), || !m.running()), "idle server kept running");
        assert!(seagent_core::proc::wait_until(Duration::from_secs(5), || !seagent_core::proc::group_alive(pid)));
        assert!(m.ensure().is_ok(), "restarted on the next request");
    }

    #[test]
    fn startup_and_idle_timeouts_are_separate() {
        let c = |idle: Option<u64>, startup: Option<u64>| timeouts(&ProviderConfig { id: "t".into(), idle_timeout_secs: idle, startup_timeout_secs: startup, ..Default::default() });
        assert_eq!(c(None, None), (Duration::from_secs(DEFAULT_STARTUP_SECS), None));
        assert_eq!(c(Some(600), None), (Duration::from_secs(600), Some(Duration::from_secs(600))), "pre-0.2.2 configs keep their long start-up wait");
        assert_eq!(c(Some(30), None), (Duration::from_secs(DEFAULT_STARTUP_SECS), Some(Duration::from_secs(30))), "a short idle stop never shortens start-up");
        assert_eq!(c(Some(30), Some(900)), (Duration::from_secs(900), Some(Duration::from_secs(30))));
    }

    /// #18: listing never needs the (empty) base URL of a managed provider.
    #[test]
    fn offline_models_come_from_config_or_the_gguf() {
        let d = seagent_core::tmp::scratch_path(format!("seagent-llama-models-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("gemma-4-E4B-it-Q4_K_M.gguf"), b"GGUF").unwrap();
        std::fs::write(d.join("qwen3-8b.GGUF"), b"GGUF").unwrap();
        std::fs::write(d.join("notes.txt"), b"").unwrap();
        let mut cfg = ProviderConfig { id: "l".into(), model_path: Some(d.to_string_lossy().into()), ..Default::default() };
        assert_eq!(offline_models(&cfg).unwrap(), vec!["gemma-4-E4B-it-Q4_K_M", "qwen3-8b"]);
        cfg.model_path = Some(d.join("gemma-4-E4B-it-Q4_K_M.gguf").to_string_lossy().into());
        assert_eq!(offline_models(&cfg).unwrap(), vec!["gemma-4-E4B-it-Q4_K_M"]);
        cfg.models = vec!["gemma-4-e4b".into()];
        assert_eq!(offline_models(&cfg).unwrap(), vec!["gemma-4-e4b"]);
    }
}
