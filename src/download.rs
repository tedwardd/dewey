use crate::book::{extension_for, Book};
use crate::errors::CliError;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{IsTerminal, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

pub fn sanitize_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
            }
            prev_space = true;
            continue;
        }
        prev_space = false;
        match ch {
            '/' | '\\' | '\0' | '<' | '>' | ':' | '"' | '|' | '?' | '*' => out.push('-'),
            _ => out.push(ch),
        }
    }
    out.trim().to_string()
}

pub fn download_filename(book: &Book, format: &str) -> String {
    let title = sanitize_component(&book.title);
    let title = if title.is_empty() { "untitled".to_string() } else { title };
    let author = if book.authors.is_empty() {
        String::new()
    } else {
        format!(" - {}", sanitize_component(&book.authors.join(", ")))
    };
    format!("{title}{author}.{}", extension_for(format))
}

pub fn resolve_dest(dir: &Path, filename: &str, force: bool) -> Result<PathBuf, CliError> {
    let dest = dir.join(filename);
    if dest.exists() && !force {
        return Err(CliError::Network(format!(
            "{} already exists; use --force to overwrite",
            dest.display()
        )));
    }
    Ok(dest)
}

enum FetchError {
    Status(u16),
    Transport(String),
    Body(String),
    Io(std::io::Error),
}

fn fetch_once(url: &str, dest: &Path) -> Result<u64, FetchError> {
    let resp = ureq::get(url).call().map_err(|e| match e {
        ureq::Error::Status(code, _) => FetchError::Status(code),
        ureq::Error::Transport(t) => FetchError::Transport(t.to_string()),
    })?;
    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok());
    let mut reader = resp.into_reader();
    let mut file = File::create(dest).map_err(FetchError::Io)?;
    let show_progress = std::io::stderr().is_terminal();
    let pb = match total {
        Some(t) => ProgressBar::new(t),
        None => ProgressBar::new_spinner(),
    };
    if show_progress {
        pb.set_style(ProgressStyle::with_template("{bar:40} {bytes}/{total_bytes} {msg}").unwrap());
    }
    let mut buf = [0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let n = reader.read(&mut buf).map_err(|e| {
            // Body read failures (truncated response, connection reset) are transient:
            // drop the partial file and let the caller retry.
            let _ = fs::remove_file(dest);
            FetchError::Body(e.to_string())
        })?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| {
            // Disk errors are not transient, but never leave a partial file behind.
            let _ = fs::remove_file(dest);
            FetchError::Io(e)
        })?;
        written += n as u64;
        if show_progress {
            pb.set_position(written);
        }
    }
    if show_progress {
        pb.finish_and_clear();
    }
    Ok(written)
}

