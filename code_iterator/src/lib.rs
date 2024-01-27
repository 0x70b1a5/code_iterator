use std::collections::HashMap;
use std::str::FromStr;

use kinode_process_lib::{
    await_message, call_init, get_blob, http::{
        bind_http_path, bind_ws_path, send_request, send_response, send_ws_push, serve_ui, HttpServerRequest, IncomingHttpRequest, Method, StatusCode, WsMessageType, send_request_await_response
    }, kernel_types::PythonRequest, println, Address, LazyLoadBlob, Message, ProcessId, Request, Response,
    
};
use url::Url;
use serde::{Deserialize, Serialize};
use serde_json::json;


wit_bindgen::generate!({
    path: "wit",
    world: "process",
    exports: {
        world: Component,
    },
});

/*
    This is an app for iterating Python code using an LLM.
    It serves a frontend with a prompt box and displays the generated code.
    It also enables the user to run the code, which is untrusted, in a sandboxed Wasm environment.

    Enable an AI-assisted workflow for developing vanilla python code that looks like

        1. Prompt LLM, e.g., "write a program that computes the fibonacci sequence",
        2. LLM generates python code,
        3. Run python code,
        4. Take result and iterate python code until it is correct.

*/


#[derive(Debug, Serialize, Deserialize)]
enum IteratorRequest {
    UserPrompt(String),
    UserRunCode(String),
    LLMPrompt(String),
}

