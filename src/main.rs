use clap::Parser;
use reedline::{DefaultPrompt, DefaultPromptSegment::Empty, Reedline, Signal};
use serde::{Deserialize, Serialize};
use spinners::{Spinner, Spinners};
use std::error::Error;
use std::env;
use termimad::MadSkin;
use termimad::crossterm::style::Color;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Role { Assistant, User }

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
    messages: &[ChatGptMessage]
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

fn termimad_skin() -> MadSkin {
    let mut skin = MadSkin::default_dark();
    skin.paragraph.set_fg(Color::AnsiValue(249));
    skin
}

#[tokio::main]
async fn main_loop(api_key: &str, model: &str) -> Result<(), Box<dyn Error>> {
    let mut line_editor = Reedline::create();
    let prompt = DefaultPrompt::new(Empty, Empty);

    let term_skin = termimad_skin();

    let mut messages: Vec<ChatGptMessage> = Vec::new();

    loop {
        let sig = line_editor.read_line(&prompt)?;
        match sig {
            Signal::Success(content) => {
                messages.push(ChatGptMessage {role: Role::User, content});

                let mut spinner = Spinner::new(Spinners::Dots2, String::new());

                let resp = get_chatgpt_response(&api_key, model, &messages);
                let mesg = resp.await?.choices.pop().unwrap().message;

                spinner.stop_with_message(
                    format!("{}", term_skin.term_text(&mesg.content))
                );
                messages.push(mesg);
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
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let api_key = args.api_key
        .or(env::var("OPENAI_API_KEY").ok())
        .expect("OpenAI API key not set");

    main_loop(&api_key, &args.model)
}
