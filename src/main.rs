use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::env;

#[derive(Deserialize)]
struct ChatGptResponse {
    choices: Vec<ChatGptChoice>,
}

#[derive(Deserialize)]
struct ChatGptChoice {
    message: ChatGptMessage,
}

#[derive(Serialize)]
struct ChatGptRequest<'a> {
    model: String,
    messages: &'a [ChatGptMessage],
}

#[derive(Deserialize, Serialize)]
struct ChatGptMessage {
    role: String,
    content: String,
}

async fn get_chatgpt_response(
    api_key: &str,
    model: &str,
    messages: &[ChatGptMessage]
) -> Result<ChatGptResponse, Box<dyn Error>> {
    let client = reqwest::Client::new();

    let payload = ChatGptRequest {
        model: model.to_string(),
        messages,
    };

    let response: ChatGptResponse = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?
        .json()
        .await?;

    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let api_key = env::var("OPENAI_API_KEY")?;
    let model = "gpt-3.5-turbo";

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
                messages.push(ChatGptMessage {
                    role: "user".to_string(),
                    content,
                });

                let response = get_chatgpt_response(&api_key, model, &messages);
                let reply = &response.await?.choices[0].message.content;

                println!("{}", reply);

                messages.push(ChatGptMessage {
                    role: "assistant".to_string(),
                    content: reply.to_string(),
                });
            }
            Signal::CtrlD | Signal::CtrlC => {
                break;
            }
        }
    }
    Ok(())
}
