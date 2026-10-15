//! 真实 Linux Server 的五笔单用、拼音混输、个人码表与缺表回退。
#![cfg(target_os = "linux")]

#[path = "support/server.rs"]
mod server;

use qingjian_platform::protocol::{
    KeyEvent, KeyModifiers, PROTOCOL_VERSION, read_message, write_message,
};
use serde_json::{Value, json};
use server::Server;
use std::path::Path;

fn query(server: &mut Server, input: &str) -> Vec<String> {
    let mut stream = server.connect();
    write_message(
        &mut stream,
        &json!({"OpenSession": {"session": 1, "app": null, "protocol": PROTOCOL_VERSION}}),
    )
    .unwrap();
    read_message::<_, Value>(&mut stream).unwrap();
    write_message(
        &mut stream,
        &json!({"LinuxHello": {"version": 3, "session": 1, "generation": 1, "context": "code-test"}}),
    )
    .unwrap();
    read_message::<_, Value>(&mut stream).unwrap();
    write_message(
        &mut stream,
        &json!({"LinuxEvent": {"session": 1, "event": {"Capabilities": {
            "sensitive": false, "password": false, "disabled": false
        }}}}),
    )
    .unwrap();
    read_message::<_, Value>(&mut stream).unwrap();
    let mut result = Value::Null;
    for character in input.chars() {
        write_message(
            &mut stream,
            &json!({"LinuxEvent": {"session": 1, "event": {"Key": {
                "release": false,
                "event": KeyEvent::new(character as u32, Some(character), KeyModifiers::default())
            }}}}),
        )
        .unwrap();
        result = read_message::<_, Value>(&mut stream).unwrap().unwrap();
    }
    result["KeyResult"]["frame"]["candidates"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["text"].as_str().map(str::to_owned))
        .collect()
}

fn setup(directory: &Path, scheme: &str, wubi: &str, bundle: bool, user: bool) -> Server {
    std::fs::create_dir_all(directory.join("config/qingjian")).unwrap();
    std::fs::write(
        directory.join("config/qingjian/config.toml"),
        format!("[general]\nscheme = \"{scheme}\"\nwubi = \"{wubi}\"\n"),
    )
    .unwrap();
    if bundle {
        let path = directory.join("resources/assets/wubi/wubi86.tsv");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "开发\tgant\t100\n").unwrap();
    }
    if user {
        let path = directory.join("data/qingjian/wubi/wubi86.tsv");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "测试\tgant\t100\n").unwrap();
    }
    Server::start(directory.to_path_buf())
}

#[test]
fn linux_input_schemes_load_from_config_and_resolve_code_tables() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("qingjian-code-{}-{nonce}", std::process::id()));
    for (name, scheme, wubi, bundle, user, input, expected) in [
        (
            "shuangpin",
            "xiaohe",
            "",
            false,
            false,
            "nihc",
            Some("你好"),
        ),
        ("zhuyin", "zhuyin", "", false, false, "su3", Some("你")),
        ("wubi", "none", "wubi86", true, false, "gant", Some("开发")),
        (
            "wubi-no-pinyin",
            "none",
            "wubi86",
            true,
            false,
            "nihao",
            None,
        ),
        (
            "mixed-code",
            "pinyin",
            "wubi86",
            true,
            false,
            "gant",
            Some("开发"),
        ),
        (
            "mixed-pinyin",
            "pinyin",
            "wubi86",
            true,
            false,
            "nihao",
            Some("你好"),
        ),
        (
            "mixed-shuangpin",
            "xiaohe",
            "wubi86",
            true,
            false,
            "gant",
            Some("开发"),
        ),
        (
            "mixed-zhuyin",
            "zhuyin",
            "wubi86",
            true,
            false,
            "gant",
            Some("开发"),
        ),
        ("user", "none", "wubi86", true, true, "gant", Some("测试")),
        (
            "fallback",
            "none",
            "wubi86",
            false,
            false,
            "nihao",
            Some("你好"),
        ),
        ("both-off", "none", "", false, false, "nihao", Some("你好")),
    ] {
        let directory = base.join(name);
        let mut server = setup(&directory, scheme, wubi, bundle, user);
        let candidates = query(&mut server, input);
        assert_eq!(
            candidates.first().map(String::as_str),
            expected,
            "{name}: {candidates:?}"
        );
        drop(server);
        std::fs::remove_dir_all(directory).unwrap();
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn running_server_switches_between_wubi_and_pinyin_without_restart() {
    use std::time::{Duration, Instant};

    let directory =
        std::env::temp_dir().join(format!("qingjian-code-reload-{}", std::process::id()));
    let mut server = setup(&directory, "none", "wubi86", true, false);
    assert_eq!(
        query(&mut server, "gant").first().map(String::as_str),
        Some("开发")
    );
    std::fs::write(
        directory.join("config/qingjian/config.toml"),
        "[general]\nscheme = \"pinyin\"\nwubi = \"\"\n",
    )
    .unwrap();
    let started = Instant::now();
    loop {
        if query(&mut server, "nihao").first().map(String::as_str) == Some("你好") {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(4));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(query(&mut server, "gant").iter().all(|word| word != "开发"));
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}
