//! 本文中に混ざる `<think>…</think>`（推論モデルの思考）を、ストリーミングのまま
//! [`ChatEvent::Reasoning`] に振り分けるフィルタ。タグがチャンクの境界で分割されて
//! 届いても正しく動くよう、閉じるかどうか未確定な末尾はバッファに保持する。

use crate::event::ChatEvent;

const OPEN_TAG: &str = "<think>";
const CLOSE_TAG: &str = "</think>";

#[derive(Debug, Default)]
pub(crate) struct ThinkTagFilter {
    in_think: bool,
    /// 次のチャンクが届けばタグの続きになるかもしれない、確定していない末尾。
    pending: String,
}

impl ThinkTagFilter {
    /// 新しい本文の断片を渡し、確定した [`ChatEvent`] を返す。
    pub fn push(&mut self, chunk: &str) -> Vec<ChatEvent> {
        let mut combined = std::mem::take(&mut self.pending);
        combined.push_str(chunk);

        let mut events = Vec::new();
        loop {
            let tag = if self.in_think { CLOSE_TAG } else { OPEN_TAG };

            if let Some(pos) = combined.find(tag) {
                push_text_event(&mut events, &combined[..pos], self.in_think);
                self.in_think = !self.in_think;
                combined = combined[pos + tag.len()..].to_string();
                continue;
            }

            let overlap = trailing_tag_prefix_len(&combined, tag);
            if overlap > 0 {
                let split_at = combined.len() - overlap;
                push_text_event(&mut events, &combined[..split_at], self.in_think);
                self.pending = combined[split_at..].to_string();
            } else {
                push_text_event(&mut events, &combined, self.in_think);
            }
            break;
        }

        events
    }

    /// ストリーム終了時に呼ぶ。保留中の末尾は、タグの続きが来ないと確定したので
    /// そのまま文章として吐き出す。
    pub fn finish(self) -> Vec<ChatEvent> {
        let mut events = Vec::new();
        push_text_event(&mut events, &self.pending, self.in_think);
        events
    }
}

fn push_text_event(events: &mut Vec<ChatEvent>, text: &str, in_think: bool) {
    if text.is_empty() {
        return;
    }
    let owned = text.to_string();
    events.push(if in_think {
        ChatEvent::Reasoning(owned)
    } else {
        ChatEvent::Content(owned)
    });
}

/// `text` の末尾が `tag` の先頭部分と重なる最長の長さを返す（0 なら重なりなし）。
/// 次のチャンクでタグが完成するかもしれない分を、確定させずに残すために使う。
fn trailing_tag_prefix_len(text: &str, tag: &str) -> usize {
    let max_overlap = tag.len().saturating_sub(1).min(text.len());
    (1..=max_overlap)
        .rev()
        .find(|&len| text.ends_with(&tag[..len]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_all(filter: &mut ThinkTagFilter, chunks: &[&str]) -> Vec<ChatEvent> {
        let mut events = Vec::new();
        for chunk in chunks {
            events.extend(filter.push(chunk));
        }
        events
    }

    #[test]
    fn タグがなければそのまま本文として返す() {
        let mut filter = ThinkTagFilter::default();
        let events = push_all(&mut filter, &["こんにちは"]);
        assert_eq!(events, vec![ChatEvent::Content("こんにちは".to_string())]);
    }

    #[test]
    fn 一つのチャンクに収まる_think_タグは推論として分離する() {
        let mut filter = ThinkTagFilter::default();
        let events = push_all(&mut filter, &["前置き<think>考え中</think>本文です"]);
        assert_eq!(
            events,
            vec![
                ChatEvent::Content("前置き".to_string()),
                ChatEvent::Reasoning("考え中".to_string()),
                ChatEvent::Content("本文です".to_string()),
            ]
        );
    }

    #[test]
    fn 開きタグがチャンク境界で分割されても認識する() {
        let mut filter = ThinkTagFilter::default();
        let events = push_all(&mut filter, &["前置き<thi", "nk>考え中</think>後"]);
        assert_eq!(
            events,
            vec![
                ChatEvent::Content("前置き".to_string()),
                ChatEvent::Reasoning("考え中".to_string()),
                ChatEvent::Content("後".to_string()),
            ]
        );
    }

    #[test]
    fn 閉じタグが一文字ずつ分割されても認識する() {
        let mut filter = ThinkTagFilter::default();
        let mut events = Vec::new();
        for chunk in [
            "<think>",
            "考え中",
            "<",
            "/",
            "t",
            "h",
            "i",
            "n",
            "k",
            ">",
            "後",
        ] {
            events.extend(filter.push(chunk));
        }
        assert_eq!(
            events,
            vec![
                ChatEvent::Reasoning("考え中".to_string()),
                ChatEvent::Content("後".to_string()),
            ]
        );
    }

    #[test]
    fn タグに似ているが違う文字列は本文として扱う() {
        let mut filter = ThinkTagFilter::default();
        let events = push_all(&mut filter, &["これは<test>タグではない"]);
        assert_eq!(
            events,
            vec![ChatEvent::Content("これは<test>タグではない".to_string())]
        );
    }

    #[test]
    fn 開きタグのないまま終了しても保留分はそのまま本文になる() {
        let mut filter = ThinkTagFilter::default();
        push_all(&mut filter, &["途中で切れた<thi"]);
        let events = filter.finish();
        assert_eq!(events, vec![ChatEvent::Content("<thi".to_string())]);
    }

    #[test]
    fn 開きタグの直後にストリームが終わっても内容を推論として返す() {
        let mut filter = ThinkTagFilter::default();
        // 曖昧さのない本文は push の時点で流れ、閉じタグを待たずに推論として確定する。
        let mut events = push_all(&mut filter, &["<think>まだ終わっていない考え"]);
        events.extend(filter.finish());
        assert_eq!(
            events,
            vec![ChatEvent::Reasoning("まだ終わっていない考え".to_string())]
        );
    }

    #[test]
    fn 推論中に閉じタグの途中で終わっても保留分を推論として返す() {
        let mut filter = ThinkTagFilter::default();
        // 末尾の "</th" は閉じタグの先頭と重なるため、確定させずに保留される。
        let mut events = push_all(&mut filter, &["<think>考え中</th"]);
        events.extend(filter.finish());
        assert_eq!(
            events,
            vec![
                ChatEvent::Reasoning("考え中".to_string()),
                ChatEvent::Reasoning("</th".to_string()),
            ]
        );
    }

    #[test]
    fn 複数回の_think_タグを扱える() {
        let mut filter = ThinkTagFilter::default();
        let events = push_all(&mut filter, &["A<think>1</think>B<think>2</think>C"]);
        assert_eq!(
            events,
            vec![
                ChatEvent::Content("A".to_string()),
                ChatEvent::Reasoning("1".to_string()),
                ChatEvent::Content("B".to_string()),
                ChatEvent::Reasoning("2".to_string()),
                ChatEvent::Content("C".to_string()),
            ]
        );
    }
}
