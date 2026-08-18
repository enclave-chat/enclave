import { useRef } from "react";
import Enclave from "@/app/app";
import ServerList from "./components/view/serverList";

export default function App() {
  const appRef = useRef<Enclave | null>(null);
  if (!appRef.current) {
    appRef.current = new Enclave();
    appRef.current
      .init()
      .then(() => {
        console.log("Encalve initialized");
      })
      .catch(console.error);
  }

  return (
    <main className="size-screen dark bg-background">
      <ServerList appRef={appRef} />
    </main>
  );
}
