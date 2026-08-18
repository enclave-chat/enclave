import { useReducer, useRef } from "react";
import Enclave from "@/app/app";
import ServerList from "./components/view/serverList";
import { NewProfilePage } from "./components/view/NewProfileView";

export default function App() {
  const appRef = useRef<Enclave | null>(null);
  const [, forceRender] = useReducer((x) => x + 1, 0);
  if (!appRef.current) {
    appRef.current = new Enclave();

    appRef.current.forceRender = forceRender;

    appRef.current
      .init()
      .then(() => {
        console.log("Encalve initialized");
        forceRender();
      })
      .catch(console.error);
  }

  return (
    <main className="size-screen">
      {appRef.current?.accounts &&
      appRef.current.accounts.accounts.length === 0 ? (
        <NewProfilePage appRef={appRef} />
      ) : (
        <ServerList appRef={appRef} />
      )}
    </main>
  );
}
