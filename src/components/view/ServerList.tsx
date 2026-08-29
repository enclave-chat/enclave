import Enclave from "@/app/app";
import { getHTTPUrl, saveServerList } from "@/lib/serverList";
import { AddServerDialog } from "../dialog/AddServerDialog";
import { cn } from "@/lib/utils";
import { Button } from "../ui/button";
import { CommandIcon } from "lucide-react";

export default function ServerList({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  if (!appRef.current) return null;

  return (
    <aside className="w-16 h-screen p-2 flex flex-col gap-2.5 border-r border-r-border shrink-0">
      <Button
        variant="secondary"
        className="aspect-square w-full h-auto rounded-full"
        onClick={() => {
          if (appRef.current) {
            appRef.current.sidebarPageKind = "main";
            appRef.current.page = undefined;
            appRef.current.forceRender();
          }
        }}
      >
        <CommandIcon />
      </Button>
      {appRef.current.serverList.map((server) => {
        return (
          <img
            src={getHTTPUrl(server.hostname, server.isSecure, "/icon")}
            key={server.hostname}
            className={cn(
              "rounded-lg",
              server.hostname === appRef.current?.server?.hostname &&
                "border-primary border border-2",
            )}

            onClick={async () => {
              if (!appRef.current) {
                console.error("AppRef is not initialized yet");
                return;
              }

              appRef.current.connectToServer(server.hostname, server.isSecure);
            }}
          />
        );
      })}
      <AddServerDialog
        onAdd={(hostname, isSecure) => {
          if (!appRef.current) {
            console.error("AppRef is not initialized yet");
            return;
          }

          appRef.current.connectToServer(hostname, isSecure).then(() => {
            if (!appRef.current) return;

            saveServerList(appRef.current.serverList);
          });
        }}
      />
    </aside>
  );
}
