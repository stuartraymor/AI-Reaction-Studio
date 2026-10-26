//! Commentary-only captions, estimated from word counts within each measured narration.
use crate::{Result, exec, id};
use filmcraft_time::{FrameRate, Tick};
use serde_json::json;

pub(crate) fn cues(text: &str, start: Tick, speech: Tick) -> Result<Vec<(Tick, Tick, String)>> {
    if text.len() > 2000 || start.0 < 0 || speech.0 <= 0 || speech.0 > 120 * crate::TICKS {
        return Err("invalid commentary caption range or text length".into());
    }
    let frame = FrameRate { num: 24, den: 1 }.frame_duration();
    if start.0 % frame.0 != 0 {
        return Err("caption start must align with an export frame".into());
    }
    let frames = speech.0 / frame.0;
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Err("cannot caption empty commentary".into());
    }
    let total = i64::try_from(words.len())?;
    let mut out = Vec::new();
    for (i, chunk) in words.chunks(6).enumerate() {
        let first = i64::try_from(i)?.checked_mul(6).ok_or("caption index overflow")?;
        let last = first.saturating_add(6).min(total);
        let a = frames.checked_mul(first).ok_or("caption timing overflow")? / total;
        let b = frames.checked_mul(last).ok_or("caption timing overflow")? / total;
        if b <= a {
            return Err("narration is too short for readable caption cues".into());
        }
        let a = start.0.checked_add(a.checked_mul(frame.0).ok_or("caption start overflow")?).ok_or("caption start overflow")?;
        let b = start.0.checked_add(b.checked_mul(frame.0).ok_or("caption end overflow")?).ok_or("caption end overflow")?;
        let line = if chunk.join(" ").chars().count() > 34 && chunk.len() > 3 {
            format!("{}\n{}", chunk.get(..3).ok_or("caption chunk missing")?.join(" "), chunk.get(3..).ok_or("caption chunk missing")?.join(" "))
        } else {
            chunk.join(" ")
        };
        out.push((Tick(a), Tick(b), line));
    }
    Ok(out)
}
pub(crate) fn add(s: &mut filmcraft_engine::Session, text: &str, start: Tick, speech: Tick) -> Result<()> {
    for (start, end, text) in cues(text, start, speech)? {
        let r = exec(s, "captions.add", json!({"track":"C1","text":text,"time":start.0,"durationSeconds":(end-start).seconds()}))?;
        exec(s, "captions.setTimes", json!({"caption":id(&r,"caption")?,"startTime":start.0,"endTime":end.0}))?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_caption_text_is_preserved_and_stays_inside_narration() {
        let text = "A café reflected in water cannot prove anything about its safety.";
        let start = Tick(2 * crate::TICKS);
        let speech = Tick(7 * crate::TICKS + crate::TICKS / 10);
        let cues = cues(text, start, speech).unwrap();
        assert_eq!(cues.first().unwrap().0, start);
        assert!(cues.last().unwrap().1 <= start + speech);
        assert!(cues.windows(2).all(|pair| pair[0].1 == pair[1].0));
        assert_eq!(
            cues.iter().map(|c| c.2.as_str()).collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>(),
            text.split_whitespace().collect::<Vec<_>>()
        );
    }
    #[test]
    fn caption_ranges_reject_hostile_inputs() {
        for (start, speech) in [(i64::MAX, 1), (-1, crate::TICKS), (0, 0), (0, i64::MAX), (0, 1)] {
            assert!(cues("Several words need time to read.", Tick(start), Tick(speech)).is_err());
        }
    }
}