pub fn fetch_to_file(url: &str, dest: &Path) -> Result<u64, CliError> {
    let mut last: Option<CliError> = None;
    for attempt in 0..2 {
        match fetch_once(url, dest) {
            Ok(n) => return Ok(n),
            Err(FetchError::Status(code)) if code >= 500 && attempt == 0 => {
                last = Some(CliError::Network(format!("http status {code} (retrying)")));
            }
            Err(FetchError::Status(code)) => {
                return Err(CliError::Network(format!("http status {code}")));
            }
            Err(FetchError::Transport(t)) if attempt == 0 => {
                last = Some(CliError::Network(format!("transport error (retrying): {t}")));
            }
            Err(FetchError::Transport(t)) => {
                return Err(CliError::Network(format!("transport error: {t}")));
            }
            Err(FetchError::Body(e)) if attempt == 0 => {
                last = Some(CliError::Network(format!("body read error (retrying): {e}")));
            }
            Err(FetchError::Body(e)) => {
                return Err(CliError::Network(format!("body read error: {e}")));
            }
            Err(FetchError::Io(e)) => return Err(CliError::Network(format!("write error: {e}"))),
        }
    }
    Err(last.unwrap_or_else(|| CliError::Network("download failed".into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "libcli-dl-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn spawn_http(
        responses: Vec<(&'static str, &'static [u8], Option<usize>)>,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/b.epub", listener.local_addr().unwrap().port());
        let handle = thread::spawn(move || {
            for (status_line, body, declared_len) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let head = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    declared_len.unwrap_or(body.len())
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body).unwrap();
            }
        });
        (url, handle)
    }

    fn book(title: &str, authors: &[&str]) -> Book {
        Book {
            id: "1".into(),
            title: title.into(),
            authors: authors.iter().map(|a| a.to_string()).collect(),
            languages: vec![],
            published: None,
            description: None,
            categories: vec![],
            formats: vec![],
        }
    }

    #[test]
    fn fetch_success_writes_file() {
        let dir = temp_dir();
        let (url, handle) = spawn_http(vec![("200 OK", b"hello epub", None)]);
        let dest = dir.join("out.epub");
        let n = fetch_to_file(&url, &dest).unwrap();
        assert_eq!(n, 10);
        assert_eq!(fs::read(&dest).unwrap(), b"hello epub");
        handle.join().unwrap();
    }

    #[test]
    fn fetch_retries_once_on_5xx() {
        let dir = temp_dir();
        let (url, handle) =
            spawn_http(vec![("503 Service Unavailable", b"", None), ("200 OK", b"data", None)]);
        let dest = dir.join("out.epub");
        let n = fetch_to_file(&url, &dest).unwrap();
        assert_eq!(n, 4);
        assert_eq!(fs::read(&dest).unwrap(), b"data");
        handle.join().unwrap();
    }

    #[test]
    fn fetch_does_not_retry_404() {
        let dir = temp_dir();
        let (url, handle) = spawn_http(vec![("404 Not Found", b"", None)]);
        let dest = dir.join("out.epub");
        let err = fetch_to_file(&url, &dest).unwrap_err();
        assert_eq!(err.exit_code(), 3);
        assert!(err.to_string().contains("404"), "got: {err}");
        handle.join().unwrap();
    }

    #[test]
    fn fetch_retries_truncated_body_then_succeeds() {
        let dir = temp_dir();
        // First response declares Content-Length 10 but sends only 3 bytes and closes
        // (truncated body); second response is a complete 200 with b"data".
        let (url, handle) = spawn_http(vec![("200 OK", b"dat", Some(10)), ("200 OK", b"data", None)]);
        let dest = dir.join("out.epub");
        let n = fetch_to_file(&url, &dest).unwrap();
        assert_eq!(n, 4);
        assert_eq!(fs::read(&dest).unwrap(), b"data");
        handle.join().unwrap();
    }

    #[test]
    fn fetch_truncated_body_twice_fails_and_leaves_no_partial() {
        let dir = temp_dir();
        // Both responses are truncated: the retry must fail and the destination
        // file (created then partially written) must be removed.
        let (url, handle) =
            spawn_http(vec![("200 OK", b"dat", Some(10)), ("200 OK", b"da", Some(10))]);
        let dest = dir.join("out.epub");
        let err = fetch_to_file(&url, &dest).unwrap_err();
        assert_eq!(err.exit_code(), 3);
        assert!(!dest.exists(), "partial file must be removed, got: {}", dest.display());
        handle.join().unwrap();
    }

    #[test]
    fn connection_refused_is_network_error() {
        let dest = temp_dir().join("out.epub");
        let err = fetch_to_file("http://127.0.0.1:1/x.epub", &dest).unwrap_err();
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn overwrite_refused_without_force() {
        let dir = temp_dir();
        let f = dir.join("x.epub");
        File::create(&f).unwrap();
        let err = resolve_dest(&dir, "x.epub", false).unwrap_err();
        assert_eq!(err.exit_code(), 3);
        assert!(err.to_string().contains("--force"), "got: {err}");
        assert!(resolve_dest(&dir, "x.epub", true).is_ok());
    }

    #[test]
    fn filename_joins_title_and_authors() {
        assert_eq!(
            download_filename(&book("Moby Dick", &["Herman Melville"]), "epub"),
            "Moby Dick - Herman Melville.epub"
        );
        assert_eq!(
            download_filename(&book("Moby Dick", &["A", "B"]), "txt"),
            "Moby Dick - A, B.txt"
        );
        assert_eq!(download_filename(&book("Moby Dick", &[]), "txt"), "Moby Dick.txt");
    }

    #[test]
    fn sanitize_replaces_illegal_chars_and_collapses_space() {
        assert_eq!(sanitize_component("A/B:C*"), "A-B-C-");
        assert_eq!(sanitize_component("  a   b  "), "a b");
        assert_eq!(sanitize_component("Moby Dick; Or, The Whale"), "Moby Dick; Or, The Whale");
    }

    #[test]
    fn filename_sanitizes_author_segment() {
        assert_eq!(
            download_filename(&book("T", &["AC/DC"]), "epub"),
            "T - AC-DC.epub"
        );
    }

    #[test]
    fn filename_falls_back_to_untitled_for_empty_title() {
        assert_eq!(download_filename(&book("", &[]), "epub"), "untitled.epub");
        assert_eq!(
            download_filename(&book("", &["Author"]), "epub"),
            "untitled - Author.epub"
        );
    }
}
