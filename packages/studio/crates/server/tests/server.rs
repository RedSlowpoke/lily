//! lily-studio-server end to end (DECISIONS D55): accounts made with
//! `user add`, signing in, the socket to a worker, the worker's commands
//! inside the user's scores and nowhere else, a second tab taking the worker
//! over. Without a sandbox, which a Mac does not have; the sandbox's own test
//! runs bubblewrap on Linux (scripts/sandbox-test.sh). The compile is checked
//! only where lilypond is installed.
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const EXE: &str = env!("CARGO_BIN_EXE_lily-studio-server");
const WAIT: Duration = Duration::from_secs(60);

/// The runtime directory as `esbuild.mjs` puts it together: @lily/common's
/// files and the studio's.
fn runtime_dir() -> PathBuf {
    static RUNTIME: OnceLock<PathBuf> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
            let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("runtime");
            std::fs::create_dir_all(&dir).unwrap();
            for source in ["common/runtime", "studio/runtime"] {
                for file in std::fs::read_dir(packages.join(source)).unwrap() {
                    let file = file.unwrap();
                    std::fs::copy(file.path(), dir.join(file.file_name())).unwrap();
                }
            }
            dir
        })
        .clone()
}

async fn add_user(data: &Path, name: &str, password: &str) {
    let mut child = Command::new(EXE)
        .args(["user", "add", name, "--password-stdin", "--data"])
        .arg(data)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin
        .write_all(format!("{password}\n").as_bytes())
        .await
        .unwrap();
    drop(stdin);
    assert!(child.wait().await.unwrap().success());
}

struct Server {
    _child: Child,
    address: String,
}

async fn start(data: &Path, site: &Path) -> Server {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let address = format!("127.0.0.1:{port}");
    let mut child = Command::new(EXE)
        .args(["serve", "--sandbox", "none", "--listen", &address, "--data"])
        .arg(data)
        .arg("--site")
        .arg(site)
        .arg("--runtime")
        .arg(runtime_dir())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();
    tokio::time::timeout(WAIT, async {
        while let Some(line) = stderr.next_line().await.unwrap() {
            if line.starts_with("Lily Studio is at") {
                return;
            }
        }
        panic!("the server ended before listening");
    })
    .await
    .expect("the server listens");
    // The workers' lines, for a failure to explain itself.
    tokio::spawn(async move {
        while let Ok(Some(line)) = stderr.next_line().await {
            eprintln!("server: {line}");
        }
    });
    Server {
        _child: child,
        address,
    }
}

/// A plain HTTP/1.1 request: the status and the headers and body as text.
async fn request(address: &str, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.unwrap();
    let status = answer[9..12].parse().unwrap();
    (status, answer)
}

