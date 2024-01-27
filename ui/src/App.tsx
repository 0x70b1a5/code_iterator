import { useState, useEffect, useCallback, useRef } from "react";
import KinodeEncryptorApi from "@uqbar/client-encryptor-api";
import useChatStore from "./store/chat";
import classNames from "classnames";
import { marked } from "marked";

const BASE_URL = import.meta.env.BASE_URL;
if (window.our) window.our.process = BASE_URL?.replace("/", "");

const PROXY_TARGET = `${(import.meta.env.VITE_NODE_URL || "http://localhost:8080")}${BASE_URL}`;

// This env also has BASE_URL which should match the process + package name
const WEBSOCKET_URL = import.meta.env.DEV
  ? `${PROXY_TARGET.replace('http', 'ws')}`
  : undefined;

function App() {
  const { chatHistory, addMessage, set } = useChatStore();
  const [message, setMessage] = useState("");
  const [nodeConnected, setNodeConnected] = useState(true);
  const [api, setApi] = useState<KinodeEncryptorApi | undefined>();
  const [parsedChats, setParsedChats] = useState([]);
  const [codeInput, setCodeInput] = useState('# Generated code will appear here.');
  const [codeOutput, setCodeOutput] = useState('# Output will appear here.');
  const [awaitingResponse, setAwaitingResponse] = useState(false);
  const [awaitingRun, setAwaitingRun] = useState(false);
  const AI_NAME = 'AI';

  useEffect(() => {
    const parseChats = async () => {
      const parsed = await Promise.all(chatHistory.map(async (chat) => {
        const parsedContent = await marked(chat.content);
        return { ...chat, content: parsedContent };
      }));
      setParsedChats(parsed);
    };
    parseChats();
  }, [chatHistory]);

  useEffect(() => {
    // Connect to the Kinode via websocket
    console.log('WEBSOCKET URL', WEBSOCKET_URL)
    if (window.our?.node && window.our?.process) {
      const api = new KinodeEncryptorApi({
        uri: WEBSOCKET_URL,
        nodeId: window.our.node,
        processId: window.our.process,
        onOpen: (_event, _api) => {
          console.log("Connected to Kinode");
          // api.send({ data: "Hello World" });
        },
        onMessage: (json, _api) => {
          console.log('WEBSOCKET MESSAGE', json)
          try {
            const data = JSON.parse(json);
            console.log("WebSocket received message", data);
            const [messageType] = Object.keys(data);
            if (!messageType) return;

            if (messageType === "LLMResponse") {
              addMessage({ author: AI_NAME, content: data.LLMResponse});
              setAwaitingResponse(false);
              parseMessageForCode(data.LLMResponse);
              scrollToBottom();
            } else if (messageType === "LLMRunResponse") {
              setCodeOutput(data.LLMRunResponse);
              setAwaitingRun(false);
            } else if (messageType === "Error") {
              console.error("Error from Kinode", data.Error);
              setAwaitingResponse(false);
              setAwaitingRun(false);
            }
          } catch (error) {
            console.error("Error parsing WebSocket message", error);
          }
        },
      });

      setApi(api);
    } else {
      setNodeConnected(false);
    }
  }, []);
  
  const parseMessageForCode = (message: string) => {
    console.log('PARSING MESSAGE', message)
    // replace <pre> and </pre> tags with ```python\n and ``` respectively
    message = message.replace(/<pre.*?>/g, '```python\n');
    message = message.replace(/<\/pre>/g, '```');
    // remove <code>
    message = message.replace(/<\/?code.*?>/g, '');

    const codeRegex = /```python\n([\s\S]*?)```/gm;
    const match = codeRegex.exec(message);
    if (match) {
      // decode HTML entities
      const textArea = document.createElement('textarea');
      textArea.innerHTML = match[1];
      const decodedMessage = textArea.value;
      console.log('PARSED MESSAGE', decodedMessage)
      setCodeInput(decodedMessage);
    }
  }

  const sendMessage = useCallback(
    async (event) => {
      event.preventDefault();

      if (!api || !message) return;

      // Create a message object
      const data = message

      // Send a message to the node via websocket
      // UNCOMMENT THE FOLLOWING 2 LINES to send message via websocket
      // api.send({ data });
      // setMessage("");

      // Send a message to the node via HTTP request
      // IF YOU UNCOMMENTED THE LINES ABOVE, COMMENT OUT THIS try/catch BLOCK
      try {
        chatHistory.push({ author: window.our?.node, content: message });
        set({ chatHistory });
        setMessage("");
        setAwaitingResponse(true);
        scrollToBottom();

        const result = await fetch(`${BASE_URL}/prompt`, {
          method: "POST",
          body: JSON.stringify(data),
        });

        if (!result.ok) throw new Error("HTTP request failed");
      } catch (error) {
        console.error(error);
      }
    },
    [api, message, setMessage, set]
  );

  const onRun = useCallback(
    async (event) => {
      event.preventDefault();

      if (!api || !codeInput) return;

      // Create a message object
      const data = codeInput

      // Send a message to the node via websocket
      // UNCOMMENT THE FOLLOWING 2 LINES to send message via websocket
      // api.send({ data });
      // setMessage("");

      // Send a message to the node via HTTP request
      // IF YOU UNCOMMENTED THE LINES ABOVE, COMMENT OUT THIS try/catch BLOCK
      try {
        const result = await fetch(`${BASE_URL}/run`, {
          method: "POST",
          body: JSON.stringify(data),
        });

        if (!result.ok) throw new Error("HTTP request failed");
        setAwaitingRun(true);
      } catch (error) {
        console.error(error);
      }
    },
    [api, codeInput, setCodeOutput]
  );

  const scrollerRef = useRef<HTMLDivElement>(null);
  const scrollToBottom = () => {
    if (scrollerRef.current) {
      setTimeout(() => {

        scrollerRef.current.scrollTop = scrollerRef.current.scrollHeight;
      }, 500)
    }
  }

  return (
    <div className='w-screen h-screen max-w-screen max-h-screen flex flex-col items-center justify-center'>
      <div className='absolute top-4 left-8'>
        ID: <strong>{window.our?.node}</strong>
      </div>
      {!nodeConnected && (
        <div className="card">
          <h2 className="text-center text-2xl color-red" >
            Node not connected
          </h2>
          <h4 className="text-center" >
            You need to start a node at {PROXY_TARGET} before you can use this UI
            in development.
          </h4>
        </div>
      )}
      <h2 className="text-center text-2xl m-2 font-bold">
        Kinode Python AI Sandbox
      </h2>
      <div className='flex max-h-[90vh]'>
        <div className="rounded-xl border m-1 flex flex-col basis-1/3 flex-1 px-4 py-2 place-items-center place-content-center">
          <h3 className="text-center text-xl font-bold mt-0">Code</h3>
          <div className="flex flex-col grow self-stretch px-2 py-1">
            <textarea
              className="text-xs grow self-stretch mb-1 font-mono resize-none overflow-y-auto p-2 bg-gray-100"
              value={codeInput}
              onChange={(event) => setCodeInput(event.target.value)}
              disabled={awaitingRun}
            />
            <div className="flex justify-center mb-1">
              <button onClick={onRun} disabled={awaitingRun}>
                {awaitingRun ? 'Running...' : 'Run'}
              </button>
            </div>
            <textarea
              className="text-xs grow self-stretch p-2 font-mono resize-none overflow-y-auto bg-gray-100"
              value={codeOutput}
              onChange={(event) => setCodeOutput(event.target.value)}
              readOnly
            />
          </div>
        </div>
        <div className="rounded-xl border m-1 flex flex-col basis-2/3 justify-between px-4 py-2 place-items-center place-content-center">
          <h3 className="text-xl font-bold">Chat</h3>
          <div className="px-2 py-1 overflow-y-auto" ref={scrollerRef}>
            <ul className="flex flex-col mb-2">
              {parsedChats.map((message, index) => (
                <li 
                  key={index} 
                  className={classNames('flex markdown mb-1 text-wrap rounded-lg px-4 py-2 align-left', { 
                    'self-end bg-blue-500 text-white': message.author === window.our?.node, 
                    'self-start bg-gray-200 mr-8': message.author !== window.our?.node,
                  })} 
                >
                  <div 
                    dangerouslySetInnerHTML={{ __html: message.content}}
                  ></div>
                  {message.author === AI_NAME && <button 
                    className={classNames('text-xs px-2 py-1 mr-[-8px] ml-2 rounded self-start bg-gray-800')}
                    onClick={(event) => {
                      parseMessageForCode(message.content);
                    }}
                    disabled={awaitingResponse || awaitingRun}
                  >
                    Select
                  </button>}
                </li>
              ))}
            </ul>
          </div>
          <form
            onSubmit={sendMessage}
            className="flex flex-col w-full mt-1"
          >
            <div className="flex">
              <input
                type="text"
                id="message"
                placeholder="Message"
                value={message}
                onChange={(event) => setMessage(event.target.value)}
                autoFocus
                disabled={awaitingResponse || awaitingRun}
              />
              <button type="submit">Send</button>
            </div>
          </form>
        </div>
      </div> 
      <div 
        className={classNames("loading-overlay absolute top-0 left-0 w-screen h-screen bg-gray-300 opacity-50 flex items-center justify-center", {
          hidden: !(awaitingResponse || awaitingRun)
        })}
      >
        <div className="lds-dual-ring">
          <div></div>
          <div></div>
        </div>
      </div>
    </div>
  );
}

export default App;
