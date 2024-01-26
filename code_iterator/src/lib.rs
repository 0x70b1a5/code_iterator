use std::collections::HashMap;
use std::str::FromStr;

use kinode_process_lib::{
    await_message, call_init, get_blob, http::{
        bind_http_path, bind_ws_path, send_response, send_ws_push, serve_ui, HttpServerRequest,
        IncomingHttpRequest, StatusCode, WsMessageType,
    }, kernel_types::PythonRequest, println, Address, LazyLoadBlob, Message, ProcessId, Request, Response
    
};
use serde::{Deserialize, Serialize};

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
    Error(String),
    RunResult(String),
    LLMResponse(String),
}


fn handle_message(our: &Address, our_channel_id: &mut i32) -> anyhow::Result<()> {
    let message = await_message()?;

    match message {
        Message::Response { ref source, ref body, .. } =>
            handle_response(source, body)?,
        Message::Request { ref source, ref body, .. } =>
            handle_request(&our, source, body)?,
    }

    Ok(())
}

fn handle_response(source: &Address, body: &Vec<u8>) -> anyhow::Result<()> {
    let iter_resp = serde_json::from_slice::<IteratorResponse>(body)?;
    match iter_resp {
        IteratorResponse::Ok => {}
        IteratorResponse::Error(e) => {
            println!("code_iterator: error: {:?}", e);
        }
        IteratorResponse::RunResult(result) => {
            println!("code_iterator: run result: {:?}", result);
        }
        IteratorResponse::LLMResponse(llm_response) => {
            println!("code_iterator: llm response: {:?}", llm_response);
        }
    }

    Ok(())
}

fn handle_request(our: &Address, source: &Address, body: &Vec<u8>) -> anyhow::Result<()> {
    let iter_req = serde_json::from_slice::<IteratorRequest>(body)?;
    match iter_req {
        IteratorRequest::UserPrompt(prompt) => {
            println!("code_iterator: user prompt: {:?}", prompt);
        }
        IteratorRequest::UserRunCode(code) => {
            println!("code_iterator: user run code: {:?}", code);

            Request::new()
                .target((&our.node, "python", "distro", "sys"))
                .body(serde_json::to_vec(&PythonRequest::Run)?)
                .blob(LazyLoadBlob{
                    mime: Some("text/plain".to_string()),
                    bytes: code.as_bytes().to_vec()
                })
                .send_and_await_response(5)?;
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
