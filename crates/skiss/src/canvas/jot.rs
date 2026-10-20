//! Jot: write a note anywhere, and Skiss puts it where it belongs.
//!
//! Press `.` and a large editor opens over the board. Enter places the note: an AI judge (a
//! TypeSafe model such as Jev when Ailloy has an eval node, else the chat model) picks the area,
//! decides whether a to-do joins an existing checklist, and a chat model shapes the note like
//! its new neighbors. A note that starts with `/` is a request for AI to write the note (or
//! `/picture …` to draw one) first. Without AI, notes land in an Inbox area.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};

use ailloy::types::{ImageFormat, Message};
use ailloy::{Answer, Question};
use skiss_core::board::place::Spot;

/// The id the jot's draft card goes by while it is being written (never a real card id).
pub const JOT_ID: &str = "\u{1}jot";

/// How sure the judge must be before a note is placed without asking.
const SURE: f64 = 0.6;

/// Where a placed note goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Area(String),
    /// Added as a to-do to this checklist card.
    Checklist(String),
    /// A new area with this title, at the end of the board.
    NewArea(String),
}

/// One of the choices offered when the judge is unsure.
#[derive(Clone, Debug)]
pub struct Choice {
    pub target: Target,
    pub label: String,
    pub probability: f64,
}

pub enum Outcome {
    /// Put this Markdown (a card, or a to-do's text) there.
    Place { target: Target, markdown: String },
    /// Not sure: let the writer pick (the note itself stays in the editor).
    Choose(Vec<Choice>),
    /// AI wrote a draft (Markdown) for the writer to look over.
    Draft(String),
    Picture {
        bytes: Vec<u8>,
        ext: &'static str,
        prompt: String,
    },
}

pub struct Job {
    pub label: String,
    rx: Receiver<Result<Outcome, String>>,
}

impl Job {
    pub fn poll(&self) -> Option<Result<Outcome, String>> {
        self.rx.try_recv().ok()
    }
}

/// What the board looks like, gathered before the request leaves the UI thread.
#[derive(Clone, Default)]
pub struct BoardView {
    pub title: String,
    pub spots: Vec<Spot>,
    /// Area id -> Markdown of its latest cards.
    pub examples: HashMap<String, Vec<String>>,
    /// Area id -> (card id, title) of its checklists.
    pub checklists: HashMap<String, Vec<(String, String)>>,
    pub today: String,
    /// The whole board as Markdown (without the layout appendix), for requests about it.
    pub document: String,
}

/// How much of the board a request may read (characters); bigger boards are cut at the end.
const MAX_DOCUMENT: usize = 200_000;

fn spawn<F>(ctx: &egui::Context, label: &str, fut: F) -> Job
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
    Job {
        label: label.to_string(),
        rx,
    }
}

/// Finds the place for a note (`markdown` is a card: `# Title` and body). With `area` given,
/// the area is already chosen and only the rest is decided.
pub fn place(ctx: &egui::Context, markdown: String, board: BoardView, area: Option<Target>) -> Job {
    spawn(ctx, "Finding its place", async move {
        place_async(markdown, board, area)
            .await
            .map_err(|e| e.to_string())
    })
}

