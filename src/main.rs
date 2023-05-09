use clap::Parser;
use reedline::{DefaultPrompt, DefaultPromptSegment::Empty, Reedline, Signal};
use serde::{Deserialize, Serialize};
use serde_jsonlines::JsonLinesWriter;
use spinners::{Spinner, Spinners};
use std::env;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
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

trait ChatMessages {
    fn messages(&self) -> &[ChatGptMessage];
    fn push(&mut self, message: ChatGptMessage) -> Result<(), Box<dyn Error>>;
}

struct TransientChatMessages(Vec<ChatGptMessage>);

impl ChatMessages for TransientChatMessages {
    fn messages(&self) -> &[ChatGptMessage] {
        &self.0
    }
    fn push(&mut self, message: ChatGptMessage) -> Result<(), Box<dyn Error>> {
        self.0.push(message);
        Ok(())
    }
}

struct DurableChatMessages {
    messages: Vec<ChatGptMessage>,
    writer: JsonLinesWriter<File>,
}

fn session_writer(filename: &str) -> io::Result<JsonLinesWriter<File>> {
    let file = File::options()
        .write(true)
        .append(true)
        .create(true)
        .open(filename)?;
    Ok(JsonLinesWriter::new(file))
}

impl DurableChatMessages {
    fn new(filename: &str) -> io::Result<DurableChatMessages> {
        Ok(DurableChatMessages {
            messages: Vec::new(),
            writer: session_writer(filename)?,
        })
    }
}

impl ChatMessages for DurableChatMessages {
    fn messages(&self) -> &[ChatGptMessage] {
        &self.messages
    }
    fn push(&mut self, message: ChatGptMessage) -> Result<(), Box<dyn Error>> {
        self.writer.write(&message)?;
        self.writer.flush()?;
        self.messages.push(message);
        Ok(())
    }
}

fn termimad_skin() -> MadSkin {
    let mut skin = MadSkin::default_dark();
    skin.paragraph.set_fg(Color::AnsiValue(249));
    skin
}

#[tokio::main]
async fn main_loop<T: ChatMessages>(
    api_key: &str,
    model: &str,
    mut messages: T,
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
                    get_chatgpt_response(api_key, model, &messages.messages());

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

    match args.session {
        Some(filename) => {
            let mesgs = DurableChatMessages::new(&filename)
                .expect("Unable to open session file");
            main_loop(&api_key, &args.model, mesgs)
        }
        None => {
            let mesgs = TransientChatMessages(Vec::new());
            main_loop(&api_key, &args.model, mesgs)
        }
    }
}
