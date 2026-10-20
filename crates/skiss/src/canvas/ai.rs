//! AI help through Ailloy: rewriting a card (spelling, clarity, translation) and pictures.
//!
//! Requests run in the background; the board keeps working and the card shimmers until the
//! answer arrives. The result is applied as one ordinary, undoable edit.

use std::sync::mpsc::{Receiver, channel};

use ailloy::types::{ImageFormat, Message};

/// What a rewrite should do with the card.
#[derive(Clone, Debug)]
pub enum Rewrite {
    Spelling,
    Clearer,
    Shorter,
    Translate(String),
}

impl Rewrite {
    pub fn label(&self) -> String {
        match self {
            Rewrite::Spelling => "Fixing spelling".into(),
            Rewrite::Clearer => "Making it clearer".into(),
            Rewrite::Shorter => "Shortening".into(),
            Rewrite::Translate(l) => format!("Translating to {l}"),
        }
    }

    fn instruction(&self) -> String {
        match self {
            Rewrite::Spelling => "Fix spelling, grammar and punctuation. Keep the language, the wording, the tone and the length; change as little as possible.".into(),
            Rewrite::Clearer => "Rewrite to be clearer and easier to scan. Keep the language, every fact, and roughly the length.".into(),
            Rewrite::Shorter => "Make it noticeably shorter while keeping every important fact. Keep the language.".into(),
            Rewrite::Translate(l) => format!("Translate into {l}. Keep names, numbers and dates as they are."),
        }
    }
}

pub enum Outcome {
    /// Rewritten title (if the card has one) and body, as Markdown.
    Text {
        title: Option<String>,
        body: String,
    },
    Image {
        bytes: Vec<u8>,
        ext: &'static str,
    },
}

/// A running request.
pub struct Job {
    /// Where the card starts in the file (to find it again when the answer arrives).
    pub card_start: usize,
    pub card_id: String,
    pub label: String,
    /// For pictures: the prompt, which becomes the alt text; and the image line to replace.
    pub prompt: Option<String>,
    pub replace: Option<std::ops::Range<usize>>,
    rx: Receiver<Result<Outcome, String>>,
}

impl Job {
    /// The answer, once it has arrived.
    pub fn poll(&self) -> Option<Result<Outcome, String>> {
        self.rx.try_recv().ok()
    }
}

/// Whether AI is set up and enabled for skiss (`skiss ai config`, `skiss ai enable`).
pub fn available() -> bool {
    ailloy::config_tui::is_ai_active("skiss")
}

fn spawn<F>(ctx: &egui::Context, fut: F) -> Receiver<Result<Outcome, String>>
where
    F: std::future::Future<Output = Result<Outcome, String>> + Send + 'static,
{
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    let run = async move {
        let out = fut.await;
        let _ = tx.send(out);
        ctx.request_repaint();
    };
    match tokio::runtime::Handle::try_current() {
        Ok(h) => {
            h.spawn(run);
        }
        Err(_) => {
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Runtime::new() {
                    rt.block_on(run);
                }
            });
        }
    }
    rx
}

const SYSTEM: &str = "You edit one card of a person's notes. The card is Markdown. \
Keep Markdown structure exactly: lists stay lists, task boxes ([ ] and [x]) stay as they are, \
links keep their targets, bold and italic stay where they belong. \
Answer with the revised card only: no preamble, no explanation, no code fences.";

pub fn rewrite(
    ctx: &egui::Context,
    how: Rewrite,
    card_start: usize,
    card_id: &str,
    title: Option<String>,
    body: String,
) -> Job {
    let label = how.label();
    let has_title = title.is_some();
    let card = match &title {
        Some(t) => format!("# {t}\n\n{body}"),
        None => body.clone(),
    };
    let ask = format!(
        "{}\n\n{}\n\n---\n{card}",
        how.instruction(),
        if has_title {
            "The first line is the card's title (\"# Title\"); keep it as the first line in the same form."
        } else {
            "The card has no title."
        }
    );
    let rx = spawn(ctx, async move {
        let client = ailloy::Client::for_capability("chat").map_err(|e| e.to_string())?;
        let reply = client
            .chat(&[Message::system(SYSTEM), Message::user(ask)])
            .await
            .map_err(|e| e.to_string())?;
        Ok(parse_reply(&reply.content, has_title))
    });
    Job {
        card_start,
        card_id: card_id.to_string(),
        label,
        prompt: None,
        replace: None,
        rx,
    }
}

/// Splits a model answer into title and body, forgiving code fences around it.
fn parse_reply(reply: &str, has_title: bool) -> Outcome {
    let mut text = reply.trim();
    if let Some(rest) = text.strip_prefix("```") {
        let rest = rest.split_once('\n').map_or("", |(_, r)| r);
        text = rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    if has_title
        && let Some(first) = text.lines().next()
        && first.trim_start().starts_with('#')
    {
        let title = first
            .trim_start()
            .trim_start_matches('#')
            .trim()
            .to_string();
        let body = text[first.len()..].trim().to_string();
        return Outcome::Text {
            title: Some(title),
            body,
        };
    }
    Outcome::Text {
        title: None,
        body: text.to_string(),
    }
}

pub fn picture(
    ctx: &egui::Context,
    prompt: String,
    card_start: usize,
    card_id: &str,
    replace: Option<std::ops::Range<usize>>,
) -> Job {
    let p = prompt.clone();
    let rx = spawn(ctx, async move {
        let client = ailloy::Client::for_capability("image").map_err(|e| e.to_string())?;
        let img = client.generate_image(&p).await.map_err(|e| e.to_string())?;
        let ext = match img.format {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Webp => "webp",
        };
        Ok(Outcome::Image {
            bytes: img.data,
            ext,
        })
    });
    Job {
        card_start,
        card_id: card_id.to_string(),
        label: "Drawing a picture".into(),
        prompt: Some(prompt),
        replace,
        rx,
    }
}

/// Saves generated picture bytes next to the board and returns the relative path.
pub fn save_picture(
    assets: &std::path::Path,
    doc_dir: &std::path::Path,
    bytes: &[u8],
    ext: &str,
) -> anyhow::Result<String> {
    std::fs::create_dir_all(assets)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let mut name = format!("picture-{stamp}.{ext}");
    let mut n = 2;
    while assets.join(&name).exists() {
        name = format!("picture-{stamp}-{n}.{ext}");
        n += 1;
    }
    let path = assets.join(&name);
    std::fs::write(&path, bytes)?;
    let rel = path.strip_prefix(doc_dir).unwrap_or(&path);
    Ok(rel.to_string_lossy().replace('\\', "/").replace(' ', "%20"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_split_into_title_and_body() {
        let Outcome::Text { title, body } =
            parse_reply("# Anna Berg\n\nProgram sponsor.\n- [ ] Call", true)
        else {
            panic!()
        };
        assert_eq!(title.as_deref(), Some("Anna Berg"));
        assert_eq!(body, "Program sponsor.\n- [ ] Call");
        let Outcome::Text { title, body } = parse_reply("```markdown\nJust text\n```", false)
        else {
            panic!()
        };
        assert!(title.is_none());
        assert_eq!(body, "Just text");
    }
}
