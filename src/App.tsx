import { useEffect, useRef, useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { EnclaveWebSocket } from "./protocol/ws";
import * as ed from "@noble/ed25519";
import { base58 } from "@scure/base";
import { sha512 } from "@noble/hashes/sha2.js";

ed.hashes.sha512 = sha512;

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  const initialized = useRef(false);

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;

    (async () => {
      const ws = new EnclaveWebSocket("ws://localhost:3415");

      const timestamp = Date.now();
      const hostname = "localhost:3415";

      const msg = new TextEncoder().encode(`${timestamp}@${hostname}`);

      const PRIVATE_KEY = ed.utils.randomSecretKey();

      ws.send({
        method: "Initialize",
        public_key: base58.encode(ed.getPublicKey(PRIVATE_KEY)),
        signature: base58.encode(ed.sign(msg, PRIVATE_KEY)),

        timestamp,
        hostname,
      });

      console.log(await ws.read());
    })();
  }, []);

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <main className="container">
      <h1>Welcome to Tauri + React</h1>

      <div className="row">
        <a href="https://vite.dev" target="_blank">
          <img src="/vite.svg" className="logo vite" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <p>Click on the Tauri, Vite, and React logos to learn more.</p>

      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <button type="submit">Greet</button>
      </form>
      <p>{greetMsg}</p>
    </main>
  );
}

export default App;