#[derive(Debug, Serialize, Deserialize)]
enum IteratorResponse {
    Ok,
    Run,
    Error(String),
    LLMResponse(String),
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAiChatMessage {
    pub role: String,
    pub content: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAiChatCompletionChoice {
    pub message: OpenAiChatMessage,
    pub index: u32,
    pub logprobs: Option<serde_json::Value>,
    pub finish_reason: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAiChatCompletion {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<OpenAiChatCompletionChoice>,
}

fn fetch_openai_response(prompt: &String, our: &Address, our_channel_id: &u32) -> anyhow::Result<()> {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("Authorization".to_string(), "Bearer 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".to_string());
    let body = serde_json::to_vec(&json!({
        "model": "gpt-3.5-turbo",
        "messages": [
            {
                "role": "system",
                "content": "You are a helpful assistant who responds in only Python code. Produce code in response to user input. MAKE SURE you include the code inside a Markdown code block, e.g. ```python print('hello world') ```"
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
    }))?;

    send_request_await_response(
        Method::POST,
        Url::from_str("https://api.openai.com/v1/chat/completions")?,
        Some(headers),
        10,
        body,
    )?;

    let Some(body) = get_blob() else {
        println!("code_iterator: no blob");
        return Ok(());
    };

    let comp = serde_json::from_slice::<OpenAiChatCompletion>(&body.bytes)?;
    println!("code_iterator: openai response: {:?}", comp.clone());
    
    send_ws_push(our.node.clone(), 
        *our_channel_id,
        WsMessageType::Text,  
        LazyLoadBlob {
        mime: Some("text/plain".to_string()),
        bytes: json!({
            "LLMResponse": comp.choices[0].message.content
        }).to_string().as_bytes().to_vec(),
    })?;

    Ok(())
}


fn handle_message(our: &Address, our_channel_id: &mut u32) -> anyhow::Result<()> {
    let message = await_message()?;
    let http_server_address = ProcessId::from_str("http_server:distro:sys").unwrap();
    match message {
        Message::Response { ref source, ref body, .. } =>
            handle_response(source, body, &our_channel_id)?,
        Message::Request { ref source, ref body, .. } => {
            if source.process == http_server_address {
                handle_http_request(&our, source, body, our_channel_id)?;
                return Ok(());
            }
            handle_request(&our, source, body, &our_channel_id)?
        }
    }

    Ok(())
}

fn handle_response(source: &Address, body: &Vec<u8>, our_channel_id: &u32) -> anyhow::Result<()> {
    println!("code_iterator: response: {:?}", String::from_utf8(body.clone())?);
    let iter_resp = serde_json::from_slice::<IteratorResponse>(body)?;
    match iter_resp {
        IteratorResponse::Ok => {}
        IteratorResponse::Error(e) => {
            println!("code_iterator: error: {:?}", e);
        }
        IteratorResponse::LLMResponse(llm_response) => {
            println!("code_iterator: llm response: {:?}", llm_response);
        }
        
        IteratorResponse::Run => {
            let Some(blob) = get_blob() else {
                println!("code_iterator: no blob");
                return Ok(());
            };
            // result is json
            let result = serde_json::from_slice::<serde_json::Value>(&blob.bytes)?;
            println!("code_iterator: run result: {:?}", result);
            send_ws_push(
                source.node.clone(), 
                *our_channel_id, 
                WsMessageType::Text, 
                LazyLoadBlob{
                    mime: Some("text/plain".to_string()),
                    bytes: json!({
                        "LLMRunResponse": result
                }).to_string().as_bytes().to_vec()
            })?;
        }
    }

    Ok(())
}


fn handle_http_request(
    our: &Address,
    source: &Address,
    body: &Vec<u8>,
    our_channel_id: &mut u32,
) -> anyhow::Result<()> {
    let http_request = serde_json::from_slice::<HttpServerRequest>(body)?;
    println!("code_iterator: http request: {:?}", http_request);

    match http_request {
        HttpServerRequest::Http(request) => {
            match request.method()?.as_str() {
                "GET" => {
                    println!("code_iterator: http GET request: {:?}", request);
                    // let mut headers = HashMap::new();
                    // headers.insert("Content-Type".to_string(), "application/json".to_string());

                    // let body = serde_json::to_vec(&TransferResponse::ListFiles(files))?;

                    // send_response(StatusCode::OK, Some(headers), body)?;
                }
                "POST" => {
                    println!("code_iterator: http POST request: {:?}", request);
                    let path = request.path()?;
                    match path.as_str() {
                        "/prompt" => {
                            let Some(body) = get_blob() else {
                                println!("code_iterator: no blob");
                                return Ok(());
                            };
                            let prompt = serde_json::from_slice::<String>(&body.bytes)?;
                            println!("code_iterator: prompt: {:?}", prompt);
                            send_response(
                                StatusCode::OK,
                                None,
                                vec![]
                            )?;
                            fetch_openai_response(&prompt, &our, our_channel_id)?;
                        }
                        "/run" => {
                            let Some(body) = get_blob() else {
                                println!("code_iterator: no blob");
                                return Ok(());
                            };
                            println!("code_iterator: run: {:?}", body);
                            let code = serde_json::from_slice::<String>(&body.bytes)?;
                            send_response(
                                StatusCode::OK,
                                None,
                                vec![]
                            )?;
                            handle_request(
                                our, 
                                source,
                                &serde_json::to_vec(&IteratorRequest::UserRunCode(code.clone()))?,
                                our_channel_id
                            )?;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        HttpServerRequest::WebSocketClose(_) => {}
        HttpServerRequest::WebSocketOpen { channel_id, .. } => {
            *our_channel_id = channel_id;
        }
        HttpServerRequest::WebSocketPush { message_type, .. } => {
            if message_type != WsMessageType::Binary {
                return Ok(());
            }
            let Some(blob) = get_blob() else {
                return Ok(());
            };
            // handle_request(our, source, &blob.bytes, our_channel_id)?
        }
    }
    Ok(())
}

fn handle_request(our: &Address, source: &Address, body: &Vec<u8>, our_channel_id: &u32) -> anyhow::Result<()> {
    println!("code_iterator: request: {:?}", body);
    let iter_req = serde_json::from_slice::<IteratorRequest>(body)?;
    match iter_req {
        IteratorRequest::UserPrompt(prompt) => {
            println!("code_iterator: user prompt: {:?}", prompt);
            fetch_openai_response(&prompt, &our, &our_channel_id)?;
        }
        IteratorRequest::UserRunCode(code) => {
            println!("code_iterator: user run code: {:?}", code);

            let resp = Request::new()
                .target((&our.node, "python", "distro", "sys"))
                .body(serde_json::to_vec(&PythonRequest::Run)?)
                .blob(LazyLoadBlob{
                    mime: Some("text/plain".to_string()),
                    bytes: code.as_bytes().to_vec()
                })
                .send_and_await_response(15)??;

            println!("code_iterator: python response: {:?}", resp);

            handle_response(&our, &resp.body().to_vec(), our_channel_id)?;
        }
        IteratorRequest::LLMPrompt(prompt) => {
            println!("code_iterator: llm prompt: {:?}", prompt);
        }
    }

    Ok(())
}

call_init!(init);

fn init(our: Address) {
    println!("code_iterator: begin");

    let mut channel_id = 0;

    // Bind UI files to routes; index.html is bound to "/"
    serve_ui(&our, "ui").unwrap();

    bind_http_path("/prompt", true, true).unwrap();
    bind_http_path("/run", true, true).unwrap();

    // Bind WebSocket path
    bind_ws_path("/", true, false).unwrap();

    loop {
        match handle_message(&our, &mut channel_id) {
            Ok(()) => {}
            Err(e) => {
                println!("code_iterator: error: {:?}", e);
            }
        };
    }
}
