use clap::Parser;
use reedline::{DefaultPrompt, DefaultPromptSegment::Empty, Reedline, Signal};
use serde::{Deserialize, Serialize};
use serde_jsonlines::{json_lines, JsonLinesWriter};
use spinners::{Spinner, Spinners};
use std::env;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::path::Path;
use termimad::crossterm::style::Color;
use termimad::MadSkin;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    Assistant,
    System,
    User,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::User => "user",
        })
    }
}

#[derive(Serialize)]
struct ChatGptRequest<'a> {
    model: &'a str,
    messages: &'a [ChatGptMessage],
}

#[derive(Deserialize)]
struct ChatGptResponse {
    choices: Vec<ChatGptChoice>,
}

#[derive(Deserialize)]
struct ChatGptChoice {
    message: ChatGptMessage,
}

#[derive(Deserialize, Serialize)]
struct ChatGptMessage {
    role: Role,
    content: String,
}

async fn get_chatgpt_response(
    api_key: &str,
    model: &str,
    messages: &[ChatGptMessage],
) -> Result<ChatGptResponse, Box<dyn Error>> {
    let client = reqwest::Client::new();

    let response: ChatGptResponse = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&ChatGptRequest { model, messages })
        .send()
        .await?
        .json()
        .await?;

    Ok(response)
}

trait ChatMessageListener {
    fn on_message(&mut self, message: &ChatGptMessage) -> Result<(), Box<dyn Error>>;
}

struct ChatMessages<'a> {
    messages: Vec<ChatGptMessage>,
    listeners: Vec<Box<dyn ChatMessageListener + 'a>>,
}

fn read_session_messages(filename: &str) -> io::Result<Vec<ChatGptMessage>> {
    let path = Path::new(filename);
    if path.try_exists()? {
        json_lines::<ChatGptMessage, _>(path)?.collect::<io::Result<Vec<_>>>()
    } else {
        Ok(Vec::new())
    }
}

impl<'a> ChatMessages<'a> {
    fn new() -> ChatMessages<'a> {
        ChatMessages {
            messages: Vec::new(),
            listeners: Vec::new(),
        }
    }

    fn from_file(filename: &str) -> io::Result<ChatMessages<'a>> {
        Ok(ChatMessages {
            messages: read_session_messages(filename)?,
            listeners: Vec::new(),
        })
    }

    fn register<L: ChatMessageListener + 'a>(&mut self, listener: L) {
        self.listeners.push(Box::new(listener));
    }

    fn push(&mut self, message: ChatGptMessage) -> Result<(), Box<dyn Error>> {
        for listener in self.listeners.iter_mut() {
            listener.on_message(&message)?;
        }
        self.messages.push(message);
        Ok(())
    }
}

struct SessionAppendListener {
    writer: JsonLinesWriter<File>,
}

impl SessionAppendListener {
    fn new(filename: &str) -> io::Result<SessionAppendListener> {
        let file = File::options()
            .write(true)
            .append(true)
            .create(true)
            .open(filename)?;
        Ok(SessionAppendListener {
            writer: JsonLinesWriter::new(file)
        })
    }
}

impl ChatMessageListener for SessionAppendListener {
    fn on_message(&mut self, message: &ChatGptMessage) -> Result<(), Box<dyn Error>> {
        self.writer.write(&message)?;
        self.writer.flush()?;
        Ok(())
    }
}

fn termimad_skin() -> MadSkin {
    let mut skin = MadSkin::default_dark();
    skin.paragraph.set_fg(Color::AnsiValue(249));
    skin
}

#[tokio::main]
async fn main_loop(
    api_key: &str,
    model: &str,
    messages: &mut ChatMessages,
) -> Result<(), Box<dyn Error>> {
    let mut line_editor = Reedline::create();
    let prompt = DefaultPrompt::new(Empty, Empty);

    let term_skin = termimad_skin();

    loop {
        let sig = line_editor.read_line(&prompt)?;
        match sig {
            Signal::Success(content) => {
                messages.push(ChatGptMessage {
                    role: Role::User,
                    content,
                })?;

                let mut spinner = Spinner::new(Spinners::Dots2, String::new());

                let resp =
                    get_chatgpt_response(api_key, model, &messages.messages);

                let mesg = resp.await?.choices.pop().unwrap().message;

                spinner.stop_with_message(format!(
                    "{}",
                    term_skin.term_text(&mesg.content)
                ));
                messages.push(mesg)?;
            }
            Signal::CtrlD | Signal::CtrlC => {
                break;
            }
        }
    }
    Ok(())
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// OpenAI model to use
    #[arg(short, long, default_value = "gpt-3.5-turbo")]
    model: String,

    /// OpenAI API Key [default: $OPENAI_API_KEY]
    #[arg(long)]
    api_key: Option<String>,

    /// Persist session to a JSONL file
    #[arg(short, long, value_name = "FILE")]
    session: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let api_key = args
        .api_key
        .or(env::var("OPENAI_API_KEY").ok())
        .expect("OpenAI API key not set");

    let mut messages = match args.session {
        Some(filename) => {
            let mut messages = ChatMessages::from_file(&filename)
                .expect("could not read session file");
            let appender = SessionAppendListener::new(&filename)
                .expect("count not open session file for writing");
            messages.register(appender);
            messages
        }
        None => ChatMessages::new(),
    };

    main_loop(&api_key, &args.model, &mut messages)
}
