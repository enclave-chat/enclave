import { useRef } from "react";
import Enclave from "@/app/app";

export default function App() {
  const appRef = useRef<Enclave | null>(null);
  if (!appRef.current) {
    appRef.current = new Enclave();
  }

  return (
    <main className="size-screen dark bg-background">
      <aside className="bg-card w-16 h-screen p-2 flex flex-col gap-2.5">
        <img
          src="https://github.com/selimaj-dev.png"
          className="rounded-full"
        />
        <img src="https://github.com/selimaj-dev.png" className="rounded-lg" />
      </aside>
    </main>
  );
}
