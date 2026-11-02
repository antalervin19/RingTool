use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const RELEASE_API: &str = "https://api.github.com/repos/antalervin19/RingTool/releases/latest";
const SETUP_NAME: &str = "RingToolSetup.exe";

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn parse_version(text: &str) -> Vec<u32> {
    text.trim()
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .map_while(|part| part.parse::<u32>().ok())
        .collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let (mut a, mut b) = (parse_version(latest), parse_version(current));
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a > b
}

fn latest_tag() -> Option<String> {
    let body = ureq::get(RELEASE_API)
        .set("User-Agent", "RingTool-Updater")
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .call()
        .ok()?
        .into_string()
        .ok()?;

    let rest = body.split_once("\"tag_name\"")?.1;
    let rest = rest.split_once('"')?.1;
    Some(rest.split_once('"')?.0.to_string())
}

pub fn spawn_check() {
    std::thread::spawn(|| {
        let Some(setup) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(SETUP_NAME)))
            .filter(|path| path.exists())
        else {
            return;
        };

        let Some(tag) = latest_tag() else {
            return;
        };

        if is_newer(&tag, CURRENT_VERSION) {
            let _ = Command::new(setup)
                .arg("/update")
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }
    });
}

pub fn updated_version() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--updated" {
            return args.next();
        }
    }
    None
}
