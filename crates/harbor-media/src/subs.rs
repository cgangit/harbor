#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cue {
    pub start_ns: i64,
    pub end_ns: i64,
    pub text: String,
}

pub fn active_text(cues: &[Cue], position_ns: i64) -> Option<&str> {
    cues.iter()
        .find(|c| position_ns >= c.start_ns && position_ns < c.end_ns)
        .map(|c| c.text.as_str())
}

pub fn parse(input: &str) -> Vec<Cue> {
    let trimmed = input.trim_start();
    if trimmed.starts_with("WEBVTT") {
        parse_vtt(input)
    } else if trimmed.contains("Dialogue:") {
        parse_ass(input)
    } else {
        parse_srt(input)
    }
}

fn parse_srt(input: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    let normalized = input.replace("\r\n", "\n");
    for block in normalized.split("\n\n") {
        if let Some(cue) = parse_timed_block(block, ',') {
            cues.push(cue);
        }
    }
    cues
}

fn parse_vtt(input: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    let normalized = input.replace("\r\n", "\n");
    let body = normalized
        .splitn(2, "\n\n")
        .nth(1)
        .unwrap_or(normalized.as_str());
    for block in body.split("\n\n") {
        if block.starts_with("NOTE") || block.starts_with("STYLE") {
            continue;
        }
        if let Some(cue) = parse_timed_block(block, '.') {
            cues.push(cue);
        }
    }
    cues
}

fn parse_ass(input: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    for line in input.lines() {
        let line = line.trim();
        if !line.starts_with("Dialogue:") {
            continue;
        }
        let rest = line.trim_start_matches("Dialogue:").trim();
        let parts: Vec<&str> = rest.splitn(10, ',').collect();
        if parts.len() < 10 {
            continue;
        }
        let Some(start_ns) = parse_ass_time(parts[1]) else {
            continue;
        };
        let Some(end_ns) = parse_ass_time(parts[2]) else {
            continue;
        };
        let text = parts[9].replace("\\N", "\n");
        cues.push(Cue {
            start_ns,
            end_ns,
            text,
        });
    }
    cues
}

fn parse_timed_block(block: &str, frac_sep: char) -> Option<Cue> {
    let mut lines = block.lines().filter(|l| !l.trim().is_empty());
    let first = lines.next()?.trim();
    let timing = if first.contains("-->") {
        first
    } else {
        lines.next()?.trim()
    };
    let (start, end) = timing.split_once("-->")?;
    let start_ns = parse_clock(start.trim(), frac_sep)?;
    let end_raw = end.trim().split_whitespace().next()?;
    let end_ns = parse_clock(end_raw, frac_sep)?;
    let text = lines.collect::<Vec<_>>().join("\n");
    if text.is_empty() {
        return None;
    }
    Some(Cue {
        start_ns,
        end_ns,
        text,
    })
}

fn parse_clock(s: &str, frac_sep: char) -> Option<i64> {
    let s = s.replace(',', ".").replace(frac_sep, ".");
    let (hms, frac) = match s.split_once('.') {
        Some((a, b)) => (a, b),
        None => (s.as_str(), "0"),
    };
    let mut parts = hms.split(':');
    let a: i64 = parts.next()?.parse().ok()?;
    let b: i64 = parts.next()?.parse().ok()?;
    let (h, m, sec) = match parts.next() {
        Some(c) => (a, b, c.parse().ok()?),
        None => (0, a, b),
    };
    let mut ms: i64 = 0;
    if !frac.is_empty() {
        let digits: String = frac.chars().filter(|c| c.is_ascii_digit()).take(3).collect();
        let pad = format!("{digits:0<3}");
        ms = pad.parse().unwrap_or(0);
    }
    Some(((h * 3600 + m * 60 + sec) * 1000 + ms) * 1_000_000)
}

fn parse_ass_time(s: &str) -> Option<i64> {
    parse_clock(s.trim(), '.')
}

pub const SAMPLE_SRT: &str = "1\n00:00:01,000 --> 00:00:04,000\nThe river at dusk\n\n2\n00:00:05,000 --> 00:00:08,000\nKeep the heading\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srt_cues_and_active() {
        let cues = parse(SAMPLE_SRT);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_ns, 1_000_000_000);
        assert_eq!(active_text(&cues, 2_000_000_000), Some("The river at dusk"));
        assert_eq!(active_text(&cues, 4_000_000_000), None);
        assert_eq!(active_text(&cues, 6_000_000_000), Some("Keep the heading"));
    }

    #[test]
    fn vtt_cues() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nHello\n";
        let cues = parse(vtt);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "Hello");
        assert_eq!(cues[0].end_ns, 2_500_000_000);
    }

    #[test]
    fn ass_dialogue_line() {
        let ass = "Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Line one\\NLine two\n";
        let cues = parse(ass);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "Line one\nLine two");
        assert_eq!(cues[0].start_ns, 1_000_000_000);
    }
}