fn post(address: &str, path: &str, body: &str, origin: Option<&str>) -> String {
    let origin = origin
        .map(|origin| format!("Origin: {origin}\r\n"))
        .unwrap_or_default();
    format!(
        "POST {path} HTTP/1.1\r\nHost: {address}\r\n{origin}Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

async fn sign_in(address: &str, name: &str, password: &str) -> String {
    let origin = format!("http://{address}");
    let (status, answer) = request(
        address,
        &post(
            address,
            "/login",
            &format!("name={name}&password={password}"),
            Some(&origin),
        ),
    )
    .await;
    assert_eq!(status, 303, "{answer}");
    let cookie = answer
        .lines()
        .find_map(|line| line.strip_prefix("set-cookie: "))
        .expect("a session cookie");
    assert!(cookie.contains("HttpOnly"));
    cookie.split(';').next().unwrap().to_string()
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Page {
    socket: Socket,
    next: u64,
    /// Events read while waiting for an answer.
    events: Vec<Value>,
}

impl Page {
    async fn open(address: &str, cookie: &str) -> (Page, Value) {
        let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
        let headers = request.headers_mut();
        headers.insert("cookie", cookie.parse().unwrap());
        headers.insert("origin", format!("http://{address}").parse().unwrap());
        let (socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        let mut page = Page {
            socket,
            next: 1,
            events: Vec::new(),
        };
        let hello = page.message().await;
        (page, hello)
    }

    async fn message(&mut self) -> Value {
        loop {
            let message = tokio::time::timeout(WAIT, self.socket.next())
                .await
                .expect("a message in time")
                .expect("the socket open")
                .unwrap();
            if let Message::Text(text) = message {
                return serde_json::from_str(&text).unwrap();
            }
        }
    }

    /// Calls `command`; its answer, Ok or Err.
    async fn call(&mut self, command: &str, args: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        let call = json!({ "id": id, "command": command, "args": args });
        self.socket
            .send(Message::Text(call.to_string().into()))
            .await
            .unwrap();
        loop {
            let message = self.message().await;
            if message["id"] == id {
                return match message.get("error") {
                    Some(error) => Err(error.as_str().unwrap().to_string()),
                    None => Ok(message["ok"].clone()),
                };
            }
            if message.get("event").is_some() {
                self.events.push(message["event"].clone());
            }
        }
    }

    /// The next compile that finished, from what was read or what comes.
    async fn finished(&mut self) -> Value {
        loop {
            if let Some(index) = self
                .events
                .iter()
                .position(|e| e["type"] == "compile" && e["event"]["kind"] == "finished")
            {
                return self.events.remove(index)["event"]["outcome"].clone();
            }
            let message = self.message().await;
            if message.get("event").is_some() {
                self.events.push(message["event"].clone());
            }
        }
    }
}

fn lilypond_installed() -> bool {
    std::process::Command::new("lilypond")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// 08 §5.1 (D170): the engine's files that carry their hash are kept a year,
/// the manifest and the licences asked again, and a file the build
/// precompressed is sent as its .br to a browser that takes brotli.
#[tokio::test(flavor = "multi_thread")]
async fn the_engine_s_files_are_cached_by_their_names() {
    let scratch = tempfile::tempdir().unwrap();
    let data = scratch.path().join("data");
    let site = scratch.path().join("site");
    let engine = site.join("engine");
    std::fs::create_dir_all(&engine).unwrap();
    std::fs::write(site.join("index.html"), "<!doctype html>").unwrap();
    std::fs::write(site.join("app.js"), "// app").unwrap();
    // A .br outside engine/ may be older than its file: never sent.
    std::fs::write(site.join("app.js.br"), "stale").unwrap();
    std::fs::write(engine.join("manifest.json"), "{}").unwrap();
    std::fs::write(engine.join("NOTICE.txt"), "notice").unwrap();
    std::fs::write(engine.join("core.0123456789ab.wasm"), "raw core").unwrap();
    std::fs::write(engine.join("core.0123456789ab.wasm.br"), "brotli core").unwrap();
    add_user(&data, "ada", "correct horse").await;
    let server = start(&data, &site).await;
    let address = server.address.clone();
    let get = |path: &str, encodings: &str| {
        format!(
            "GET {path} HTTP/1.1\r\nHost: {address}\r\nAccept-Encoding: {encodings}\r\nConnection: close\r\n\r\n"
        )
    };

    let (status, answer) =
        request(&address, &get("/engine/core.0123456789ab.wasm", "gzip, br")).await;
    assert_eq!(status, 200);
    assert!(
        answer.contains("cache-control: public, max-age=31536000, immutable"),
        "{answer}"
    );
    assert!(answer.contains("content-encoding: br"), "{answer}");
    assert!(answer.ends_with("brotli core"), "{answer}");
    let (status, answer) = request(&address, &get("/engine/core.0123456789ab.wasm", "gzip")).await;
    assert_eq!(status, 200);
    assert!(answer.ends_with("raw core"), "{answer}");
    for path in ["/engine/manifest.json", "/engine/NOTICE.txt"] {
        let (status, answer) = request(&address, &get(path, "br")).await;
        assert_eq!(status, 200);
        assert!(
            answer.contains("cache-control: no-cache"),
            "{path}: {answer}"
        );
    }
    let (status, answer) = request(&address, &get("/engine/core.ffffffffffff.wasm", "br")).await;
    assert_eq!(status, 404);
    assert!(
        !answer.contains("immutable"),
        "a miss is not kept: {answer}"
    );
    let (_, answer) = request(&address, &get("/app.js", "br")).await;
    assert!(!answer.contains("cache-control"), "{answer}");
    assert!(answer.ends_with("// app"), "{answer}");
}

/// Live preview by the page's engine (D171) turns the server's live switch
/// off: an edit then compiles nothing on the server, and an unsaved text it
/// forgot is not compiled when the switch goes off. On again, the unsaved
/// text it was given compiles there, as before (D36).
#[tokio::test(flavor = "multi_thread")]
async fn edits_compile_nothing_on_the_server_while_its_live_switch_is_off() {
    let scratch = tempfile::tempdir().unwrap();
    let data = scratch.path().join("data");
    let site = scratch.path().join("site");
    std::fs::create_dir_all(&site).unwrap();
    std::fs::write(site.join("index.html"), "<!doctype html>").unwrap();
    add_user(&data, "ada", "correct horse").await;
    let server = start(&data, &site).await;
    let cookie = sign_in(&server.address, "ada", "correct horse").await;
    let (mut page, _) = Page::open(&server.address, &cookie).await;
    let opened = page.call("open_sample", json!({})).await.unwrap();
    let sample = opened["file"].as_str().unwrap().to_string();
    let saved = page
        .call("read_file", json!({ "file": sample }))
        .await
        .unwrap();
    let typed = format!("{}\n{{ c'4 }}\n", saved.as_str().unwrap());
    // The score shown, compiled: the one the switch going off would compile again.
    page.call("show_score", json!({ "file": sample }))
        .await
        .unwrap();
    page.finished().await;

    // As the engine takes over: the text forgotten, then the switch off.
    page.call("edited", json!({ "file": sample, "text": typed }))
        .await
        .unwrap();
    page.call("edited", json!({ "file": sample, "text": null }))
        .await
        .unwrap();
    page.call("set_live", json!({ "on": false })).await.unwrap();
    page.call("edited", json!({ "file": sample, "text": typed }))
        .await
        .unwrap();
    // Past the live delay's longest wait (750 ms): no compile comes.
    let compiled = tokio::time::timeout(Duration::from_millis(2_000), async {
        loop {
            let message = page.message().await;
            if message["event"]["type"] == "compile" {
                return message["event"]["event"]["kind"].clone();
            }
        }
    })
    .await;
    assert!(
        compiled.is_err(),
        "no compile while the switch is off: {compiled:?}"
    );
    page.events.clear();

    if lilypond_installed() {
        page.call("set_live", json!({ "on": true })).await.unwrap();
        let outcome = page.finished().await;
        assert_eq!(outcome["rootFile"], sample.as_str());
        assert_eq!(outcome["state"], "ok", "{outcome}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn users_sign_in_and_work_only_in_their_own_scores() {
    let scratch = tempfile::tempdir().unwrap();
    let data = scratch.path().join("data");
    let site = scratch.path().join("site");
    std::fs::create_dir_all(&site).unwrap();
    std::fs::write(
        site.join("index.html"),
        "<!doctype html><title>studio</title>",
    )
    .unwrap();
    std::fs::write(site.join("app.js"), "// app").unwrap();
    add_user(&data, "ada", "correct horse").await;
    add_user(&data, "bo", "battery staple").await;
    let server = start(&data, &site).await;
    let address = server.address.clone();

    // Signed out: the sign-in page, no studio, no socket.
    let get = |path: &str, cookie: &str| {
        format!(
            "GET {path} HTTP/1.1\r\nHost: {address}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"
        )
    };
    let (status, answer) = request(&address, &get("/", "")).await;
    assert_eq!(status, 303);
    assert!(answer.contains("location: /login"));
    let (status, answer) = request(&address, &get("/login", "")).await;
    assert_eq!(status, 200);
    assert!(answer.contains("content-security-policy: default-src 'none'"));
    let (status, _) = request(&address, &get("/app.js", "")).await;
    assert_eq!(status, 200);

    // A form from another site, and a wrong password.
    let body = "name=ada&password=correct%20horse";
    let (status, _) = request(&address, &post(&address, "/login", body, None)).await;
    assert_eq!(status, 403);
    let (status, _) = request(
        &address,
        &post(&address, "/login", body, Some("http://evil.example")),
    )
    .await;
    assert_eq!(status, 403);
    let origin = format!("http://{address}");
    let (status, answer) = request(
        &address,
        &post(&address, "/login", "name=ada&password=wrong", Some(&origin)),
    )
    .await;
    assert_eq!(status, 401);
    assert!(answer.contains("do not match"));

    let ada = sign_in(&address, "ada", "correct%20horse").await;
    let (status, answer) = request(&address, &get("/", &ada)).await;
    assert_eq!(status, 200);
    assert!(answer.contains("<title>studio</title>"));

    let (mut page, first_hello) = Page::open(&address, &ada).await;
    assert_eq!(first_hello["hello"]["user"], "ada");

    // The workspace starts empty; the sample goes in it.
    let listing = page.call("browse", json!({})).await.unwrap();
    let root = listing["root"].as_str().unwrap().to_string();
    assert!(root.ends_with("/scores"), "{root}");
    assert_eq!(listing["files"], json!([]));
    let opened = page.call("open_sample", json!({})).await.unwrap();
    let sample = opened["file"].as_str().unwrap().to_string();
    assert_eq!(opened["listing"]["folder"], root);
    let text = page
        .call("read_file", json!({ "file": sample }))
        .await
        .unwrap();
    assert!(text.as_str().unwrap().contains("\\version"));

    // Nothing outside the workspace, however it is named.
    for outside in [
        "/etc/passwd.ly".to_string(),
        format!("{root}/../studio/settings.ly"),
        format!("{root}/../../{}/scores/x.ly", "someone"),
    ] {
        let error = page
            .call("read_file", json!({ "file": outside }))
            .await
            .unwrap_err();
        assert!(error.contains("outside your scores"), "{outside}: {error}");
    }
    let error = page
        .call("browse", json!({ "dir": format!("{root}/..") }))
        .await
        .unwrap_err();
    assert!(error.contains("outside your scores"), "{error}");
    let error = page.call("chat_list", json!({})).await.unwrap_err();
    assert!(error.contains("not of Lily Studio on the web"), "{error}");
    let error = page
        .call("separate", json!({ "request": { "input": sample } }))
        .await
        .unwrap_err();
    assert!(error.contains("not of Lily Studio on the web"), "{error}");

    // A folder, an upload into it, and the same file back.
    let parts = page
        .call("create_folder", json!({ "parent": root, "name": "Parts" }))
        .await
        .unwrap();
    let parts = parts.as_str().unwrap().to_string();
    assert!(
        page.call("create_folder", json!({ "parent": root, "name": "../up" }))
            .await
            .is_err()
    );
    let melody = "melody = { c'4 d' e' f' }\n";
    let data64 = base64_encode(melody.as_bytes());
    let uploaded = page
        .call(
            "upload",
            json!({ "dir": parts, "name": "melody.ily", "data": data64 }),
        )
        .await
        .unwrap();
    assert!(
        uploaded["path"]
            .as_str()
            .unwrap()
            .ends_with("/Parts/melody.ily")
    );
    let again = page
        .call(
            "upload",
            json!({ "dir": parts, "name": "melody.ily", "data": data64 }),
        )
        .await
        .unwrap();
    assert_eq!(again, json!({ "exists": true }));
    assert!(
        page.call(
            "upload",
            json!({ "dir": parts, "name": "run.sh", "data": data64 })
        )
        .await
        .unwrap_err()
        .contains("not a LilyPond file")
    );
    let downloaded = page
        .call("download", json!({ "file": format!("{parts}/melody.ily") }))
        .await
        .unwrap();
    assert_eq!(downloaded["name"], "melody.ily");
    assert_eq!(downloaded["data"], data64);

    // Import Guitar Pro (D166) refuses what is not a Guitar Pro file, and
    // GP7's .gp, before anything is written, and never writes outside the
    // workspace. Until the readers land no file imports; the session crate's
    // tests cover the score written and opened.
    let refused = page
        .call(
            "import_guitar_pro",
            json!({ "name": "melody.gp5", "data": data64, "path": format!("{parts}/melody") }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "melody.gp5 is not a Guitar Pro file; Lily Studio imports .gp3, .gp4, .gp5 and .gpx."
    );
    let newer = page
        .call(
            "import_guitar_pro",
            json!({ "name": "song.gp", "data": base64_encode(b"PK\x03\x04rest") }),
        )
        .await
        .unwrap_err();
    assert!(newer.contains("is a Guitar Pro 7 or 8 file"), "{newer}");
    let listed = page.call("browse", json!({ "dir": parts })).await.unwrap();
    assert_eq!(listed["files"].as_array().unwrap().len(), 1, "{listed}");
    let error = page
        .call(
            "import_guitar_pro",
            json!({ "name": "../up.gp5", "data": data64 }),
        )
        .await
        .unwrap_err();
    assert!(error.contains("cannot be used as a name"), "{error}");

    // Export for GarageBand (D162) gives the page lilypond's MIDI rewritten
    // by the renderer's plan, as the session crate's fixture has them; the
    // muted Violin is left out when the page's checkbox says so.
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../session/tests/fixtures");
    let midi = base64_encode(&std::fs::read(fixtures.join("garageband.midi")).unwrap());
    let plan: Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("garageband.plan.json")).unwrap())
            .unwrap();
    let muted = json!([{ "track": 3, "name": "Violin" }]);
    let export = |leave_out: bool| json!({ "rootFile": sample, "midi": midi, "plan": plan, "muted": muted, "leaveOut": leave_out });
    let exported = page.call("export_garageband", export(false)).await.unwrap();
    let score = Path::new(&sample).file_stem().unwrap().to_string_lossy();
    assert_eq!(exported["name"], format!("{score}.mid"), "{exported}");
    assert_eq!(exported["leftOut"], 0);
    let expected = std::fs::read(fixtures.join("garageband.expected.mid")).unwrap();
    assert_eq!(exported["data"], base64_encode(&expected));
    let without = page.call("export_garageband", export(true)).await.unwrap();
    assert_eq!(without["leftOut"], 1);
    let typed: lily_session::midi_export::GarageBandPlan =
        serde_json::from_value(plan.clone()).unwrap();
    let rewritten = lily_session::midi_export::for_garageband(
        &std::fs::read(fixtures.join("garageband.midi")).unwrap(),
        &typed.without_parts(&[3]),
    )
    .unwrap();
    assert_eq!(without["data"], base64_encode(&rewritten));
    assert_eq!(
        &rewritten[10..12],
        &[0, 6],
        "the control track and five parts"
    );
    let error = page
        .call(
            "export_garageband",
            json!({ "rootFile": "/etc/passwd.ly", "midi": midi, "plan": plan, "muted": [] }),
        )
        .await
        .unwrap_err();
    assert!(error.contains("outside your scores"), "{error}");

    // A new score from a template, which does not replace one unasked.
    let created = page
        .call(
            "new_score",
            json!({ "template": "melody", "path": format!("{root}/Song") }),
        )
        .await
        .unwrap();
    let song = created["file"].as_str().unwrap().to_string();
    assert!(song.ends_with("/Song.ly"));
    let exists = page
        .call("new_score", json!({ "template": "melody", "path": song }))
        .await
        .unwrap();
    assert_eq!(exists, json!({ "exists": true }));

    if lilypond_installed() {
        page.call(
            "save_file",
            json!({ "file": sample, "text": "\\version \"2.24.0\"\n\\score {\n  <<\n    \\new Staff \\new Voice = \"v\" { c'4 d' e' f' }\n    \\new Lyrics \\lyricsto \"v\" { la la la la }\n  >>\n  \\layout { }\n  \\midi { }\n}\n" }),
        )
        .await
        .unwrap();
        let outcome = page.finished().await;
        assert_eq!(outcome["state"], "ok", "{outcome}");
        assert!(
            outcome["svg"][0]
                .as_str()
                .is_some_and(|svg| svg.contains("<svg"))
        );
        // Karaoke's words reach the web's page by the server's compile (D133).
        assert_eq!(outcome["timing"]["lyrics"][3]["text"], "la", "{outcome}");
    } else {
        eprintln!("lilypond is not installed: the compile is not checked");
    }

    // A second tab takes the worker over; the first is told.
    let (mut second, hello) = Page::open(&address, &ada).await;
    assert_eq!(
        hello["hello"]["instance"], first_hello["hello"]["instance"],
        "the same worker"
    );
    let replaced = page.message().await;
    assert_eq!(replaced, json!({ "replaced": true }));
    let listed = second.call("list_folder", json!({})).await.unwrap();
    assert_eq!(listed["folder"], root);

    // Another user has scores of their own, and cannot reach ada's.
    let bo = sign_in(&address, "bo", "battery%20staple").await;
    let (mut theirs, hello) = Page::open(&address, &bo).await;
    assert_eq!(hello["hello"]["user"], "bo");
    let their_root = theirs.call("browse", json!({})).await.unwrap()["root"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(their_root, root);
    let error = theirs
        .call("read_file", json!({ "file": sample }))
        .await
        .unwrap_err();
    assert!(error.contains("outside your scores"), "{error}");

    // Signed out, the session no longer opens anything.
    let (status, _) = request(
        &address,
        &format!(
            "POST /logout HTTP/1.1\r\nHost: {address}\r\nOrigin: {origin}\r\nCookie: {bo}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
    )
    .await;
    assert_eq!(status, 303);
    let (status, _) = request(&address, &get("/", &bo)).await;
    assert_eq!(status, 303);
    drop(server);
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(data)
}