async fn place_async(
    markdown: String,
    board: BoardView,
    area: Option<Target>,
) -> anyhow::Result<Outcome> {
    let judge = ailloy::Client::for_capability("eval")?;
    let target = match area {
        Some(t) => t,
        None if board.spots.is_empty() => Target::NewArea(new_area_name(&markdown, &board).await?),
        None => {
            let mut q = Question::choice(format!(
                "A note is being added to a notes board titled \"{}\". Which section does it \
                 belong in? Pick the section a person would look in to find it again. Weigh \
                 what kind of note it is as much as its topic: a record of something that \
                 happened (a diary entry, a meeting, a visit) belongs with similar dated \
                 entries, a task with tasks, an idea with ideas, and how-to knowledge with \
                 other reference notes.",
                board.title
            ));
            for (i, s) in board.spots.iter().enumerate().take(250) {
                q = q.option(format!("s{i}"), s.describe());
            }
            q = q.option(
                "new",
                "None of these: it is about something the board has no section for yet",
            );
            let ranked = ranked(judge.eval_one(markdown.clone(), q).await?);
            let Some((best, p)) = ranked.first().cloned() else {
                anyhow::bail!("the judge gave no answer");
            };
            if p < SURE {
                let mut choices = Vec::new();
                for (key, p) in ranked.iter().take(3) {
                    choices.push(match key.as_str() {
                        "new" => Choice {
                            target: Target::NewArea(String::new()),
                            label: "A new area".into(),
                            probability: *p,
                        },
                        k => {
                            let s = spot(&board, k)?;
                            Choice {
                                target: Target::Area(s.id.clone()),
                                label: s.path.clone(),
                                probability: *p,
                            }
                        }
                    });
                }
                return Ok(Outcome::Choose(choices));
            }
            match best.as_str() {
                "new" => Target::NewArea(String::new()),
                k => Target::Area(spot(&board, k)?.id.clone()),
            }
        }
    };
    let target = match target {
        Target::NewArea(name) if name.is_empty() => {
            Target::NewArea(new_area_name(&markdown, &board).await?)
        }
        t => t,
    };

    // A short to-do may belong in a checklist that is already there.
    if let Target::Area(id) = &target
        && let Some(lists) = board.checklists.get(id)
        && !lists.is_empty()
        && is_short(&markdown)
    {
        let mut q = Question::choice(
            "Is this note a to-do that belongs in one of these existing checklists, or a note \
             of its own?",
        );
        for (i, (_, title)) in lists.iter().enumerate().take(50) {
            q = q.option(
                format!("c{i}"),
                format!("A task in the checklist \"{title}\""),
            );
        }
        q = q.option("own", "A note of its own, not a task in a checklist");
        let ranked = ranked(judge.eval_one(markdown.clone(), q).await?);
        if let Some((key, p)) = ranked.first()
            && *p >= SURE
            && let Some(i) = key.strip_prefix('c').and_then(|n| n.parse::<usize>().ok())
            && let Some((card, _)) = lists.get(i)
        {
            let text = markdown
                .lines()
                .next()
                .unwrap_or("")
                .trim_start_matches('#')
                .trim()
                .to_string();
            return Ok(Outcome::Place {
                target: Target::Checklist(card.clone()),
                markdown: text,
            });
        }
    }

    // Shaped like its neighbors.
    let examples = match &target {
        Target::Area(id) => board.examples.get(id).cloned().unwrap_or_default(),
        _ => Vec::new(),
    };
    let markdown = if examples.is_empty() {
        markdown
    } else {
        reshape(&markdown, &examples, &board.today)
            .await
            .unwrap_or(markdown)
    };
    Ok(Outcome::Place { target, markdown })
}

fn spot<'a>(board: &'a BoardView, key: &str) -> anyhow::Result<&'a Spot> {
    key.strip_prefix('s')
        .and_then(|n| n.parse::<usize>().ok())
        .and_then(|i| board.spots.get(i))
        .ok_or_else(|| anyhow::anyhow!("the judge picked an unknown section"))
}

/// Options with their probabilities, most likely first.
fn ranked(answer: Answer) -> Vec<(String, f64)> {
    let Answer::Choice {
        choice,
        probabilities,
        ..
    } = answer
    else {
        return Vec::new();
    };
    let mut v: Vec<(String, f64)> = probabilities.into_iter().collect();
    if v.is_empty() {
        v.push((choice, 1.0));
    }
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
}

fn is_short(markdown: &str) -> bool {
    let lines: Vec<&str> = markdown.lines().filter(|l| !l.trim().is_empty()).collect();
    lines.len() == 1 && lines[0].chars().count() <= 140
}

