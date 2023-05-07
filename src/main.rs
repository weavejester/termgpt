use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};
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
async fn main() -> Result<(), Box<dyn Error>> {
    let api_key = env::var("OPENAI_API_KEY")?;
    let model = "gpt-3.5-turbo";
    let skin = termimad_skin();

    let mut line_editor = Reedline::create();
    let prompt = DefaultPrompt::new(
        DefaultPromptSegment::Empty,
        DefaultPromptSegment::Empty,
    );

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
                    format!("{}", skin.term_text(&mesg.content))
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
