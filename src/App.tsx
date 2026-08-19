import { useReducer, useRef } from "react";
import Enclave from "@/app/app";
import ServerList from "./components/view/ServerList";
import { NewProfilePage } from "./components/view/NewProfileView";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "./components/ui/resizable";
import Sidebar from "./components/view/Sidebar";
import { ThemeProvider } from "next-themes";

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
    <ThemeProvider attribute="class" defaultTheme="system">
      <main className="size-screen flex">
        {appRef.current?.accounts &&
        appRef.current.accounts.accounts.length === 0 ? (
          <NewProfilePage appRef={appRef} />
        ) : (
          <>
            <ServerList appRef={appRef} />
            <ResizablePanelGroup orientation="horizontal" className="h-screen">
              <ResizablePanel defaultSize="420px">
                <Sidebar appRef={appRef} />
              </ResizablePanel>
              <ResizableHandle withHandle />
              <ResizablePanel defaultSize="100%">
                <div className="h-screen"></div>
              </ResizablePanel>
            </ResizablePanelGroup>
          </>
        )}
      </main>
    </ThemeProvider>
  );
}
