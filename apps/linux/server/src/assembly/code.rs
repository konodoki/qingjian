//! 按配置装载五笔码表；缺表或读表失败时回退拼音，避免无候选。

use qingjian_core::Engine;
use qingjian_dictionary::CodeTable;
use qingjian_platform::Scheme;
use std::path::{Path, PathBuf};

pub fn find_code_table(user: &Path, root: &Path) -> Option<PathBuf> {
    [
        user.join("wubi/wubi86.tsv"),
        root.join("assets/wubi/wubi86.tsv"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

pub fn configure_code(engine: &mut Engine, scheme: Scheme, wubi: bool, path: Option<&Path>) {
    engine.set_shuangpin(scheme.shuangpin());
    engine.set_zhuyin_mode(scheme == Scheme::Zhuyin);

    let table = if wubi {
        match path {
            Some(path) => match CodeTable::from_path(path) {
                Ok(table) => {
                    tracing::info!(path = %path.display(), entries = table.len(), "五笔码表已加载");
                    Some(table)
                }
                Err(error) => {
                    tracing::error!(path = %path.display(), %error, "五笔码表读不了，继续拼音输入");
                    None
                }
            },
            None => {
                tracing::warn!("五笔码表不存在，继续拼音输入");
                None
            }
        }
    } else {
        None
    };
    engine.set_code_table(table);
    engine.set_phonetic(scheme.is_on() || !engine.is_code_mode());
}