fn chat_text(reply: &str) -> String {
    let mut text = reply.trim();
    if let Some(rest) = text.strip_prefix("```") {
        let rest = rest.split_once('\n').map_or("", |(_, r)| r);
        text = rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    text.to_string()
}

async fn chat(system: &str, user: String) -> anyhow::Result<String> {
    let client = ailloy::Client::for_capability("chat")?;
    let reply = client
        .chat(&[Message::system(system), Message::user(user)])
        .await?;
    Ok(chat_text(&reply.content))
}

async fn new_area_name(markdown: &str, board: &BoardView) -> anyhow::Result<String> {
    let existing: Vec<&str> = board.spots.iter().map(|s| s.path.as_str()).collect();
    let name = chat(
        "You name sections of a person's notes. Answer with the name only: one to three words, \
         in the same language and style as the existing section names. No quotes, no punctuation.",
        format!(
            "Existing sections: {}\n\nName a new section for this note:\n\n{markdown}",
            if existing.is_empty() {
                "(none yet)".to_string()
            } else {
                existing.join(", ")
            }
        ),
    )
    .await?;
    let name = name
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(['"', '.', '#', ' ']);
    Ok(if name.is_empty() {
        skiss_core::board::place::INBOX.to_string()
    } else {
        name.to_string()
    })
}

/// Rewrites a note in the format of the notes next to it, keeping its content.
async fn reshape(markdown: &str, examples: &[String], today: &str) -> anyhow::Result<String> {
    let out = chat(
        "You file a new note among a person's existing notes. Rewrite the NEW NOTE so it looks \
         like one more of the EXISTING NOTES: the same kind of title (for example a date first, \
         if theirs have one), the same structure, sub-headings and list style, the same \
         language. Keep everything the new note says and keep its wording where you can. Never \
         invent facts, names or tasks; leave out parts of the format the note has nothing for. \
         Keep Markdown formatting, links, images and tables. Answer with the rewritten note \
         only, starting with a heading line `# Title`.",
        format!(
            "Today is {today}.\n\nEXISTING NOTES:\n\n{}\n\nNEW NOTE:\n\n{markdown}",
            examples.join("\n\n---\n\n")
        ),
    )
    .await?;
    if out.trim_start().starts_with('#') {
        Ok(out)
    } else {
        anyhow::bail!("the rewrite did not start with a title")
    }
}

/// Asks AI to write a note from a request (the text after `/`). The whole board goes along,
/// so a request can be about it ("summarize this board in two paragraphs").
pub fn draft(ctx: &egui::Context, request: String, board: BoardView) -> Job {
    spawn(ctx, "Writing", async move {
        let mut document = board.document.clone();
        if document.len() > MAX_DOCUMENT {
            let mut cut = MAX_DOCUMENT;
            while !document.is_char_boundary(cut) {
                cut -= 1;
            }
            document.truncate(cut);
            document.push_str("\n\n(the rest of the board is left out)");
        }
        chat(
            "You write one note for a person's notes board, in Markdown. The board itself is \
             given below: use it whenever the request is about the board, its cards or its \
             contents, and stay true to it. Start with a heading line `# Title`, then the \
             content. Use lists, tables, bold and links where they help, and keep it as long \
             as the request asks for. Answer with the note only.",
            format!(
                "Today is {}.\n\nTHE BOARD (\"{}\"):\n\n{document}\n\nREQUEST: {request}",
                board.today, board.title
            ),
        )
        .await
        .map(Outcome::Draft)
        .map_err(|e| e.to_string())
    })
}

/// Draws a picture for the note.
pub fn picture(ctx: &egui::Context, prompt: String) -> Job {
    spawn(ctx, "Drawing", async move {
        let client = ailloy::Client::for_capability("image").map_err(|e| e.to_string())?;
        let img = client
            .generate_image(&prompt)
            .await
            .map_err(|e| e.to_string())?;
        let ext = match img.format {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Webp => "webp",
        };
        Ok(Outcome::Picture {
            bytes: img.data,
            ext,
            prompt,
        })
    })
}

/// What a request typed into the jot asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Write(String),
    Picture(String),
}

/// A jot whose first line starts with `/` is a request to AI.
pub fn request(first_line: &str) -> Option<Request> {
    let rest = first_line.trim().strip_prefix('/')?.trim();
    if rest.is_empty() {
        return None;
    }
    for word in ["picture", "image", "draw"] {
        if let Some(p) = rest.strip_prefix(word)
            && (p.is_empty() || p.starts_with(' '))
        {
            return Some(Request::Picture(p.trim().to_string()));
        }
    }
    Some(Request::Write(rest.to_string()))
}

/// Typed table rows become a real table: consecutive `|` lines stay together, and a header
/// separator is added after the first row when there is none.
pub fn tidy_tables(md: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let lines: Vec<&str> = md.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i].trim();
        if l.starts_with('|') {
            let mut rows = vec![l.to_string()];
            let mut j = i + 1;
            while j < lines.len() {
                let t = lines[j].trim();
                if t.is_empty() && lines.get(j + 1).is_some_and(|n| n.trim().starts_with('|')) {
                    j += 1;
                    continue;
                }
                if !t.starts_with('|') {
                    break;
                }
                rows.push(t.to_string());
                j += 1;
            }
            let is_rule = |r: &str| r.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '));
            if rows.len() >= 2 && !rows.iter().any(|r| is_rule(r)) {
                let cols = rows[0].trim_matches('|').split('|').count().max(1);
                rows.insert(1, format!("|{}", " --- |".repeat(cols)));
            }
            out.extend(rows);
            i = j;
        } else {
            out.push(lines[i].to_string());
            i += 1;
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_start_with_a_slash() {
        assert_eq!(request("Lunch with Maria"), None);
        assert_eq!(request("/"), None);
        assert_eq!(
            request("/summarize the lander risks"),
            Some(Request::Write("summarize the lander risks".into()))
        );
        assert_eq!(
            request("/picture a red barn in snow"),
            Some(Request::Picture("a red barn in snow".into()))
        );
        assert_eq!(
            request("/pictures of the week"),
            Some(Request::Write("pictures of the week".into()))
        );
    }

    #[test]
    fn typed_tables_become_tables() {
        assert_eq!(
            tidy_tables("# Prices\n\n| Size | Price |\n\n| Small | 10 |\n\nAfter."),
            "# Prices\n\n| Size | Price |\n| --- | --- |\n| Small | 10 |\n\nAfter."
        );
        let done = "| a | b |\n| --- | --- |\n| 1 | 2 |";
        assert_eq!(tidy_tables(done), done);
    }

    #[test]
    fn short_notes_can_be_tasks() {
        assert!(is_short("# Call the framer"));
        assert!(!is_short("# Call the framer\n\nAbout the oak frames."));
    }
}
