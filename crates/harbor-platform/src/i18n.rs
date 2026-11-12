#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    ZhHans,
}

impl Lang {
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        let lower = s.to_ascii_lowercase();
        if lower == "zh-hans"
            || lower == "zh_hans"
            || lower == "zh-cn"
            || lower == "zh_cn"
            || lower == "zh"
            || s == "zh-Hans"
        {
            Lang::ZhHans
        } else {
            Lang::En
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::ZhHans => "zh-Hans",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Lang::En => Lang::ZhHans,
            Lang::ZhHans => Lang::En,
        }
    }
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

pub fn t<'a>(lang: Lang, key: &'a str) -> &'a str {
    match key {
        "home" => pick(lang, "Home", "首页"),
        "browser" => pick(lang, "Browser", "浏览"),
        "settings" => pick(lang, "Settings", "设置"),
        "continue_watching" => pick(lang, "Continue Watching", "继续观看"),
        "recently_added" => pick(lang, "Recently Added", "最近添加"),
        "sources" => pick(lang, "Sources", "来源"),
        "no_sources" => pick(lang, "No sources yet", "还没有来源"),
        "resume_from" => pick(lang, "Resume from 12:04", "从 12:04 继续"),
        "add_source" => pick(lang, "Add Source", "添加来源"),
        "add_source_hint" => pick(lang, "Add a source to get started", "添加一个来源开始使用"),
        "ok_add_source" => pick(lang, "OK — Add Source", "确定 — 添加来源"),
        "ui_language" => pick(lang, "UI language", "界面语言"),
        "lang_en" => pick(lang, "English", "英语"),
        "lang_zh" => pick(lang, "Simplified Chinese", "简体中文"),
        "sort" => pick(lang, "Sort", "排序"),
        "name" => pick(lang, "Name", "名称"),
        "date" => pick(lang, "Date", "日期"),
        "size" => pick(lang, "Size", "大小"),
        "library" => pick(lang, "Library", "资料库"),
        "playing" => pick(lang, "Playing", "播放中"),
        "paused" => pick(lang, "Paused", "已暂停"),
        "continue_prompt" => pick(lang, "Continue watching", "继续观看"),
        "resume_at" => pick(lang, "Resume at", "从该处继续"),
        "continue" => pick(lang, "Continue", "继续"),
        "start_over" => pick(lang, "Start over", "从头播放"),
        "native_unavailable" => pick(lang, "Native video unavailable", "原生视频不可用"),
        "cannot_decode_path" => pick(
            lang,
            "This TV cannot decode on this path",
            "此电视无法在此路径解码",
        ),
        "cannot_decode_file" => pick(
            lang,
            "This TV cannot decode this file",
            "此电视无法解码此文件",
        ),
        "ok_back" => pick(lang, "OK / Back — Return", "确定 / 返回"),
        "choose_type" => pick(lang, "Choose a source type", "选择来源类型"),
        "looking_servers" => pick(lang, "Looking for MediaServers", "正在查找媒体服务器"),
        "searching_lan" => pick(lang, "Searching the LAN...", "正在搜索局域网..."),
        "no_servers" => pick(lang, "No MediaServers found", "未找到媒体服务器"),
        "lan_hint" => pick(
            lang,
            "TV and server must share a LAN",
            "电视与服务器需在同一局域网",
        ),
        "retry" => pick(lang, "Retry", "重试"),
        "smb_share" => pick(lang, "SMB share", "SMB 共享"),
        "host" => pick(lang, "Host", "主机"),
        "share" => pick(lang, "Share", "共享名"),
        "username" => pick(lang, "Username", "用户名"),
        "password" => pick(lang, "Password", "密码"),
        "domain" => pick(lang, "Domain", "域"),
        "save" => pick(lang, "Save", "保存"),
        "auth_failed" => pick(lang, "authentication failed", "认证失败"),
        "unreachable" => pick(lang, "network-unreachable", "网络不可达"),
        "smb_timeout_hint" => pick(
            lang,
            "Host did not respond in 10 seconds",
            "主机 10 秒内未响应",
        ),
        "audio" => pick(lang, "Audio", "音频"),
        "subtitles" => pick(lang, "Subtitles", "字幕"),
        other => other,
    }
}

fn pick(lang: Lang, en: &'static str, zh: &'static str) -> &'static str {
    match lang {
        Lang::En => en,
        Lang::ZhHans => zh,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_cycles() {
        assert_eq!(Lang::parse("en"), Lang::En);
        assert_eq!(Lang::parse("zh-Hans"), Lang::ZhHans);
        assert_eq!(Lang::parse("zh_CN"), Lang::ZhHans);
        assert_eq!(Lang::En.tag(), "en");
        assert_eq!(Lang::ZhHans.tag(), "zh-Hans");
        assert_eq!(Lang::En.next(), Lang::ZhHans);
        assert_eq!(Lang::ZhHans.next(), Lang::En);
    }

    #[test]
    fn en_and_zh_hans_catalog() {
        assert_eq!(t(Lang::En, "settings"), "Settings");
        assert_eq!(t(Lang::ZhHans, "settings"), "设置");
        assert_eq!(t(Lang::ZhHans, "home"), "首页");
        assert_eq!(t(Lang::ZhHans, "add_source"), "添加来源");
        assert_eq!(t(Lang::En, "playing"), "Playing");
        assert_eq!(t(Lang::ZhHans, "playing"), "播放中");
        assert_eq!(t(Lang::ZhHans, "unknown-key"), "unknown-key");
    }
}
