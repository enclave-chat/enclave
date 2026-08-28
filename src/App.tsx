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
import Page from "./components/page/PageView";
import { SettingsDialog } from "./components/settings/SettingsDialog";
import { TooltipProvider } from "./components/ui/tooltip";

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
      <TooltipProvider>
        <main className="size-screen flex select-none cursor-default">
          {appRef.current?.accounts &&
          appRef.current.accounts.accounts.length === 0 ? (
            <NewProfilePage appRef={appRef} />
          ) : (
            <>
              <ResizablePanelGroup
                orientation="horizontal"
                className="h-screen"
              >
                <ResizablePanel
                  defaultSize="600px"
                  maxSize="400px"
                  minSize="4rem"
                  groupResizeBehavior="preserve-pixel-size"
                >
                  <div className="flex">
                    <ServerList appRef={appRef} />
                    <Sidebar appRef={appRef} />
                  </div>
                </ResizablePanel>
                <ResizableHandle withHandle />

                <ResizablePanel defaultSize="100%">
                  <Page appRef={appRef} />
                </ResizablePanel>
              </ResizablePanelGroup>

              <SettingsDialog appRef={appRef} />
            </>
          )}
        </main>
      </TooltipProvider>
    </ThemeProvider>
  );
}
