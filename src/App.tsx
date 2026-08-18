import { useRef, useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import Enclave from "./app/app";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  const appRef = useRef<Enclave | null>(null);
  if (!appRef.current) {
    appRef.current = new Enclave();
  }

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <main className="bg-black">
      <div></div>
    </main>
  );
}

export default App;
