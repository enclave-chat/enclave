import Enclave from "@/app/app";
import { getHTTPUrl, saveServerList } from "@/lib/serverList";
import { AddServerDialog } from "../dialog/AddServerDialog";

export default function ServerList({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  if (!appRef.current) return null;

  return (
    <aside className="bg-card w-16 h-screen p-2 flex flex-col gap-2.5 border-r border-r-border">
      {Object.entries(appRef.current.serverList).map(([hostname, server]) => {
        return (
          <img
            src={getHTTPUrl(hostname, server.isSecure, "/icon")}
            key={hostname}
            className={
              hostname === appRef.current?.server?.hostname
                ? "rounded-lg"
                : "rounded-full"
            }

            onClick={async () => {
              if (!appRef.current) {
                console.error("AppRef is not initialized yet");
                return;
              }

              appRef.current.connectToServer(hostname, server.isSecure);
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
